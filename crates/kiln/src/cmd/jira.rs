//! The Jira mirror as the commands use it: the cached mirror when it is
//! fresh, else one fetch with the token from `KILN_JIRA_TOKEN` or the
//! credential store, cached in the state folder for the next command
//! (INTEGRATION.md, "Jira"). `kiln auth jira` is here too, and (K10)
//! `kiln jira sync` and `kiln jira pull`, which push the items Kiln
//! manages to Jira and take a ticket's status back.

use std::io::{IsTerminal, Write};
use std::path::Path;

use kiln_index::Project;
use kiln_jira::Mirror;

use super::export::Answer;
use crate::project::{folders, load};
use crate::{Fail, EXIT_FINDINGS, EXIT_NETWORK};

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The mirror for every key the project links. An error is one line
/// naming what's missing (no `[jira] url`, no token) or what Jira said.
pub fn mirror(project: &Project) -> Result<Mirror, String> {
    let url = project
        .config
        .jira_url
        .as_deref()
        .ok_or("kiln.toml has no [jira] url to read tickets from")?;
    mirror_for(url, &kiln_jira::keys(project))
}

/// The mirror of `keys` from the Jira at `url`.
pub fn mirror_for(url: &str, keys: &[String]) -> Result<Mirror, String> {
    let url = url.trim_end_matches('/');
    let keys = keys.to_vec();
    let now = now();
    if keys.is_empty() {
        return Ok(Mirror {
            url: url.to_owned(),
            fetched: now,
            ..Mirror::default()
        });
    }
    let cache = folders().map(|f| f.state.join(kiln_jira::cache_name(url)));
    if let Some(cached) = cache
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| Mirror::parse(&t))
    {
        if cached.covers(url, &keys, now) {
            return Ok(cached);
        }
    }
    if kiln_jira::is_cloud(url) {
        return Err(kiln_jira::cloud_refusal(url));
    }
    let (token, _) = kiln_secrets::jira_token_for(url)?.ok_or_else(|| {
        format!(
            "no Jira token for {url}: set {} or run `kiln auth jira`",
            kiln_secrets::JIRA_TOKEN
        )
    })?;
    let mirror = kiln_jira::fetch(url, token.expose(), &keys, now)?;
    // A cache that can't be written costs a fetch next time, nothing more.
    if let Some(path) = cache {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = fenix_storage::write(&path, mirror.to_text().as_bytes());
    }
    Ok(mirror)
}

/// `kiln auth jira`: asks for a personal access token without echo,
/// checks it with one search and stores it as `fenix-suite`/`jira:<url>`,
/// where Flint finds it too; `--forget` removes it.
pub fn auth(project: Option<&Path>, forget: bool) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let url = loaded
        .project
        .config
        .jira_url
        .clone()
        .ok_or_else(|| Fail::usage("kiln.toml has no [jira] url to store a token for"))?;
    let url = url.trim_end_matches('/').to_owned();
    let account = kiln_secrets::account("jira", &url);
    if forget {
        kiln_secrets::forget(&account).map_err(Fail::usage)?;
        println!("removed the token for {url} from the credential store");
        return Ok(0);
    }
    if kiln_jira::is_cloud(&url) {
        return Err(Fail::usage(kiln_jira::cloud_refusal(&url)));
    }
    if !std::io::stdin().is_terminal() {
        return Err(Fail::usage(format!(
            "kiln auth asks for the token on a terminal; in CI, set {} instead",
            kiln_secrets::JIRA_TOKEN
        )));
    }
    let token =
        kiln_secrets::prompt(&format!("Personal access token for {url}: ")).map_err(Fail::usage)?;
    kiln_jira::sign_in(&url, token.expose())
        .map_err(|e| Fail::new(EXIT_NETWORK, format!("the token wasn't stored: {e}")))?;
    kiln_secrets::store(&account, &token).map_err(Fail::usage)?;
    println!(
        "stored the token for {url} in the credential store as {}/{account}",
        kiln_secrets::SERVICE
    );
    Ok(0)
}

/// What a sync needs that every run reads the same way.
struct Ready {
    loaded: crate::project::Loaded,
    sync: kiln_model::JiraSync,
    url: String,
    token: kiln_secrets::Token,
}

fn ready(project: Option<&Path>) -> Result<Ready, Fail> {
    let loaded = load(project)?;
    let config = &loaded.project.config;
    let url = config
        .jira_url
        .clone()
        .ok_or_else(|| Fail::usage("kiln.toml has no [jira] url"))?
        .trim_end_matches('/')
        .to_owned();
    let sync = config.jira_sync.clone().ok_or_else(|| {
        Fail::usage(
            "kiln.toml's [jira] has no `project`: say which Jira project Kiln's tickets go in (docs/FORMAT.md)",
        )
    })?;
    if kiln_jira::is_cloud(&url) {
        return Err(Fail::usage(kiln_jira::cloud_refusal(&url)));
    }
    let (token, _) = kiln_secrets::jira_token_for(&url)
        .map_err(Fail::usage)?
        .ok_or_else(|| {
            Fail::usage(format!(
                "no Jira token for {url}: set {} or run `kiln auth jira`",
                kiln_secrets::JIRA_TOKEN
            ))
        })?;
    Ok(Ready {
        loaded,
        sync,
        url,
        token,
    })
}

/// The text a new sub-task starts with.
fn description(project: &Project, item: &kiln_model::Item) -> String {
    format!(
        "Managed by Kiln: {} in the {} repository. Edit the item there; `kiln jira sync` keeps this ticket's summary, due date and status in step.",
        item.path, project.config.name
    )
}

/// The plan, as `kiln jira sync --dry-run` prints it.
fn plan_lines(
    sync: &kiln_model::JiraSync,
    remote: &kiln_jira::sync::Remote,
    plan: &kiln_jira::sync::Plan,
) -> Vec<String> {
    use kiln_jira::sync::Op;
    let mut out = vec![match (&sync.parent, &remote.parent) {
        (Some(key), Some(p)) => format!("jira ({}) → {key} \"{}\"", sync.project, p.summary),
        _ => format!(
            "jira ({}) → a new project issue ({})",
            sync.project, sync.parent_type
        ),
    }];
    let sub = &remote.subtask_type.1;
    for op in &plan.ops {
        out.push(match op {
            Op::CreateParent { summary } => format!(
                "  create     \"{summary}\" ({}), the project issue",
                sync.parent_type
            ),
            Op::Create {
                summary, target, ..
            } => format!(
                "  create     {summary} ({sub}){}",
                target
                    .as_ref()
                    .map(|t| format!(", then to {}", t.describe()))
                    .unwrap_or_default()
            ),
            Op::Update {
                item,
                key,
                summary,
                due,
            } => {
                let mut what = Vec::new();
                if let Some(s) = summary {
                    what.push(format!("summary \"{s}\""));
                }
                if let Some(d) = due {
                    what.push(format!("due {}", d.as_deref().unwrap_or("(none)")));
                }
                format!("  update     {key} {item}: {}", what.join("; "))
            }
            Op::Transition {
                item,
                key,
                from,
                to,
                ..
            } => format!("  move       {key} {item}: {from} → {to}"),
            Op::Refused { item, key, why } => format!("  refused    {key} {item}: {why}"),
            Op::Orphan { key, summary } => {
                format!("  orphan     {key} \"{summary}\": no synced item links it (left alone)")
            }
            Op::LeftAlone { item, keys } => format!(
                "  left alone {item}: linked to {}, not a sub-task of the project issue",
                keys.join(", ")
            ),
        });
    }
    let creates = plan.count(|o| matches!(o, Op::Create { .. } | Op::CreateParent { .. }));
    let updates = plan.count(|o| matches!(o, Op::Update { .. }));
    let moves = plan.count(|o| matches!(o, Op::Transition { .. }));
    let refused = plan.count(|o| matches!(o, Op::Refused { .. }));
    out.push(format!(
        "  {creates} create, {updates} update, {moves} move, {} unchanged{}",
        plan.unchanged,
        if refused == 0 {
            String::new()
        } else {
            format!(", {refused} refused")
        }
    ));
    for r in &plan.refusals {
        out.push(format!("  refused: {r}"));
    }
    out
}

/// `kiln jira sync`: plans the project's tickets, prints the plan and,
/// without `--dry-run`, applies it after a yes, then records the keys it
/// created in the items and `kiln.toml`.
pub fn sync(project: Option<&Path>, dry_run: bool, yes: bool) -> Result<u8, Fail> {
    let mut confirm = |summary: &str, _plan: &str| {
        let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
        if !interactive {
            return Answer::CantAsk;
        }
        print!("Sync {summary}? [y/N] ");
        let _ = std::io::stdout().flush();
        let mut answer = String::new();
        let _ = std::io::stdin().read_line(&mut answer);
        if matches!(answer.trim(), "y" | "Y" | "yes") {
            Answer::Yes
        } else {
            Answer::No
        }
    };
    sync_with(project, dry_run, yes, &mut std::io::stdout(), &mut confirm)
}

/// [`sync`] with its output and its question elsewhere (the GUI): the
/// question gets the plan's first line and the plan as printed.
pub fn sync_with(
    project: Option<&Path>,
    dry_run: bool,
    yes: bool,
    out: &mut dyn Write,
    confirm: &mut dyn FnMut(&str, &str) -> Answer,
) -> Result<u8, Fail> {
    let r = ready(project)?;
    let p = &r.loaded.project;
    let remote = kiln_jira::push::read(&r.url, r.token.expose(), p, &r.sync)
        .map_err(|e| Fail::new(EXIT_NETWORK, e))?;
    let mut plan = kiln_jira::sync::plan(p, &r.sync, &remote, &|i| description(p, i));
    let lines = plan_lines(&r.sync, &remote, &plan);
    let shown: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let _ = out.write_all(shown.as_bytes());
    let refused = plan.refused();
    if dry_run || !plan.changes() {
        return Ok(if refused { EXIT_FINDINGS } else { 0 });
    }
    if !yes {
        match confirm(&lines[0], &shown) {
            Answer::Yes => {}
            Answer::No => {
                let _ = writeln!(out, "  not synced");
                return Ok(EXIT_FINDINGS);
            }
            Answer::CantAsk => {
                eprintln!("kiln: Jira not synced: pass --yes to sync without asking");
                return Ok(EXIT_FINDINGS);
            }
        }
    }
    if !plan.refusals.is_empty() {
        // Jira would refuse each create; the rest still goes.
        plan.ops.retain(|o| {
            !matches!(
                o,
                kiln_jira::sync::Op::Create { .. } | kiln_jira::sync::Op::CreateParent { .. }
            )
        });
    }
    let done = kiln_jira::push::apply(&r.url, r.token.expose(), &r.sync, &remote, &plan);
    // The keys first, whatever else happened: a rerun must not create
    // these tickets again.
    let source = &r.loaded.source;
    let mut written = 0;
    let mut write_errors = Vec::new();
    if let Some(parent) = &done.parent {
        match kiln_index::apply_config_key(source, "jira", "parent", parent) {
            Ok(()) => written += 1,
            Err(e) => write_errors.push(format!(
                "record `parent = \"{parent}\"` under [jira] in kiln.toml by hand: {e}"
            )),
        }
    }
    for (id, key) in &done.created {
        match kiln_index::apply_jira_key(p, source, id, key) {
            Ok(()) => written += 1,
            Err(e) => write_errors.push(format!("record `jira = \"{key}\"` in {id} by hand: {e}")),
        }
    }
    let _ = writeln!(
        out,
        "jira: {} created, {} updated, {} moved{}",
        done.created.len() + usize::from(done.parent.is_some()),
        done.updated,
        done.moved,
        if written > 0 {
            format!(
                "; {written} file{} gained ticket keys (commit them)",
                if written == 1 { "" } else { "s" }
            )
        } else {
            String::new()
        }
    );
    for r in &done.refused {
        let _ = writeln!(out, "  refused: {r}");
    }
    for e in &write_errors {
        eprintln!("kiln: {e}");
    }
    if let Some(e) = done.error {
        return Err(Fail::new(
            EXIT_NETWORK,
            format!("{e}; run it again to finish"),
        ));
    }
    if !write_errors.is_empty() || refused || !done.refused.is_empty() {
        return Ok(EXIT_FINDINGS);
    }
    Ok(0)
}

/// `kiln jira pull [ID…]`: takes the status of each item's managed ticket
/// into the item, mapped back (`[jira.status]`, else by category), for a
/// ticket someone moved in Jira. Every synced item without IDs.
pub fn pull(project: Option<&Path>, ids: &[String]) -> Result<u8, Fail> {
    pull_with(project, ids, &mut std::io::stdout(), &mut std::io::stderr())
}

/// [`pull`] writing to `out`, and its warnings to `err` (the GUI).
pub fn pull_with(
    project: Option<&Path>,
    ids: &[String],
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Fail> {
    let r = ready(project)?;
    let p = &r.loaded.project;
    let wanted: Vec<kiln_model::ItemId> = ids
        .iter()
        .map(|s| super::parse_id(s))
        .collect::<Result<_, _>>()?;
    let remote = kiln_jira::push::read(&r.url, r.token.expose(), p, &r.sync)
        .map_err(|e| Fail::new(EXIT_NETWORK, e))?;
    let tickets: std::collections::BTreeMap<&str, &kiln_jira::sync::RemoteTicket> = remote
        .subtasks
        .iter()
        .map(|t| (t.key.as_str(), t))
        .collect();
    let mut code = 0;
    let mut changed = 0;
    for item in kiln_jira::sync::synced(p, &r.sync) {
        if !wanted.is_empty() && !wanted.contains(&item.id) {
            continue;
        }
        let Some(t) = item.jira.iter().find_map(|k| tickets.get(k.as_str())) else {
            if !wanted.is_empty() {
                let _ = writeln!(
                    err,
                    "kiln: {} has no ticket under the project issue",
                    item.id
                );
                code = EXIT_FINDINGS;
            }
            continue;
        };
        let Some(status) = kiln_jira::sync::status_back(p, &r.sync, item, &t.status, &t.category)
        else {
            let _ = writeln!(
                err,
                "kiln: {} ({}): the Jira status {} maps to no {} status; add it to [jira.status]",
                item.id, t.key, t.status, item.type_name
            );
            code = EXIT_FINDINGS;
            continue;
        };
        if status == item.status {
            continue;
        }
        let pairs = [("status".to_owned(), status)];
        let changes = kiln_index::apply_set(p, &r.loaded.source, &item.id, &pairs)
            .map_err(crate::project::edit_fail)?;
        for c in changes {
            let _ = writeln!(out, "{} (from {} {})", c.line(), t.key, t.status);
            changed += 1;
        }
    }
    if changed == 0 && code == 0 {
        let _ = writeln!(out, "every item's status already matches its ticket");
    }
    Ok(code)
}
