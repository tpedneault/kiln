//! The provider's life (Flint's PROVIDERS.md): `hello`, `provider/register`
//! with the static items, then queries, executes and, when the project
//! changes on disk, `provider/setItems`. One thread reads stdin; the main
//! loop owns the project and writes stdout, so nothing is shared.
//!
//! Which project: `--project` pins one; otherwise the one Fenix is showing
//! when Flint is summoned (`context.fenix.project`), remembered in Kiln's
//! state folder for next time; otherwise the folder it was started in.
//! Without one, it registers no items and answers nothing, which costs
//! Flint nothing.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossbeam_channel::{select, tick, Receiver};
use flint_protocol::frame::{read_frame, write_frame};
use flint_protocol::hello::{Hello, Role, PROTOCOL_VERSION};
use flint_protocol::item::Item;
use flint_protocol::message::{code, Id, Message, RpcError};
use flint_protocol::provider::{
    Execute, ExecuteResult, LogLevel, LogParams, Query, Register, Results, SetItems, MAX_BATCH,
};
use kiln_index::{DiskSource, FileSource, Project, Stamp};
use kiln_parse::{set_field, FieldValue};
use serde_json::{json, Value};

use crate::items::{self, Context};

/// How the provider was started.
#[derive(Debug, Clone)]
pub struct Options {
    /// The name in Flint's `[[providers]]`, which `hello` must repeat.
    pub name: String,
    /// The scope keyword asked for (settings may override it).
    pub prefix: String,
    /// `--project`: always this project.
    pub pinned: Option<PathBuf>,
    /// The folder it started in.
    pub start: Option<PathBuf>,
    /// Kiln's state folder (the last project, the Confluence page cache).
    pub state: Option<PathBuf>,
}

/// How often the project is checked for changes on disk.
const POLL: Duration = Duration::from_secs(2);
/// Flint re-indexes a provider's items at most this often.
const SET_ITEMS_EVERY: Duration = Duration::from_millis(250);

struct Loaded {
    root: PathBuf,
    project: Project,
    stamps: BTreeMap<String, Stamp>,
}

struct Provider<W: Write> {
    out: W,
    opts: Options,
    current: Option<Loaded>,
    next_id: i64,
    last_set: Option<Instant>,
    /// Items changed while a `setItems` would come too soon.
    owed: bool,
}

/// Item files and `kiln.toml` with their stamps: what a change shows in.
fn stamps(root: &Path, project: &Project) -> BTreeMap<String, Stamp> {
    let disk = DiskSource::new(root);
    let mut out = BTreeMap::new();
    if let Ok(listing) = disk.list(&project.config.paths, &[], Some("md")) {
        for f in listing.files {
            if let Ok(s) = Stamp::of(&disk.path(&f)) {
                out.insert(f, s);
            }
        }
    }
    if let Ok(s) = Stamp::of(&root.join(kiln_parse::CONFIG_FILE)) {
        out.insert(kiln_parse::CONFIG_FILE.to_owned(), s);
    }
    out
}

fn load(root: &Path) -> Result<Loaded, String> {
    let project =
        Project::load(&DiskSource::new(root)).map_err(|e| format!("{}: {e}", root.display()))?;
    Ok(Loaded {
        stamps: stamps(root, &project),
        root: root.to_path_buf(),
        project,
    })
}

fn last_project_file(state: &Path) -> PathBuf {
    state.join("flint-project")
}

/// Serves Flint on `input` and `output` until `input` ends.
pub fn serve<R, W>(input: R, output: W, opts: Options) -> Result<(), String>
where
    R: BufRead + Send + 'static,
    W: Write,
{
    let (tx, rx) = crossbeam_channel::unbounded();
    std::thread::spawn(move || {
        let mut input = input;
        while let Ok(Some(body)) = read_frame(&mut input) {
            match Message::parse(&body) {
                Ok(m) => {
                    if tx.send(m).is_err() {
                        return;
                    }
                }
                Err(e) => eprintln!("kiln provider: ignored a message: {e}"),
            }
        }
    });
    let mut p = Provider {
        out: output,
        opts,
        current: None,
        next_id: 1,
        last_set: None,
        owed: false,
    };
    p.handshake(&rx)?;
    p.run(&rx)
}

impl<W: Write> Provider<W> {
    fn send(&mut self, m: &Message) -> Result<(), String> {
        write_frame(&mut self.out, &m.to_bytes()).map_err(|e| format!("writing to Flint: {e}"))
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Id, String> {
        let id = Id::Number(self.next_id);
        self.next_id += 1;
        self.send(&Message::request(id.clone(), method, params))?;
        Ok(id)
    }

    fn notify(&mut self, method: &str, params: impl serde::Serialize) -> Result<(), String> {
        let params = serde_json::to_value(params).map_err(|e| e.to_string())?;
        self.send(&Message::Notification {
            method: method.to_owned(),
            params,
        })
    }

    fn log(&mut self, level: LogLevel, message: String) -> Result<(), String> {
        self.notify("log", LogParams { level, message })
    }

    fn ctx_parts(&self) -> Option<(&Loaded, Context<'_>)> {
        let loaded = self.current.as_ref()?;
        Some((
            loaded,
            Context {
                provider: &self.opts.name,
                prefix: &self.opts.prefix,
                root: &loaded.root,
                confluence: crate::confluence::target(&loaded.project).is_some(),
            },
        ))
    }

    /// The static items, cut to fit one Flint message: about 800 bytes
    /// each, so the cut only bites past roughly 4,500 open items, and then
    /// the last ones (by type, then ID) are left to `all`.
    fn static_items(&self) -> Vec<Item> {
        let mut items = self
            .ctx_parts()
            .map(|(l, ctx)| items::items(&l.project, &ctx))
            .unwrap_or_default();
        let budget = flint_protocol::frame::MAX_BODY - 256 * 1024;
        let size = |items: &[Item]| serde_json::to_vec(items).map_or(usize::MAX, |b| b.len());
        let total = size(&items);
        if total > budget {
            let keep = items.len() * budget / total;
            eprintln!(
                "kiln provider: {} items are {} KiB, over what one Flint message carries; registering the first {keep} (the rest through `all`)",
                items.len(),
                total / 1024
            );
            items.truncate(keep);
        }
        items
    }

    /// `hello`, then `provider/register` with the items.
    fn handshake(&mut self, rx: &Receiver<Message>) -> Result<(), String> {
        let hello = Hello {
            protocol: PROTOCOL_VERSION,
            role: Role::Provider,
            name: self.opts.name.clone(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            pid: std::process::id(),
        };
        let id = self.request(
            "hello",
            serde_json::to_value(hello).map_err(|e| e.to_string())?,
        )?;
        loop {
            match rx.recv() {
                Ok(Message::Response {
                    id: Some(got),
                    outcome,
                }) if got == id => {
                    outcome.map_err(|e| format!("Flint refused hello: {}", e.message))?;
                    break;
                }
                Ok(_) => {}
                Err(_) => return Err("Flint closed the connection before answering hello".into()),
            }
        }
        let start = self
            .opts
            .pinned
            .clone()
            .or_else(|| {
                let state = self.opts.state.as_ref()?;
                let text = std::fs::read_to_string(last_project_file(state)).ok()?;
                Some(PathBuf::from(text.trim()))
            })
            .or_else(|| self.opts.start.clone());
        if let Some(root) = start.as_deref().and_then(kiln_index::find_root) {
            match load(&root) {
                Ok(l) => self.current = Some(l),
                Err(e) => eprintln!("kiln provider: {e}"),
            }
        }
        let register = Register {
            id: self.opts.name.clone(),
            title: "Kiln".into(),
            prefix: Some(self.opts.prefix.clone()),
            // Dynamic only for the drill-downs and to see which project
            // Fenix is showing; every other query gets an empty answer.
            dynamic: true,
            debounce_ms: 30,
            min_query_length: 0,
            items: self.static_items(),
        };
        self.last_set = Some(Instant::now());
        self.request(
            "provider/register",
            serde_json::to_value(register).map_err(|e| e.to_string())?,
        )?;
        Ok(())
    }

    fn run(&mut self, rx: &Receiver<Message>) -> Result<(), String> {
        let ticker = tick(POLL);
        loop {
            select! {
                recv(rx) -> m => {
                    let Ok(m) = m else { return Ok(()) };
                    self.message(m)?;
                }
                recv(ticker) -> _ => {
                    self.poll()?;
                    if self.owed {
                        self.set_items()?;
                    }
                }
            }
        }
    }

    fn message(&mut self, m: Message) -> Result<(), String> {
        match m {
            Message::Request { id, method, params } => {
                let outcome = match method.as_str() {
                    "execute" => {
                        match flint_protocol::message::params::<Execute>(&method, params) {
                            Ok(e) => self.execute(&e).map(|r| json!(r)),
                            Err(e) => Err(e),
                        }
                    }
                    other => Err(RpcError::new(
                        code::METHOD_NOT_FOUND,
                        format!("kiln provider doesn't answer `{other}`"),
                    )),
                };
                self.send(&Message::Response {
                    id: Some(id),
                    outcome,
                })
            }
            Message::Notification { method, params } if method == "query" => {
                match serde_json::from_value::<Query>(params) {
                    Ok(q) => self.query(&q),
                    Err(e) => self.log(LogLevel::Warn, format!("a query Kiln couldn't read: {e}")),
                }
            }
            Message::Response {
                outcome: Err(e), ..
            } => self.log(LogLevel::Error, format!("Flint refused: {}", e.message)),
            _ => Ok(()),
        }
    }

    /// Sends the items again, at most every 250 ms.
    fn set_items(&mut self) -> Result<(), String> {
        if self.last_set.is_some_and(|t| t.elapsed() < SET_ITEMS_EVERY) {
            self.owed = true;
            return Ok(());
        }
        self.owed = false;
        self.last_set = Some(Instant::now());
        let items = self.static_items();
        self.notify("provider/setItems", SetItems { items })
    }

    /// Reloads when an item file or `kiln.toml` changed on disk.
    fn poll(&mut self) -> Result<(), String> {
        let Some(l) = &self.current else {
            return Ok(());
        };
        if stamps(&l.root, &l.project) == l.stamps {
            return Ok(());
        }
        let root = l.root.clone();
        match load(&root) {
            Ok(l) => {
                self.current = Some(l);
                self.set_items()
            }
            Err(e) => self.log(LogLevel::Warn, e),
        }
    }

    /// Follows Fenix to another Kiln project.
    fn follow(&mut self, q: &Query) -> Result<(), String> {
        if self.opts.pinned.is_some() {
            return Ok(());
        }
        let Some(fenix) = &q.context.fenix else {
            return Ok(());
        };
        let Some(start) = fenix.project.as_deref().or(fenix.file.as_deref()) else {
            return Ok(());
        };
        let Some(root) = kiln_index::find_root(Path::new(start)) else {
            return Ok(());
        };
        if self.current.as_ref().is_some_and(|l| l.root == root) {
            return Ok(());
        }
        match load(&root) {
            Ok(l) => {
                self.current = Some(l);
                if let Some(state) = &self.opts.state {
                    let _ = std::fs::create_dir_all(state);
                    let _ = fenix_storage::write(
                        &last_project_file(state),
                        root.to_string_lossy().as_bytes(),
                    );
                }
                self.last_set = None;
                self.set_items()
            }
            Err(e) => self.log(LogLevel::Warn, e),
        }
    }

    fn query(&mut self, q: &Query) -> Result<(), String> {
        self.follow(q)?;
        let found = self
            .ctx_parts()
            .and_then(|(l, ctx)| items::drilldown(&l.project, &q.text, &ctx))
            .unwrap_or_default();
        let mut batches: Vec<Vec<Item>> = found.chunks(MAX_BATCH).map(<[Item]>::to_vec).collect();
        if batches.is_empty() {
            batches.push(Vec::new());
        }
        let last = batches.len() - 1;
        for (n, items) in batches.into_iter().enumerate() {
            self.notify(
                "results",
                Results {
                    query_id: q.query_id,
                    items,
                    done: n == last,
                },
            )?;
        }
        Ok(())
    }

    fn execute(&mut self, e: &Execute) -> Result<ExecuteResult, RpcError> {
        let failed = |m: String| RpcError::new(code::PROVIDER_FAILED, m);
        let (l, ctx) = self
            .ctx_parts()
            .ok_or_else(|| failed("no Kiln project is loaded".into()))?;
        let (id, tail) = items::parse_item_id(&ctx, &l.project, &e.item_id).ok_or_else(|| {
            failed(format!(
                "`{}` isn't an item of {}; summon Flint again",
                e.item_id, l.project.config.name
            ))
        })?;
        // An impact result acts on the item it lists.
        let id = match tail.strip_prefix("impact:").map(str::parse) {
            Some(Ok(source)) => source,
            _ => id,
        };
        match e.action_id.as_str() {
            "set-status" => {
                let status = tail
                    .strip_prefix("status:")
                    .ok_or_else(|| failed("not a status choice".into()))?;
                let message = self.set_status(&id, status).map_err(failed)?;
                Ok(ExecuteResult {
                    close: true,
                    message: Some(message),
                })
            }
            "open-confluence" => {
                let url = crate::confluence::url(&l.project, self.opts.state.as_deref(), &id)
                    .map_err(failed)?;
                kiln_fenix::open_url(&url).map_err(failed)?;
                Ok(ExecuteResult {
                    close: true,
                    message: Some(format!("Opened {id} in Confluence")),
                })
            }
            "clock-in" => {
                let item = l
                    .project
                    .item(&id)
                    .ok_or_else(|| failed(format!("{id} doesn't exist")))?;
                let key = item.jira.first().ok_or_else(|| {
                    failed(format!("{id} has no Jira key to find its agenda task by"))
                })?;
                let task = kiln_fenix::clock_in_current(key).map_err(failed)?;
                Ok(ExecuteResult {
                    close: true,
                    message: Some(format!("Clocked in on {task}")),
                })
            }
            other => Err(RpcError::new(
                code::METHOD_NOT_FOUND,
                format!("Kiln has no action `{other}`"),
            )),
        }
    }

    /// The front-matter edit `kiln set ID status=…` makes.
    fn set_status(&mut self, id: &kiln_model::ItemId, status: &str) -> Result<String, String> {
        let l = self.current.as_ref().ok_or("no Kiln project is loaded")?;
        let copies = l.project.indices(id);
        let item = match copies {
            [one] => &l.project.items[*one],
            [] => return Err(format!("{id} doesn't exist")),
            _ => {
                return Err(format!(
                    "{id} is in {} files; `kiln renumber` first",
                    copies.len()
                ))
            }
        };
        let ty = l
            .project
            .type_of(item)
            .ok_or_else(|| format!("{id}'s type isn't in kiln.toml"))?;
        if !ty.statuses.iter().any(|s| s == status) {
            return Err(format!("`{status}` isn't a status of {}", ty.name));
        }
        let before = item.status.clone();
        if before == status {
            return Ok(format!("{id} is already {status}"));
        }
        let path = l.root.join(&item.path);
        let rel = item.path.clone();
        let file = kiln_index::read_for_edit(&path).map_err(|e| e.to_string())?;
        let text =
            kiln_parse::normalize(&file.bytes).ok_or_else(|| format!("{rel} isn't UTF-8"))?;
        let new = set_field(&text, "status", Some(FieldValue::String(status.to_owned())))
            .map_err(|e| format!("{rel}: {e}"))?;
        kiln_index::write_back(&path, &rel, &file.bytes, file.stamp, &new, true)
            .map_err(|e| e.to_string())?;
        let root = l.root.clone();
        if let Ok(l) = load(&root) {
            self.current = Some(l);
            self.set_items()?;
        }
        Ok(format!("{id}: {before} → {status}"))
    }
}
