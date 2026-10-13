//! The provider driven the way Flint drives it: frames over pipes, with
//! the test as the host (DESIGN.md, "Testing"), using `flint-protocol`'s
//! own types and framing.

use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

use crossbeam_channel::Receiver;
use flint_protocol::frame::{read_frame, write_frame};
use flint_protocol::message::{Id, Message};
use serde_json::{json, Value};

use crate::{serve, Options};

const REQ_001: &str = "+++\r\nid = \"REQ-001\"\r\ntype = \"requirement\"\r\ntitle = \"Fast start\"\r\nstatus = \"approved\"\r\njira = \"FLINT-88\"\r\n+++\r\n";
const REQ_002: &str = "+++\nid = \"REQ-002\"\ntype = \"requirement\"\ntitle = \"Cold start\"\nstatus = \"draft\"\nparent = \"REQ-001\"\n+++\n";
const REQ_003: &str = "+++\nid = \"REQ-003\"\ntype = \"requirement\"\ntitle = \"Dropped idea\"\nstatus = \"obsolete\"\n+++\n";
const TEST_001: &str = "+++\nid = \"TEST-001\"\ntype = \"test\"\ntitle = \"Timing\"\nstatus = \"ready\"\n\n[links]\nverifies = [\"REQ-001\"]\n+++\n";

fn project(dir: &Path, name: &str) {
    let config = kiln_model::default_config(name)
        .replace("# [code]\n# paths = [\"src\"]", "[code]\npaths = [\"src\"]")
        + "\n[jira]\nurl = \"https://jira.example.com/\"\n";
    let files = [
        ("kiln.toml", config.as_str()),
        ("design/requirements/REQ-001-fast-start.md", REQ_001),
        ("design/requirements/REQ-002-cold-start.md", REQ_002),
        ("design/requirements/REQ-003-dropped-idea.md", REQ_003),
        ("design/tests/TEST-001-timing.md", TEST_001),
    ];
    for (path, text) in files {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("src/lib.rs"),
        format!("fn main() {{}}\n// {}: REQ-001\n", "implements"),
    )
    .unwrap();
}

/// The test's side: Flint.
struct Host {
    to: std::io::PipeWriter,
    from: Receiver<Message>,
    next: i64,
    /// Notifications that arrived while waiting for something else.
    held: Vec<(String, Value)>,
}

impl Host {
    /// Starts a provider, answers `hello` and `provider/register`, and
    /// returns the registration.
    fn start(opts: Options) -> (Host, Value) {
        let (provider_in, to) = std::io::pipe().unwrap();
        let (from_provider, provider_out) = std::io::pipe().unwrap();
        std::thread::spawn(move || serve(BufReader::new(provider_in), provider_out, opts));
        let (tx, from) = crossbeam_channel::unbounded();
        std::thread::spawn(move || {
            let mut r = BufReader::new(from_provider);
            while let Ok(Some(body)) = read_frame(&mut r) {
                if tx.send(Message::parse(&body).unwrap()).is_err() {
                    return;
                }
            }
        });
        let mut host = Host {
            to,
            from,
            next: 100,
            held: Vec::new(),
        };
        let (id, method, params) = host.request_in();
        assert_eq!(method, "hello");
        assert_eq!(params["role"], "provider");
        assert_eq!(params["protocol"], 1);
        host.send(Message::result(
            id,
            json!({ "protocol": 1, "host": { "name": "flint", "version": "0.1.0", "pid": 1 } }),
        ));
        let (id, method, register) = host.request_in();
        assert_eq!(method, "provider/register");
        host.send(Message::result(id, json!({})));
        (host, register)
    }

    fn send(&mut self, m: Message) {
        write_frame(&mut self.to, &m.to_bytes()).unwrap();
    }

    fn recv(&self) -> Message {
        self.from
            .recv_timeout(Duration::from_secs(20))
            .expect("the provider answers")
    }

    fn request_in(&self) -> (Id, String, Value) {
        match self.recv() {
            Message::Request { id, method, params } => (id, method, params),
            other => panic!("expected a request, got {other:?}"),
        }
    }

    /// The next notification named `method`, skipping `log`s.
    fn notification(&mut self, method: &str) -> Value {
        if let Some(i) = self.held.iter().position(|(m, _)| m == method) {
            return self.held.remove(i).1;
        }
        loop {
            match self.recv() {
                Message::Notification { method: m, params } if m == method => return params,
                Message::Notification { method: m, params } => self.held.push((m, params)),
                other => panic!("expected {method}, got {other:?}"),
            }
        }
    }

    /// Sends a query and collects every batch of its results.
    fn query(&mut self, id: u64, text: &str, context: Value) -> Vec<Value> {
        self.send(Message::Notification {
            method: "query".into(),
            params: json!({ "queryId": id, "text": text, "scoped": true, "context": context }),
        });
        let mut out = Vec::new();
        loop {
            let r = self.notification("results");
            assert_eq!(r["queryId"], id);
            out.extend(r["items"].as_array().unwrap().iter().cloned());
            if r["done"] == true {
                return out;
            }
        }
    }

    fn execute(&mut self, item: &str, action: &str) -> Result<Value, String> {
        let id = Id::Number(self.next);
        self.next += 1;
        self.send(Message::request(
            id.clone(),
            "execute",
            json!({ "itemId": item, "actionId": action, "query": "" }),
        ));
        loop {
            match self.recv() {
                Message::Response {
                    id: Some(got),
                    outcome,
                } if got == id => return outcome.map_err(|e| e.message),
                Message::Notification { method, params } => self.held.push((method, params)),
                other => panic!("expected the answer, got {other:?}"),
            }
        }
    }
}

fn options(root: &Path, state: &Path) -> Options {
    Options {
        name: "kiln".into(),
        prefix: "k".into(),
        pinned: Some(root.to_path_buf()),
        start: None,
        state: Some(state.to_path_buf()),
    }
}

fn titles(items: &[Value]) -> Vec<String> {
    items
        .iter()
        .map(|i| i["title"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
// verifies: REQ-009
fn registers_open_items_with_their_actions() {
    let dir = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    project(dir.path(), "Demo");
    let (_host, register) = Host::start(options(dir.path(), state.path()));
    assert_eq!(register["id"], "kiln");
    assert_eq!(register["prefix"], "k");
    assert_eq!(register["dynamic"], true);
    let items = register["items"].as_array().unwrap();
    assert_eq!(
        titles(items),
        [
            "REQ-001 Fast start",
            "REQ-002 Cold start",
            "TEST-001 Timing"
        ],
        "closed REQ-003 is behind `all`"
    );
    let req1 = &items[0];
    assert_eq!(req1["id"], "kiln:Demo:REQ-001");
    assert_eq!(req1["subtitle"], "requirement · approved");
    assert_eq!(req1["keywords"], json!(["FLINT-88"]));
    assert_eq!(items[1]["subtitle"], "requirement · draft · under REQ-001");
    let actions: Vec<&str> = req1["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        actions,
        [
            "open-in-fenix",
            "copy-id",
            "copy-reference",
            "status",
            "impact",
            "open-jira",
            "clock-in"
        ]
    );
    let open = &req1["actions"][0];
    assert_eq!(open["kind"], "fenix");
    assert_eq!(open["method"], "fenix/open");
    let path = open["params"]["paths"][0]["path"].as_str().unwrap();
    assert!(
        Path::new(path).ends_with("design/requirements/REQ-001-fast-start.md"),
        "{path}"
    );
    assert_eq!(open["params"]["paths"][0]["line"], 4);
    assert_eq!(req1["actions"][2]["text"], "[[REQ-001]]");
    assert_eq!(req1["actions"][3]["kind"], "query");
    assert_eq!(req1["actions"][3]["text"], "k status REQ-001 ");
    assert_eq!(
        req1["actions"][5]["target"],
        "https://jira.example.com/browse/FLINT-88"
    );
}

#[test]
fn drilldowns_answer_and_other_queries_get_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    project(dir.path(), "Demo");
    let (mut host, _) = Host::start(options(dir.path(), state.path()));
    let status = host.query(1, "status REQ-002 ", json!({}));
    assert_eq!(
        titles(&status)[..3],
        [
            "status REQ-002 → draft",
            "status REQ-002 → proposed",
            "status REQ-002 → approved"
        ]
    );
    assert_eq!(status[0]["subtitle"], "Cold start · current status");
    assert_eq!(status[0]["id"], "kiln:Demo:REQ-002:status:draft");

    let impact = host.query(2, "impact REQ-001", json!({}));
    let mut t = titles(&impact);
    t.sort();
    assert_eq!(
        t,
        [
            "impact REQ-001 ← REQ-002 Cold start",
            "impact REQ-001 ← TEST-001 Timing",
            "impact REQ-001 ← src/lib.rs:2"
        ]
    );

    let all = host.query(3, "all dropped", json!({}));
    assert_eq!(titles(&all), ["all REQ-003 Dropped idea"]);
    assert!(host.query(4, "fast", json!({})).is_empty());
    assert!(host.query(5, "status REQ-099", json!({})).is_empty());
}

#[test]
fn setting_a_status_writes_the_file_and_updates_the_items() {
    let dir = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    project(dir.path(), "Demo");
    let (mut host, _) = Host::start(options(dir.path(), state.path()));
    let done = host
        .execute("kiln:Demo:REQ-001:status:verified", "set-status")
        .unwrap();
    assert_eq!(done["message"], "REQ-001: approved → verified");
    let text =
        std::fs::read_to_string(dir.path().join("design/requirements/REQ-001-fast-start.md"))
            .unwrap();
    assert_eq!(
        text,
        REQ_001.replace("approved", "verified"),
        "the file's CRLF line ends are kept"
    );
    let items = host.notification("provider/setItems");
    assert_eq!(items["items"][0]["subtitle"], "requirement · verified");

    let err = host
        .execute("kiln:Demo:REQ-001:status:nonsense", "set-status")
        .unwrap_err();
    assert_eq!(err, "`nonsense` isn't a status of requirement");
    let err = host.execute("kiln:Other:REQ-001", "clock-in").unwrap_err();
    assert!(err.contains("isn't an item of Demo"), "{err}");
}

#[test]
fn it_follows_fenix_to_another_project_and_notices_changes() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    project(a.path(), "Alpha");
    project(b.path(), "Beta");
    let mut opts = options(a.path(), state.path());
    opts.pinned = None;
    opts.start = Some(a.path().join("design"));
    let (mut host, register) = Host::start(opts);
    assert_eq!(register["items"][0]["id"], "kiln:Alpha:REQ-001");
    let context = json!({ "fenix": { "project": b.path().to_string_lossy(), "file": null } });
    host.query(1, "x", context);
    let items = host.notification("provider/setItems");
    assert_eq!(items["items"][0]["id"], "kiln:Beta:REQ-001");
    let remembered = std::fs::read_to_string(state.path().join("flint-project")).unwrap();
    assert_eq!(Path::new(&remembered), b.path());

    // Someone edits a title outside Flint: the items follow.
    std::fs::write(
        b.path().join("design/requirements/REQ-002-cold-start.md"),
        REQ_002.replace("Cold start", "Colder start"),
    )
    .unwrap();
    let items = host.notification("provider/setItems");
    assert_eq!(items["items"][1]["title"], "REQ-002 Colder start");
}

#[test]
fn without_a_project_it_registers_nothing() {
    let state = tempfile::tempdir().unwrap();
    let nowhere = tempfile::tempdir().unwrap();
    let opts = Options {
        name: "kiln".into(),
        prefix: "k".into(),
        pinned: None,
        start: Some(nowhere.path().to_path_buf()),
        state: Some(state.path().to_path_buf()),
    };
    let (mut host, register) = Host::start(opts);
    assert_eq!(register["items"], json!([]));
    assert!(host.query(1, "status REQ-001", json!({})).is_empty());
}

#[test]
fn confluence_pages_come_from_the_page_cache() {
    let cache = json!({ "pages": {
        "10": { "version": 3, "property": { "schema": 1, "project": "Demo", "export": "spec", "key": "doc", "items": { "REQ-001": "c1", "REQ-002": "c1" } } },
        "11": { "version": 2, "property": { "schema": 1, "project": "Demo", "export": "spec", "key": "item:REQ-002", "items": { "REQ-002": "c1" } } },
    } })
    .to_string();
    let id = |s: &str| s.parse().unwrap();
    assert_eq!(
        crate::confluence::from_cache(&cache, "https://c.example.com/", &id("REQ-002")).as_deref(),
        Some("https://c.example.com/pages/viewpage.action?pageId=11#REQ-002"),
        "an item's own page first"
    );
    assert_eq!(
        crate::confluence::from_cache(&cache, "https://c.example.com", &id("REQ-001")).as_deref(),
        Some("https://c.example.com/pages/viewpage.action?pageId=10#REQ-001")
    );
    assert_eq!(
        crate::confluence::from_cache(&cache, "https://c.example.com", &id("REQ-009")),
        None
    );
}
