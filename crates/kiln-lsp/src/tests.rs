//! The server driven through `lsp-server`'s in-memory connection, over a
//! small project on disk (DESIGN.md, "Testing"): what a client sends and
//! what comes back, message by message.

use std::path::Path;
use std::thread::JoinHandle;
use std::time::Duration;

use kiln_model::Date;
use lsp_server::{Connection, Message, Notification, Request, RequestId};
use serde_json::{json, Value};

use crate::uri;

fn today() -> Date {
    Date::parse("2026-10-13").unwrap()
}

const REQ_001: &str = "+++\nid = \"REQ-001\"\ntype = \"requirement\"\ntitle = \"Fast start\"\nstatus = \"approved\"\n+++\n\nStarts in a second.\n";
const REQ_002: &str = "+++\nid = \"REQ-002\"\ntype = \"requirement\"\ntitle = \"Cold start\"\nstatus = \"draft\"\nparent = \"REQ-001\"\n+++\n\nSee [[REQ-001]].\n";
const TEST_001: &str = "+++\nid = \"TEST-001\"\ntype = \"test\"\ntitle = \"Timing\"\nstatus = \"ready\"\n\n[links]\nverifies = [\"REQ-001\"]\n+++\n\nTimes REQ-002 too.\n";

/// A project: two requirements (one the other's child), a test, a code
/// reference.
fn project(dir: &Path) {
    let config = kiln_model::default_config("Demo")
        .replace("# [code]\n# paths = [\"src\"]", "[code]\npaths = [\"src\"]");
    let files = [
        ("kiln.toml", config.as_str()),
        ("design/requirements/REQ-001-fast-start.md", REQ_001),
        ("design/requirements/REQ-002-cold-start.md", REQ_002),
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
        format!("fn main() {{}}\n// {}: REQ-002\n", "implements"),
    )
    .unwrap();
}

struct Client {
    conn: Connection,
    next: i32,
    server: Option<JoinHandle<Result<(), String>>>,
    /// Notifications received while waiting for responses.
    notifications: Vec<Notification>,
    root: std::path::PathBuf,
}

impl Client {
    fn start(root: &Path, resource_operations: &[&str]) -> Client {
        let (server, conn) = Connection::memory();
        let handle = std::thread::spawn(move || crate::serve(server, today));
        let mut client = Client {
            conn,
            next: 1,
            server: Some(handle),
            notifications: Vec::new(),
            root: root.to_path_buf(),
        };
        let root_uri = uri::from_path(root).unwrap();
        let result = client.request(
            "initialize",
            json!({
                "processId": null,
                "rootUri": root_uri.as_str(),
                "capabilities": { "workspace": { "workspaceEdit": {
                    "documentChanges": true,
                    "resourceOperations": resource_operations,
                } } },
            }),
        );
        assert_eq!(result["serverInfo"]["name"], "kiln");
        client.notify("initialized", json!({}));
        client
    }

    fn uri(&self, rel: &str) -> String {
        uri::from_path(&self.root.join(rel))
            .unwrap()
            .as_str()
            .to_owned()
    }

    fn notify(&self, method: &str, params: Value) {
        self.conn
            .sender
            .send(Message::Notification(Notification::new(
                method.into(),
                params,
            )))
            .unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = RequestId::from(self.next);
        self.next += 1;
        self.conn
            .sender
            .send(Message::Request(Request::new(
                id.clone(),
                method.into(),
                params,
            )))
            .unwrap();
        loop {
            match self
                .conn
                .receiver
                .recv_timeout(Duration::from_secs(20))
                .expect("a response")
            {
                Message::Response(r) if r.id == id => {
                    if let Some(e) = r.error {
                        return json!({ "error": e.message });
                    }
                    return r.result.unwrap_or(Value::Null);
                }
                Message::Notification(n) => self.notifications.push(n),
                _ => {}
            }
        }
    }

    /// Opens `rel` with `text` (default: what is on disk).
    fn open(&self, rel: &str, text: Option<&str>) {
        let text = text
            .map(str::to_owned)
            .unwrap_or_else(|| std::fs::read_to_string(self.root.join(rel)).unwrap());
        self.notify(
            "textDocument/didOpen",
            json!({ "textDocument": { "uri": self.uri(rel), "languageId": "markdown", "version": 1, "text": text } }),
        );
    }

    fn change(&self, rel: &str, text: &str) {
        self.notify(
            "textDocument/didChange",
            json!({ "textDocument": { "uri": self.uri(rel), "version": 2 }, "contentChanges": [{ "text": text }] }),
        );
    }

    /// The next diagnostics published for `rel` (waiting for them).
    fn diagnostics(&mut self, rel: &str) -> Vec<Value> {
        let uri = self.uri(rel);
        let take = |n: &Notification| {
            n.method == "textDocument/publishDiagnostics" && n.params["uri"] == uri.as_str()
        };
        loop {
            if let Some(i) = self.notifications.iter().position(take) {
                let n = self.notifications.remove(i);
                return n.params["diagnostics"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
            }
            match self
                .conn
                .receiver
                .recv_timeout(Duration::from_secs(20))
                .expect("diagnostics")
            {
                Message::Notification(n) => self.notifications.push(n),
                other => panic!("unexpected {other:?}"),
            }
        }
    }

    fn at(&self, rel: &str, line: u32, character: u32) -> Value {
        json!({ "textDocument": { "uri": self.uri(rel) }, "position": { "line": line, "character": character } })
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.request("shutdown", Value::Null);
        self.notify("exit", Value::Null);
        if let Some(h) = self.server.take() {
            h.join().unwrap().unwrap();
        }
    }
}

fn codes(diagnostics: &[Value]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|d| {
            format!(
                "{} {}:{}",
                d["code"].as_str().unwrap_or(""),
                d["range"]["start"]["line"],
                d["range"]["start"]["character"]
            )
        })
        .collect()
}

const REQ_002_PATH: &str = "design/requirements/REQ-002-cold-start.md";

#[test]
// verifies: REQ-008
fn diagnostics_follow_each_edit() {
    let dir = tempfile::tempdir().unwrap();
    project(dir.path());
    let mut c = Client::start(dir.path(), &[]);
    c.open(REQ_002_PATH, None);
    assert_eq!(codes(&c.diagnostics(REQ_002_PATH)), Vec::<String>::new());
    // A parent that doesn't exist, then a status the type doesn't have.
    c.change(
        REQ_002_PATH,
        &REQ_002.replace("parent = \"REQ-001\"", "parent = \"REQ-009\""),
    );
    assert_eq!(codes(&c.diagnostics(REQ_002_PATH)), ["K102 5:10"]);
    c.change(REQ_002_PATH, &REQ_002.replace("\"draft\"", "\"sketch\""));
    let d = c.diagnostics(REQ_002_PATH);
    let k107 = d.iter().find(|d| d["code"] == "K107").unwrap();
    assert_eq!(
        k107["range"],
        json!({ "start": { "line": 4, "character": 9 }, "end": { "line": 4, "character": 17 } }),
        "the quoted value"
    );
    assert_eq!(k107["severity"], 1);
    assert_eq!(k107["source"], "kiln");
    // Fixed: the diagnostics clear.
    c.change(REQ_002_PATH, REQ_002);
    assert!(c.diagnostics(REQ_002_PATH).is_empty());
}

#[test]
fn completion_offers_what_each_place_allows() {
    let dir = tempfile::tempdir().unwrap();
    project(dir.path());
    let mut c = Client::start(dir.path(), &[]);
    let labels = |v: &Value| -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|i| i["label"].as_str().unwrap().to_owned())
            .collect()
    };

    // `parent = "` in a requirement: other requirements.
    let text = REQ_002.replace("parent = \"REQ-001\"", "parent = \"RE");
    c.open(REQ_002_PATH, Some(&text));
    let got = c.request("textDocument/completion", c.at(REQ_002_PATH, 5, 12));
    assert_eq!(labels(&got), ["REQ-001"]);
    assert_eq!(got[0]["detail"], "Fast start (approved)");
    assert_eq!(got[0]["textEdit"]["range"]["start"]["character"], 10);

    // `status = "`: the type's statuses.
    let text = REQ_002.replace("status = \"draft\"", "status = \"");
    c.change(REQ_002_PATH, &text);
    let got = c.request("textDocument/completion", c.at(REQ_002_PATH, 4, 10));
    assert_eq!(
        labels(&got),
        [
            "draft",
            "proposed",
            "approved",
            "implemented",
            "verified",
            "obsolete"
        ]
    );

    // `[[` in the body: every ID but its own, closing the brackets.
    let text = format!("{REQ_002}More [[TE");
    c.change(REQ_002_PATH, &text);
    let got = c.request("textDocument/completion", c.at(REQ_002_PATH, 9, 9));
    assert_eq!(labels(&got), ["REQ-001", "TEST-001"]);
    assert_eq!(got[1]["textEdit"]["newText"], "TEST-001]]");

    // `[links]`: roles a test may use, then requirements not yet listed.
    let path = "design/tests/TEST-001-timing.md";
    let text = TEST_001.replace(
        "verifies = [\"REQ-001\"]",
        "verifies = [\"REQ-001\", \"\"]\nve",
    );
    c.open(path, Some(&text));
    let roles = c.request("textDocument/completion", c.at(path, 8, 2));
    assert_eq!(labels(&roles), ["verifies"]);
    let ids = c.request("textDocument/completion", c.at(path, 7, 24));
    assert_eq!(labels(&ids), ["REQ-002"]);
}

#[test]
fn hover_definition_and_references() {
    let dir = tempfile::tempdir().unwrap();
    project(dir.path());
    let mut c = Client::start(dir.path(), &[]);
    c.open(REQ_002_PATH, None);
    // On `REQ-001` in `parent = "REQ-001"`.
    let hover = c.request("textDocument/hover", c.at(REQ_002_PATH, 5, 12));
    let text = hover["contents"]["value"].as_str().unwrap();
    assert!(text.starts_with("REQ-001 Fast start\nrequirement · approved · design/requirements/REQ-001-fast-start.md:2\n"), "{text}");
    assert!(
        text.contains("children") && text.contains("REQ-002 Cold start"),
        "{text}"
    );
    assert!(
        text.contains("verified by") && text.contains("TEST-001 Timing"),
        "{text}"
    );

    let def = c.request("textDocument/definition", c.at(REQ_002_PATH, 5, 12));
    assert_eq!(
        def[0]["uri"],
        c.uri("design/requirements/REQ-001-fast-start.md")
    );
    assert_eq!(
        def[0]["range"]["start"],
        json!({ "line": 1, "character": 6 })
    );

    // References to REQ-002 from its own body: a mention in TEST-001,
    // the code reference, and (asked for) its own `id`.
    let mut params = c.at(REQ_002_PATH, 1, 8);
    params["context"] = json!({ "includeDeclaration": true });
    let refs = c.request("textDocument/references", params);
    let mut places: Vec<String> = refs
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            format!(
                "{} {}",
                l["uri"].as_str().unwrap().rsplit('/').next().unwrap(),
                l["range"]["start"]["line"]
            )
        })
        .collect();
    places.sort();
    assert_eq!(
        places,
        [
            "REQ-002-cold-start.md 1",
            "TEST-001-timing.md 10",
            "lib.rs 1"
        ]
    );

    let missing = c.request(
        "textDocument/hover",
        c.at("design/tests/TEST-001-timing.md", 0, 0),
    );
    assert_eq!(missing, Value::Null);
}

#[test]
fn rename_rewrites_every_reference_without_touching_files() {
    let dir = tempfile::tempdir().unwrap();
    project(dir.path());
    let mut c = Client::start(dir.path(), &[]);
    c.open("design/requirements/REQ-001-fast-start.md", None);
    let mut params = c.at("design/requirements/REQ-001-fast-start.md", 1, 7);
    params["newName"] = json!("REQ-010");
    let edit = c.request("textDocument/rename", params.clone());
    let changes = edit["changes"].as_object().unwrap();
    let mut files: Vec<(&str, usize)> = changes
        .iter()
        .map(|(u, e)| (u.rsplit('/').next().unwrap(), e.as_array().unwrap().len()))
        .collect();
    files.sort();
    assert_eq!(
        files,
        [
            ("REQ-001-fast-start.md", 1),
            ("REQ-002-cold-start.md", 2),
            ("TEST-001-timing.md", 1)
        ]
    );
    assert!(
        edit.get("documentChanges").is_none(),
        "no file rename for a client without them"
    );
    // Nothing was written: the edit is the client's to apply.
    assert_eq!(
        std::fs::read_to_string(dir.path().join(REQ_002_PATH)).unwrap(),
        REQ_002
    );

    params["newName"] = json!("REQ-2");
    assert!(c.request("textDocument/rename", params.clone())["error"]
        .as_str()
        .unwrap()
        .contains("REQ-2"));
    params["newName"] = json!("TEST-005");
    assert!(c.request("textDocument/rename", params)["error"]
        .as_str()
        .unwrap()
        .contains("prefix"));
}

#[test]
fn rename_moves_the_file_when_the_client_can() {
    let dir = tempfile::tempdir().unwrap();
    project(dir.path());
    let mut c = Client::start(dir.path(), &["create", "rename"]);
    let mut params = c.at("design/requirements/REQ-001-fast-start.md", 1, 7);
    params["newName"] = json!("REQ-010");
    let edit = c.request("textDocument/rename", params);
    let ops = edit["documentChanges"].as_array().unwrap();
    let last = ops.last().unwrap();
    assert_eq!(last["kind"], "rename");
    assert!(last["newUri"]
        .as_str()
        .unwrap()
        .ends_with("/REQ-010-fast-start.md"));
}

#[test]
fn code_actions_and_formatting() {
    let dir = tempfile::tempdir().unwrap();
    project(dir.path());
    let mut c = Client::start(dir.path(), &[]);
    let path = "design/tests/TEST-001-timing.md";
    c.open(path, None);
    let titles = |v: &Value| -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|a| {
                format!(
                    "{}{}",
                    a["title"].as_str().unwrap(),
                    if a.get("disabled").is_some() {
                        " (disabled)"
                    } else {
                        ""
                    }
                )
            })
            .collect()
    };
    // On REQ-002 in the body: link it.
    let mut params =
        json!({ "textDocument": { "uri": c.uri(path) }, "context": { "diagnostics": [] } });
    params["range"] =
        json!({ "start": { "line": 10, "character": 8 }, "end": { "line": 10, "character": 8 } });
    let actions = c.request("textDocument/codeAction", params.clone());
    let t = titles(&actions);
    assert_eq!(t[0], "Add `verifies` link to REQ-002");
    assert!(t.contains(&"Set status → passing".to_owned()), "{t:?}");
    let add = &actions[0]["edit"]["changes"][c.uri(path)][0];
    assert_eq!(add["newText"], ", \"REQ-002\"");

    // On a requirement: new items are disabled for a client that can't
    // create files, with the command to run instead.
    let req = "design/requirements/REQ-001-fast-start.md";
    c.open(req, None);
    params["textDocument"]["uri"] = json!(c.uri(req));
    params["range"] =
        json!({ "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } });
    let actions = c.request("textDocument/codeAction", params.clone());
    let t = titles(&actions);
    assert!(
        t.contains(&"New child requirement (disabled)".to_owned()),
        "{t:?}"
    );
    assert!(
        t.contains(&"Create test that verifies REQ-001 (disabled)".to_owned()),
        "{t:?}"
    );
    let child = actions
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["title"] == "New child requirement")
        .unwrap();
    assert!(child["disabled"]["reason"]
        .as_str()
        .unwrap()
        .contains("kiln new requirement --parent REQ-001 --open"));

    // Formatting puts keys in order and quotes strings.
    c.change(
        req,
        &REQ_001
            .replace("status = \"approved\"\n", "")
            .replace("+++\nid", "+++\nstatus = 'approved'\nid"),
    );
    let edits = c.request("textDocument/formatting", json!({ "textDocument": { "uri": c.uri(req) }, "options": { "tabSize": 4, "insertSpaces": true } }));
    let mut text = REQ_001
        .replace("status = \"approved\"\n", "")
        .replace("+++\nid", "+++\nstatus = 'approved'\nid");
    for e in edits.as_array().unwrap().iter().rev() {
        let doc = crate::text::Doc::new(&text);
        let start = doc.offset(serde_json::from_value(e["range"]["start"].clone()).unwrap());
        let end = doc.offset(serde_json::from_value(e["range"]["end"].clone()).unwrap());
        text.replace_range(start..end, e["newText"].as_str().unwrap());
    }
    assert_eq!(text, REQ_001);
}

#[test]
fn creating_items_when_the_client_can() {
    let dir = tempfile::tempdir().unwrap();
    project(dir.path());
    let mut c = Client::start(dir.path(), &["create", "rename"]);
    let req = "design/requirements/REQ-001-fast-start.md";
    c.open(req, None);
    let params = json!({
        "textDocument": { "uri": c.uri(req) },
        "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
        "context": { "diagnostics": [] },
    });
    let actions = c.request("textDocument/codeAction", params);
    let create = actions
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["title"] == "Create test that verifies REQ-001")
        .unwrap();
    let ops = create["edit"]["documentChanges"].as_array().unwrap();
    assert_eq!(ops[0]["kind"], "create");
    assert!(ops[0]["uri"]
        .as_str()
        .unwrap()
        .ends_with("/design/tests/TEST-002-untitled-test.md"));
    let text = ops[1]["edits"][0]["newText"].as_str().unwrap();
    assert!(
        text.contains("id = \"TEST-002\"") && text.contains("verifies = [\"REQ-001\"]"),
        "{text}"
    );
}

#[test]
fn files_changed_on_disk_are_noticed() {
    let dir = tempfile::tempdir().unwrap();
    project(dir.path());
    let mut c = Client::start(dir.path(), &[]);
    c.open(REQ_002_PATH, None);
    assert!(c.diagnostics(REQ_002_PATH).is_empty());
    // REQ-001 disappears outside the editor: REQ-002's parent dangles.
    std::thread::sleep(Duration::from_millis(2500));
    std::fs::remove_file(dir.path().join("design/requirements/REQ-001-fast-start.md")).unwrap();
    assert_eq!(
        codes(&c.diagnostics(REQ_002_PATH)),
        ["K102 5:10", "K102 8:6"]
    );
}

#[test]
fn markdown_outside_the_project_is_harmless() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("README.md"), "# Hello [[REQ-001]]\n").unwrap();
    let mut c = Client::start(dir.path(), &[]);
    c.open("README.md", None);
    assert_eq!(
        c.request("textDocument/hover", c.at("README.md", 0, 12)),
        Value::Null
    );
    assert_eq!(
        c.request("textDocument/completion", c.at("README.md", 0, 10)),
        Value::Null
    );
    assert_eq!(c.request("textDocument/formatting", json!({ "textDocument": { "uri": c.uri("README.md") }, "options": { "tabSize": 4, "insertSpaces": true } })), Value::Null);
}

#[test]
// verifies: REQ-008
fn code_files_beside_their_own_language_server() {
    // As Fenix runs it as an additional server on a `.rs` buffer.
    let dir = tempfile::tempdir().unwrap();
    project(dir.path());
    let mut c = Client::start(dir.path(), &[]);
    let labels = |v: &Value| -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|i| i["label"].as_str().unwrap().to_owned())
            .collect()
    };
    let code = "src/lib.rs";
    let role = "implements";

    // After the role word: the IDs that role may point at (requirements),
    // without those already listed; a finished list offers nothing.
    let text = format!("fn main() {{}}\n// {role}: REQ-001, RE\n");
    c.open(code, Some(&text));
    let got = c.request("textDocument/completion", c.at(code, 1, 26));
    assert_eq!(labels(&got), ["REQ-002"]);
    assert_eq!(got[0]["textEdit"]["range"]["start"]["character"], 24);
    c.change(code, &format!("fn main() {{}}\n// {role}: \n"));
    let got = c.request("textDocument/completion", c.at(code, 1, 15));
    assert_eq!(labels(&got), ["REQ-001", "REQ-002"]);
    c.change(code, &format!("fn main() {{}}\n// {role}: REQ-001 \n"));
    assert_eq!(
        labels(&c.request("textDocument/completion", c.at(code, 1, 23))),
        Vec::<String>::new()
    );
    // Ordinary code completes nothing: that is the primary server's.
    c.change(code, "fn main() { let x = REQ }\n");
    assert_eq!(
        labels(&c.request("textDocument/completion", c.at(code, 0, 23))),
        Vec::<String>::new()
    );

    // A code reference to an ID that doesn't exist is a diagnostic on the
    // buffer, from the editor's text, not the disk's.
    c.change(code, &format!("fn main() {{}}\n// {role}: REQ-099\n"));
    let found = loop {
        let d = c.diagnostics(code);
        if !d.is_empty() {
            break d;
        }
    };
    assert_eq!(codes(&found), ["K102 1:15"]);

    // Hover and definition on an ID in a comment.
    c.change(code, &format!("fn main() {{}}\n// {role}: REQ-001\n"));
    let hover = c.request("textDocument/hover", c.at(code, 1, 18));
    let shown = hover["contents"]["value"].as_str().unwrap();
    assert!(shown.starts_with("REQ-001 Fast start"), "{shown}");
    let def = c.request("textDocument/definition", c.at(code, 1, 18));
    assert!(def.to_string().contains("REQ-001-fast-start.md"), "{def}");

    // Formatting is the code's own server's: nothing from Kiln.
    let fmt = c.request(
        "textDocument/formatting",
        json!({ "textDocument": { "uri": c.uri(code) }, "options": { "tabSize": 4, "insertSpaces": true } }),
    );
    assert_eq!(fmt, Value::Null);
}
