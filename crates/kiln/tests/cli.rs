//! The binary's contract as a process sees it: output streams, exit codes
//! and files on disk (DESIGN.md, "Command line"; docs/CLI.md). Spawns the
//! built `kiln`, so it also proves the binary runs headless.

use std::path::Path;
use std::process::{Command, Output};

fn kiln_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(args)
        .current_dir(dir)
        // A Fenix on this machine must not receive test files.
        .env("FENIX_HOME", dir.join("no-fenix"))
        .output()
        .expect("the kiln binary runs")
}

fn kiln(args: &[&str]) -> Output {
    kiln_in(&std::env::temp_dir(), args)
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).expect("kiln writes UTF-8")
}

fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).expect("kiln writes UTF-8")
}

/// Runs and asserts the exit code, returning stdout.
fn ok(dir: &Path, args: &[&str]) -> String {
    let out = kiln_in(dir, args);
    assert_eq!(
        out.status.code(),
        Some(0),
        "kiln {args:?}: {}",
        stderr(&out)
    );
    stdout(&out)
}

#[test]
fn version_prints_name_and_version_and_exits_0() {
    let out = kiln(&["--version"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        stdout(&out).trim_end(),
        format!("kiln {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn the_gui_asked_about_a_project_outside_one_is_one_line_and_exits_2() {
    // `kiln` alone outside a project opens Home (GUI.md, "Home"), so no
    // test runs it bare: CI may have a desktop to open it on. What is
    // about a project still stops before any window when there is none.
    let empty = tempfile::tempdir().unwrap();
    for args in [
        vec!["gui", "--item", "REQ-7"],
        vec!["gui", "--since", "v1.0"],
    ] {
        let out = kiln_in(empty.path(), &args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let err = stderr(&out);
        assert!(err.starts_with("kiln: no kiln.toml"), "{err}");
        assert_eq!(err.lines().count(), 1);
    }
    let elsewhere = empty.path().to_str().unwrap();
    let out = kiln_in(empty.path(), &["--project", elsewhere]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).starts_with("kiln: no kiln.toml"));
}

#[test]
fn unknown_command_is_one_line_on_stderr_and_exits_2() {
    let out = kiln(&["frobnicate"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let err = stderr(&out);
    assert_eq!(err.lines().count(), 1, "{err:?}");
    assert!(
        err.starts_with("kiln: ") && err.contains("frobnicate"),
        "{err:?}"
    );
}

#[test]
fn outside_a_project_is_a_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    let out = kiln_in(dir.path(), &["list"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("no kiln.toml"), "{}", stderr(&out));
}

#[test]
fn a_project_from_init_to_check() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--preset", "none", "--name", "Demo"]);
    assert_eq!(
        kiln_in(d, &["init", "--preset", "none"]).status.code(),
        Some(2),
        "init never overwrites"
    );
    assert!(d.join("design/README.md").is_file());

    assert_eq!(
        ok(d, &["new", "requirement", "--title", "Fast enough"]).trim(),
        "design/requirements/REQ-001-fast-enough.md"
    );
    assert_eq!(
        ok(d, &["new", "requirement", "--parent", "REQ-001"]).trim(),
        "design/requirements/REQ-002-untitled-requirement.md"
    );
    assert_eq!(
        ok(d, &["new", "test", "--title", "Benchmark"]).trim(),
        "design/tests/TEST-001-benchmark.md"
    );
    let refused = kiln_in(d, &["new", "requirement", "--parent", "TEST-001"]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(stderr(&refused).contains("can't have a test as parent"));

    ok(d, &["set", "REQ-001", "status=approved"]);
    let check = kiln_in(d, &["check"]);
    assert_eq!(check.status.code(), Some(0), "warnings don't fail");
    assert!(stdout(&check).contains("K201 warning: REQ-001 is approved but nothing verifies it"));
    // Approved with the template's prompts still in it (K17).
    assert!(stdout(&check).contains("K215 warning: REQ-001 is approved but still has"));
    assert_eq!(stderr(&check).trim(), "kiln: 3 warnings");

    ok(d, &["link", "TEST-001", "verifies", "REQ-001"]);
    let wrong = kiln_in(d, &["link", "REQ-001", "verifies", "REQ-002"]);
    assert_eq!(wrong.status.code(), Some(2));
    assert!(stderr(&wrong).contains("a requirement can't be the source of `verifies`"));

    let list = ok(d, &["list", "--type", "requirement"]);
    assert_eq!(
        list,
        "REQ-001  approved  Fast enough\nREQ-002  draft     Untitled requirement\n"
    );
    let json: serde_json::Value =
        serde_json::from_str(&ok(d, &["show", "REQ-001", "--format", "json"])).unwrap();
    assert_eq!(json["inbound"]["verifies"][0]["id"], "TEST-001");
    assert_eq!(json["inbound"]["verifies"][0]["title"], "Benchmark");
    assert_eq!(json["children"][0]["id"], "REQ-002");
    assert_eq!(json["coverage"]["verifies"], true);
    let impact = ok(d, &["impact", "REQ-001"]);
    assert!(
        impact.contains("TEST-001  verifies REQ-001 — TEST-001 Benchmark"),
        "{impact}"
    );
    let trace = ok(d, &["trace", "--format", "csv"]);
    assert!(
        trace.contains("REQ-001,Fast enough,approved,,TEST-001,implements"),
        "{trace}"
    );

    // An error fails the check: a dangling reference.
    let test = d.join("design/tests/TEST-001-benchmark.md");
    let text = std::fs::read_to_string(&test).unwrap();
    std::fs::write(&test, format!("{text}\nSee [[REQ-099]].\n")).unwrap();
    let check = kiln_in(d, &["check", "--format", "github"]);
    assert_eq!(check.status.code(), Some(1));
    assert!(stdout(&check).contains("::error file=design/tests/TEST-001-benchmark.md,line=17,col=7,title=K102::reference REQ-099 doesn't exist"), "{}", stdout(&check));
}

#[test]
// verifies: REQ-018
fn trace_levels_follows_requirements_down_the_levels() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--preset", "none", "--name", "Demo"]);
    let one = kiln_in(d, &["trace", "--levels"]);
    assert_eq!(one.status.code(), Some(2));
    assert!(
        stderr(&one).contains("one requirement level"),
        "{}",
        stderr(&one)
    );

    let config = d.join("kiln.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        &config,
        format!(
            "{text}
[types.system]
prefix = \"SYS\"
title = \"System requirement\"
statuses = [\"draft\", \"approved\"]

[links.refines]
from = [\"system\"]
to = [\"requirement\"]
inverse = \"refined by\"

[requirements]
levels = [\"requirement\", \"system\"]
"
        ),
    )
    .unwrap();
    ok(d, &["new", "requirement", "--title", "Faults are visible"]);
    ok(d, &["new", "system", "--title", "Fault telemetry"]);
    ok(d, &["new", "system", "--title", "Orphan"]);
    ok(d, &["link", "SYS-001", "refines", "REQ-001"]);
    ok(d, &["set", "SYS-002", "status=approved"]);
    let check = ok(d, &["check"]);
    assert!(
        check.contains("K208 warning: SYS-002 is approved but refines no requirement"),
        "{check}"
    );

    let text = ok(d, &["trace", "--levels"]);
    assert!(text.starts_with("Requirement levels"), "{text}");
    assert!(
        text.contains("REQ-001  Faults are visible  (draft)\n"),
        "{text}"
    );
    assert!(
        text.contains("\n  SYS-001  Fault telemetry  (draft)\n"),
        "{text}"
    );
    assert!(
        text.contains("Not traced to the level above\n  SYS-002  Orphan  (approved)"),
        "{text}"
    );
    let csv = ok(d, &["trace", "--levels", "--format", "csv"]);
    assert!(csv.starts_with("Requirement,System requirement,"), "{csv}");
    assert!(csv.contains("\nREQ-001,SYS-001,"), "{csv}");
    assert!(csv.contains("\n,SYS-002,"), "{csv}");
    let json: serde_json::Value =
        serde_json::from_str(&ok(d, &["trace", "--levels", "--format", "json"])).unwrap();
    assert_eq!(json["chains"][0]["below"][0]["id"], "SYS-001");
    assert_eq!(json["untraced"][0]["id"], "SYS-002");
}

#[test]
// verifies: REQ-015
fn renumber_moves_an_id_everywhere() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--preset", "none"]);
    std::fs::write(
        d.join("kiln.toml"),
        format!(
            "{}\n[code]\npaths = [\"src\"]\n",
            std::fs::read_to_string(d.join("kiln.toml")).unwrap()
        ),
    )
    .unwrap();
    ok(d, &["new", "requirement", "--title", "A"]);
    ok(d, &["new", "test", "--title", "T"]);
    ok(d, &["link", "TEST-001", "verifies", "REQ-001"]);
    std::fs::create_dir_all(d.join("src")).unwrap();
    std::fs::write(
        d.join("src/lib.rs"),
        format!("// {}: REQ-001\r\n", "verifies"),
    )
    .unwrap();

    let out = ok(d, &["renumber", "REQ-001", "--to", "REQ-005"]);
    assert_eq!(out.trim(), "REQ-001 → REQ-005: 2 references in 2 other files; design/requirements/REQ-001-a.md → design/requirements/REQ-005-a.md");
    assert!(
        std::fs::read_to_string(d.join("design/tests/TEST-001-t.md"))
            .unwrap()
            .contains("verifies = [\"REQ-005\"]")
    );
    assert_eq!(
        std::fs::read(d.join("src/lib.rs")).unwrap(),
        format!("// {}: REQ-005\r\n", "verifies").as_bytes()
    );
    assert_eq!(kiln_in(d, &["check"]).status.code(), Some(0));
}

#[test]
fn export_writes_markdown_and_check_notices_staleness() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--preset", "none", "--name", "Demo"]);
    let config = std::fs::read_to_string(d.join("kiln.toml")).unwrap();
    std::fs::write(
        d.join("kiln.toml"),
        format!("{config}\n[[export]]\nname = \"doc\"\nformat = \"markdown\"\npath = \"docs/DESIGN.md\"\n\n[[export]]\nname = \"page\"\nformat = \"html\"\nlayout = \"sections\"\npath = \"out/html\"\n\n[[export]]\nname = \"word\"\nformat = \"docx\"\npath = \"out/design.docx\"\n"),
    )
    .unwrap();
    ok(d, &["new", "requirement", "--title", "Fast"]);

    let dry = ok(d, &["export", "--dry-run"]);
    assert!(dry.contains("would create docs/DESIGN.md"), "{dry}");
    assert!(
        !d.join("docs/DESIGN.md").exists(),
        "a dry run writes nothing"
    );
    let check = kiln_in(d, &["export", "--check"]);
    assert_eq!(check.status.code(), Some(1));
    assert!(stderr(&check).contains("exports are out of date"));

    ok(d, &["export"]);
    let md = std::fs::read_to_string(d.join("docs/DESIGN.md")).unwrap();
    assert!(md.starts_with("# Demo\n"), "{md}");
    assert!(md.contains("### 1.1 REQ-001 Fast"), "{md}");
    assert!(d.join("out/html/index.html").is_file() && d.join("out/html/.kiln-export").is_file());
    let word = std::fs::read(d.join("out/design.docx")).unwrap();
    assert!(word.starts_with(b"PK\x03\x04"), "a .docx is a zip");
    // A second process renders the same bytes, Word included.
    assert_eq!(kiln_in(d, &["export", "--check"]).status.code(), Some(0));

    ok(d, &["set", "REQ-001", "status=approved"]);
    assert_eq!(
        kiln_in(d, &["export", "--check", "doc"]).status.code(),
        Some(1)
    );
    let unknown = kiln_in(d, &["export", "nope"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(stderr(&unknown).contains("no export target `nope` (doc, page, word)"));
}

#[test]
fn confluence_targets_plan_from_a_snapshot_and_never_write() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--preset", "none", "--name", "Demo"]);
    let config = std::fs::read_to_string(d.join("kiln.toml")).unwrap();
    std::fs::write(
        d.join("kiln.toml"),
        format!("{config}\n[confluence]\nurl = \"https://confluence.example.com\"\nspace = \"DEMO\"\n\n[[export]]\nname = \"spec\"\nformat = \"confluence\"\ntitle = \"Demo design\"\n[export.confluence]\nparent = 42\n"),
    )
    .unwrap();
    ok(d, &["new", "requirement", "--title", "Fast"]);
    std::fs::write(d.join("snapshot.json"), "{\"anchorPage\": 42}\n").unwrap();

    let refused = kiln_in(d, &["export", "--snapshot", "snapshot.json"]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(stderr(&refused).contains("--snapshot plans offline"));

    let plan = ok(d, &["export", "--dry-run", "--snapshot", "snapshot.json"]);
    assert!(
        plan.contains(
            "spec (confluence, document) → space DEMO, page 42
"
        ),
        "a snapshot without the parent's title names it by ID alone: {plan}"
    );
    std::fs::write(
        d.join("named.json"),
        "{\"anchorPage\": 42, \"anchorTitle\": \"Team space home\"}
",
    )
    .unwrap();
    let named = ok(d, &["export", "--dry-run", "--snapshot", "named.json"]);
    assert!(
        named.contains(
            "spec (confluence, document) → space DEMO, page 42 \"Team space home\"
"
        ),
        "{named}"
    );
    let json = ok(
        d,
        &[
            "export",
            "--dry-run",
            "--snapshot",
            "named.json",
            "--format",
            "json",
        ],
    );
    assert!(
        json.contains("\"parentTitle\": \"Team space home\""),
        "{json}"
    );
    assert!(plan.contains("create    \"Demo design\": new"), "{plan}");
    assert!(
        plan.contains("1 create, 0 update, 0 unchanged, 0 drift, 0 retire"),
        "{plan}"
    );

    let json: serde_json::Value = serde_json::from_str(&ok(
        d,
        &[
            "export",
            "--dry-run",
            "--snapshot",
            "snapshot.json",
            "--format",
            "json",
        ],
    ))
    .unwrap();
    assert_eq!(json[0]["plan"]["ops"][0]["kind"], "create");

    // Without a token or snapshot a dry run says what it needs.
    let out = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(["export", "--dry-run"])
        .current_dir(d)
        .env_remove("KILN_CONFLUENCE_TOKEN")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    // No token stored, or (headless) no credential store to look in.
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("KILN_CONFLUENCE_TOKEN"), "{err}");
    assert!(!err.contains('\n') || err.lines().count() == 1, "{err}");
}

/// `kiln` with a Confluence token and its own state folder.
fn kiln_publishing(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(args)
        .current_dir(dir)
        .env("FENIX_HOME", dir.join("no-fenix"))
        .env("KILN_HOME", dir.join("kiln-home"))
        .env("KILN_CONFLUENCE_TOKEN", "test-token")
        .env_remove("KILN_CONFLUENCE_USER")
        .output()
        .expect("the kiln binary runs")
}

#[test]
// verifies: REQ-002
fn confluence_targets_publish_with_yes_and_stop_on_drift() {
    use kiln_confluence::fake::{Fake, FakePage, State};
    let mut state = State::default();
    state.pages.insert(
        42,
        FakePage {
            title: "Designs".into(),
            parent: None,
            version: 1,
            author: "alice".into(),
            body: String::new(),
            property: None,
        },
    );
    let fake = Fake::start(state, false);
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--preset", "none", "--name", "Demo"]);
    let config = std::fs::read_to_string(d.join("kiln.toml")).unwrap();
    std::fs::write(
        d.join("kiln.toml"),
        format!("{config}\n[confluence]\nurl = \"{}\"\nspace = \"DEMO\"\n\n[[export]]\nname = \"spec\"\nformat = \"confluence\"\ntitle = \"Demo design\"\n[export.confluence]\nparent = 42\n", fake.url),
    )
    .unwrap();
    ok(d, &["new", "requirement", "--title", "Fast"]);

    // Not a terminal and no --yes: the plan, then exit 1, nothing written.
    let asked = kiln_publishing(d, &["export"]);
    assert_eq!(asked.status.code(), Some(1), "{}", stderr(&asked));
    assert!(
        stdout(&asked).contains("create    \"Demo design\": new"),
        "{}",
        stdout(&asked)
    );
    assert!(stderr(&asked).contains("pass --yes"), "{}", stderr(&asked));
    assert_eq!(fake.state.lock().unwrap().writes(), 0);

    let published = kiln_publishing(d, &["export", "--yes"]);
    assert_eq!(published.status.code(), Some(0), "{}", stderr(&published));
    assert!(
        stdout(&published).contains("spec: published 1 created, 0 updated"),
        "{}",
        stdout(&published)
    );
    let id = {
        let s = fake.state.lock().unwrap();
        let (id, page) = s
            .pages
            .iter()
            .find(|(_, p)| p.title == "Demo design")
            .unwrap();
        assert_eq!(page.parent, Some(42));
        assert!(page.body.contains("REQ-001"), "{}", page.body);
        *id
    };
    assert!(d
        .join("kiln-home/state/confluence/DEMO-Demo-spec.json")
        .is_file());

    let writes = fake.state.lock().unwrap().writes();
    let again = kiln_publishing(d, &["export", "--yes"]);
    assert_eq!(again.status.code(), Some(0), "{}", stderr(&again));
    assert!(
        stdout(&again).contains("0 create, 0 update, 1 unchanged"),
        "{}",
        stdout(&again)
    );
    assert_eq!(
        fake.state.lock().unwrap().writes(),
        writes,
        "unchanged costs no write"
    );

    fake.state
        .lock()
        .unwrap()
        .edit(id, "<p>A fix made in Confluence.</p>");
    let drift = kiln_publishing(d, &["export", "--yes"]);
    assert_eq!(drift.status.code(), Some(1));
    assert!(
        stdout(&drift)
            .contains("refused: \"Demo design\" was edited in Confluence (edited by alice"),
        "{}",
        stdout(&drift)
    );
    let forced = kiln_publishing(d, &["export", "--yes", "--force"]);
    assert_eq!(forced.status.code(), Some(0), "{}", stderr(&forced));
    assert!(fake.state.lock().unwrap().pages[&id]
        .body
        .contains("REQ-001"));
    let out = kiln_publishing(d, &["export", "--yes", "--format", "json"]);
    let json: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(
        json[0]["applied"],
        serde_json::Value::Null,
        "nothing to publish"
    );
}

#[test]
fn init_registers_the_language_server_for_fenix() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    std::fs::create_dir_all(d.join(".fenix")).unwrap();
    std::fs::write(
        d.join(".fenix/tools.json"),
        "{\"tasks\": {\"build\": {\"executable\": \"cargo\", \"args\": [\"build\"]}}}\n",
    )
    .unwrap();
    let read = || -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(d.join(".fenix/tools.json")).unwrap())
            .unwrap()
    };
    ok(
        d,
        &["init", "--preset", "none", "--name", "Demo", "--fenix"],
    );
    let tools = read();
    let kiln = &tools["lsp"]["additional"][0];
    assert_eq!(kiln["name"], "kiln");
    assert_eq!(kiln["executable"], "kiln");
    assert_eq!(kiln["args"][0], "lsp");
    assert_eq!(kiln["languages"], serde_json::json!(["markdown"]));
    assert_eq!(
        tools["tasks"]["build"]["executable"], "cargo",
        "the rest is kept"
    );

    // With code to scan, running it again adds the code's languages; the
    // pre-G3 form (Kiln as the Markdown server) moves over, and another
    // Markdown server stays as the primary.
    let config = std::fs::read_to_string(d.join("kiln.toml")).unwrap();
    std::fs::write(
        d.join("kiln.toml"),
        config.replace("# [code]\n# paths = [\"src\"]", "[code]\npaths = [\"src\"]"),
    )
    .unwrap();
    std::fs::create_dir_all(d.join("src")).unwrap();
    std::fs::write(d.join("src/lib.rs"), "fn x() {}\n").unwrap();
    std::fs::write(d.join("src/run.tcl"), "puts x\n").unwrap();
    std::fs::write(
        d.join(".fenix/tools.json"),
        "{\"lsp\": {\"markdown\": {\"executable\": \"kiln\", \"args\": [\"lsp\"]}, \"rust\": {\"executable\": \"rust-analyzer\"}}}\n",
    )
    .unwrap();
    let out = kiln_in(d, &["init", "--preset", "none", "--fenix"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("moved `kiln lsp` from the Markdown server"),
        "{}",
        stderr(&out)
    );
    let tools = read();
    assert!(tools["lsp"].get("markdown").is_none());
    assert_eq!(tools["lsp"]["rust"]["executable"], "rust-analyzer");
    assert_eq!(
        tools["lsp"]["additional"][0]["languages"],
        serde_json::json!(["markdown", "rust", "tcl"])
    );

    // Again: nothing to do. A language added by hand is kept.
    let mut tools = read();
    tools["lsp"]["additional"][0]["languages"]
        .as_array_mut()
        .unwrap()
        .push("python".into());
    std::fs::write(d.join(".fenix/tools.json"), tools.to_string()).unwrap();
    let out = kiln_in(d, &["init", "--preset", "none", "--fenix"]);
    assert!(
        stderr(&out).contains("already runs `kiln lsp` for markdown, rust, tcl, python"),
        "{}",
        stderr(&out)
    );
    assert_eq!(
        kiln_in(d, &["init", "--preset", "none"]).status.code(),
        Some(2),
        "kiln.toml exists"
    );
}

#[test]
fn lsp_speaks_the_protocol_on_stdio() {
    use std::io::{BufRead, BufReader, Read, Write};
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--preset", "none", "--name", "Demo"]);
    let mut child = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .arg("lsp")
        .current_dir(d)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("kiln lsp starts");
    let mut stdin = child.stdin.take().unwrap();
    let mut send = |body: serde_json::Value| {
        let body = body.to_string();
        write!(stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        stdin.flush().unwrap();
    };
    send(
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"capabilities": {}, "rootUri": null}}),
    );
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut length = 0;
    loop {
        let mut line = String::new();
        out.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
        if let Some(n) = line.strip_prefix("Content-Length: ") {
            length = n.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    out.read_exact(&mut body).unwrap();
    let reply: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(reply["result"]["serverInfo"]["name"], "kiln");
    assert!(reply["result"]["capabilities"]["renameProvider"].is_object());
    send(serde_json::json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}));
    send(serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown"}));
    send(serde_json::json!({"jsonrpc": "2.0", "method": "exit"}));
    drop(stdin);
    assert!(child.wait().unwrap().success());
}

#[test]
fn provider_says_hello_and_registers_the_items() {
    use std::io::{BufRead, BufReader, Read, Write};
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--preset", "none", "--name", "Demo"]);
    ok(d, &["new", "requirement", "--title", "Fast"]);
    let mut child = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(["provider", "--project"])
        .arg(d)
        .current_dir(d)
        .env("KILN_HOME", d.join("kiln-home"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("kiln provider starts");
    let mut stdin = child.stdin.take().unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut read = || -> serde_json::Value {
        let mut length = 0;
        loop {
            let mut line = String::new();
            out.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some(n) = line.strip_prefix("Content-Length: ") {
                length = n.trim().parse().unwrap();
            }
        }
        let mut body = vec![0; length];
        out.read_exact(&mut body).unwrap();
        serde_json::from_slice(&body).unwrap()
    };
    let hello = read();
    assert_eq!(hello["method"], "hello");
    assert_eq!(hello["params"]["name"], "kiln");
    let answer = serde_json::json!({"jsonrpc": "2.0", "id": hello["id"], "result": {"protocol": 1, "host": {"name": "flint", "version": "0.1.0", "pid": 1}}}).to_string();
    write!(stdin, "Content-Length: {}\r\n\r\n{answer}", answer.len()).unwrap();
    stdin.flush().unwrap();
    let register = read();
    assert_eq!(register["method"], "provider/register");
    assert_eq!(register["params"]["items"][0]["title"], "REQ-001 Fast");
    // Flint closes stdin when it stops: the provider exits.
    drop(stdin);
    assert!(child.wait().unwrap().success());
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=Kiln",
            "-c",
            "user.email=kiln@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
// verifies: REQ-016
fn diff_and_export_since_a_tag() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    git(d, &["init", "-q"]);
    ok(d, &["init", "--preset", "none", "--name", "Demo"]);
    let config = std::fs::read_to_string(d.join("kiln.toml")).unwrap();
    std::fs::write(
        d.join("kiln.toml"),
        format!("{config}\n[[export]]\nname = \"doc\"\nformat = \"markdown\"\npath = \"out/DESIGN.md\"\nreports = [\"changes\"]\n"),
    )
    .unwrap();
    ok(d, &["new", "requirement", "--title", "Fast"]);
    ok(d, &["new", "requirement", "--title", "Small"]);
    ok(d, &["new", "requirement", "--title", "Gone soon"]);
    git(d, &["add", "."]);
    git(d, &["commit", "-q", "-m", "one"]);
    git(d, &["tag", "v1"]);

    ok(d, &["set", "REQ-001", "status=approved"]);
    ok(d, &["new", "requirement", "--title", "Quiet"]);
    std::fs::remove_file(d.join("design/requirements/REQ-003-gone-soon.md")).unwrap();

    let diff = ok(d, &["diff", "v1"]);
    let lines: Vec<&str> = diff.lines().collect();
    assert!(
        lines[0].starts_with("v1 (") && lines[0].ends_with(") → the working tree"),
        "{diff}"
    );
    assert_eq!(
        &lines[1..],
        [
            "~ REQ-001 Fast: status draft → approved",
            "+ REQ-004 Quiet (requirement)",
            "- REQ-003 Gone soon (requirement)",
            "1 added, 1 changed, 1 removed",
        ]
    );
    let json: serde_json::Value =
        serde_json::from_str(&ok(d, &["diff", "v1", "--format", "json"])).unwrap();
    assert_eq!(json["items"][0]["details"][0], "status draft → approved");
    git(d, &["commit", "-q", "-am", "two"]);
    let committed = ok(d, &["diff", "v1..HEAD"]);
    assert!(
        committed.contains("0 added, 1 changed, 1 removed"),
        "REQ-004 isn't committed: {committed}"
    );
    assert_eq!(kiln_in(d, &["diff", "nope"]).status.code(), Some(2));

    ok(d, &["export", "--since", "v1"]);
    let md = std::fs::read_to_string(d.join("out/DESIGN.md")).unwrap();
    assert!(md.contains("REQ-001 Fast\n\n*Changed*\n"), "{md}");
    assert!(md.contains("REQ-004 Quiet\n\n*New*\n"), "{md}");
    assert!(
        md.contains("REQ-002 Small\n\n**draft**"),
        "unchanged items have no marker: {md}"
    );
    assert!(md.contains("Since v1 ("), "{md}");
    assert!(md.contains("| REQ-003 Gone soon | **Removed** |"), "{md}");
    // Without a baseline the report says how to get one.
    ok(d, &["export"]);
    let md = std::fs::read_to_string(d.join("out/DESIGN.md")).unwrap();
    assert!(md.contains("export with `kiln export --since REV`"), "{md}");
}

#[test]
fn confluence_comments_are_listed_per_item() {
    use kiln_confluence::fake::{Fake, FakePage, State};
    let mut state = State::default();
    state.pages.insert(
        42,
        FakePage {
            title: "Designs".into(),
            parent: None,
            version: 1,
            author: "alice".into(),
            body: String::new(),
            property: None,
        },
    );
    let fake = Fake::start(state, false);
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--preset", "none", "--name", "Demo"]);
    let config = std::fs::read_to_string(d.join("kiln.toml")).unwrap();
    std::fs::write(
        d.join("kiln.toml"),
        format!("{config}\n[confluence]\nurl = \"{}\"\nspace = \"DEMO\"\n\n[[export]]\nname = \"spec\"\nformat = \"confluence\"\ntitle = \"Demo design\"\n[export.confluence]\nparent = 42\n", fake.url),
    )
    .unwrap();
    let path = ok(d, &["new", "requirement", "--title", "Fast"]);
    std::fs::write(d.join(path.trim()), "+++\nid = \"REQ-001\"\ntype = \"requirement\"\ntitle = \"Fast\"\nstatus = \"draft\"\n+++\n\nStarts in a second.\n").unwrap();
    let before = kiln_publishing(d, &["confluence", "comments"]);
    assert!(
        stdout(&before).contains("spec → space DEMO: not published yet"),
        "{}",
        stdout(&before)
    );
    assert_eq!(
        kiln_publishing(d, &["export", "--yes"]).status.code(),
        Some(0)
    );
    {
        let mut s = fake.state.lock().unwrap();
        let doc = *s
            .pages
            .iter()
            .find(|(_, p)| p.title == "Demo design")
            .unwrap()
            .0;
        s.extra.entry(doc).or_default().comments = vec![
            (
                "7".into(),
                "bob".into(),
                "<blockquote><p>Starts in a second.</p></blockquote><p>Cold or warm?</p>".into(),
                false,
            ),
            ("8".into(), "carol".into(), "<p>Done.</p>".into(), true),
        ];
    }
    let out = kiln_publishing(d, &["confluence", "comments"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("spec → space DEMO: 1 unresolved comment\n"),
        "{text}"
    );
    assert!(
        text.contains("  REQ-001   bob, 2026-10-12: Starts in a second. Cold or warm?\n"),
        "{text}"
    );
    assert!(text.contains("focusedCommentId=7#comment-7"), "{text}");
    let json: serde_json::Value = serde_json::from_str(&stdout(&kiln_publishing(
        d,
        &["confluence", "comments", "spec", "--format", "json"],
    )))
    .unwrap();
    assert_eq!(json[0]["comments"][0]["item"], "REQ-001");
    assert_eq!(
        kiln_publishing(d, &["confluence", "comments", "nope"])
            .status
            .code(),
        Some(2)
    );
}

fn kiln_jira(dir: &Path, token: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(args)
        .current_dir(dir)
        .env("FENIX_HOME", dir.join("no-fenix"))
        .env("KILN_HOME", dir.join("kiln-home"))
        .env("KILN_JIRA_TOKEN", token)
        .output()
        .expect("the kiln binary runs")
}

#[test]
// verifies: REQ-006
fn list_jira_and_k207_read_the_mirror_once_per_ten_minutes() {
    use kiln_jira::fake::{Fake, State, TOKEN};
    let fake = Fake::start(
        State::default()
            .issue("FLINT-1", "In Review", "indeterminate", Some("Alice"))
            .issue("FLINT-2", "Closed", "done", None),
    );
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--preset", "none", "--name", "Demo"]);
    let config = std::fs::read_to_string(d.join("kiln.toml")).unwrap();
    std::fs::write(
        d.join("kiln.toml"),
        format!(
            "{config}\n[jira]\nurl = \"{}\"\n\n[check]\nK207 = \"error\"\n",
            fake.url
        ),
    )
    .unwrap();
    std::fs::create_dir_all(d.join("design/tasks")).unwrap();
    let task = |id: &str, status: &str, jira: &str| {
        format!("+++\nid = \"{id}\"\ntype = \"task\"\ntitle = \"{id} title\"\nstatus = \"{status}\"\njira = \"{jira}\"\n+++\n")
    };
    std::fs::write(
        d.join("design/tasks/TASK-001.md"),
        task("TASK-001", "done", "FLINT-1"),
    )
    .unwrap();
    std::fs::write(
        d.join("design/tasks/TASK-002.md"),
        task("TASK-002", "done", "FLINT-2"),
    )
    .unwrap();

    let out = kiln_jira(d, TOKEN, &["list", "--jira"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("TASK-001 title  [FLINT-1 In Review · Alice]"),
        "{text}"
    );
    assert!(text.contains("TASK-002 title  [FLINT-2 Closed]"), "{text}");
    assert_eq!(fake.requests(), 1);

    // K207 from the cached mirror: no second request.
    let out = kiln_jira(d, TOKEN, &["check"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("design/tasks/TASK-001.md:5:10: K207 error: TASK-001 is done but its Jira ticket isn't resolved: FLINT-1 (In Review)"),
        "{}",
        stdout(&out)
    );
    assert!(!stdout(&out).contains("TASK-002"), "{}", stdout(&out));
    assert_eq!(fake.requests(), 1);
    let json = kiln_jira(d, TOKEN, &["list", "--jira", "--format", "json"]);
    assert!(
        stdout(&json).contains("\"resolved\": false"),
        "{}",
        stdout(&json)
    );

    // Jira unreachable for K207 is a note, not a failed check; for an
    // explicit --jira it is an error. The token never shows.
    std::fs::remove_dir_all(d.join("kiln-home")).unwrap();
    let out = kiln_jira(d, "wrong-secret", &["check"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("K207 not checked: Jira at"),
        "{}",
        stderr(&out)
    );
    let out = kiln_jira(d, "wrong-secret", &["list", "--jira"]);
    assert_ne!(out.status.code(), Some(0));
    assert_eq!(stderr(&out).lines().count(), 1, "{}", stderr(&out));
    assert!(!stderr(&out).contains("wrong-secret"));
}

#[test]
// verifies: REQ-001
fn jira_sync_creates_sub_tasks_records_their_keys_and_pull_takes_status_back() {
    use kiln_jira::fake::{Fake, State, TOKEN};
    let fake = Fake::start(State::default());
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--preset", "none", "--name", "Demo"]);
    let config = std::fs::read_to_string(d.join("kiln.toml")).unwrap();
    std::fs::write(
        d.join("kiln.toml"),
        format!(
            "{config}\n[jira]\nurl = \"{}\"   # the team's Jira\nproject = \"DEMO\"\n",
            fake.url
        ),
    )
    .unwrap();
    std::fs::create_dir_all(d.join("design/tasks")).unwrap();
    let task = |id: &str, status: &str| {
        format!("+++\nid = \"{id}\"  # keep\ntype = \"task\"\ntitle = \"{id} title\"\nstatus = \"{status}\"\n+++\n\nThe body.\n")
    };
    std::fs::write(d.join("design/tasks/TASK-001.md"), task("TASK-001", "todo")).unwrap();
    std::fs::write(
        d.join("design/tasks/TASK-002.md"),
        task("TASK-002", "doing"),
    )
    .unwrap();

    // A dry run plans and writes nothing, in Jira or in the files.
    let out = kiln_jira(d, TOKEN, &["jira", "sync", "--dry-run"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let plan = stdout(&out);
    assert!(
        plan.starts_with("jira (DEMO) → a new project issue (Task)\n"),
        "{plan}"
    );
    assert!(
        plan.contains("  create     \"Kiln: Demo\" (Task), the project issue"),
        "{plan}"
    );
    assert!(
        plan.contains(
            "  create     TASK-002 TASK-002 title (Sub-task), then to an in-progress status"
        ),
        "{plan}"
    );
    assert!(
        plan.contains("  3 create, 0 update, 0 move, 0 unchanged"),
        "{plan}"
    );
    assert!(fake.state.lock().unwrap().writes().is_empty());

    // Not on a terminal and without --yes: nothing happens.
    let out = kiln_jira(d, TOKEN, &["jira", "sync"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(fake.state.lock().unwrap().writes().is_empty());

    let out = kiln_jira(d, TOKEN, &["jira", "sync", "--yes"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stdout(&out).contains(
            "jira: 3 created, 0 updated, 1 moved; 3 files gained ticket keys (commit them)"
        ),
        "{}",
        stdout(&out)
    );
    let config = std::fs::read_to_string(d.join("kiln.toml")).unwrap();
    assert!(config.contains("parent = \"DEMO-101\""), "{config}");
    assert!(
        config.contains("# the team's Jira"),
        "comments kept: {config}"
    );
    let t1 = std::fs::read_to_string(d.join("design/tasks/TASK-001.md")).unwrap();
    assert!(t1.contains("jira = \"DEMO-102\""), "{t1}");
    assert!(
        t1.contains("id = \"TASK-001\"  # keep") && t1.ends_with("The body.\n"),
        "{t1}"
    );

    // Synced: the next plan changes nothing.
    let out = kiln_jira(d, TOKEN, &["jira", "sync", "--dry-run"]);
    let plan = stdout(&out);
    assert!(
        plan.starts_with("jira (DEMO) → DEMO-101 \"Kiln: Demo\"\n"),
        "{plan}"
    );
    assert!(
        plan.contains("0 create, 0 update, 0 move, 2 unchanged"),
        "{plan}"
    );

    // A change in Kiln becomes a move; a move in Jira comes back by pull.
    ok(d, &["set", "TASK-001", "status=done"]);
    let out = kiln_jira(d, TOKEN, &["jira", "sync", "--yes"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("  move       DEMO-102 TASK-001: To Do → Done"),
        "{}",
        stdout(&out)
    );
    assert_eq!(fake.state.lock().unwrap().issues["DEMO-102"].status, "Done");

    fake.state
        .lock()
        .unwrap()
        .issues
        .get_mut("DEMO-103")
        .unwrap()
        .status = "Done".into();
    fake.state
        .lock()
        .unwrap()
        .issues
        .get_mut("DEMO-103")
        .unwrap()
        .category = "done".into();
    let out = kiln_jira(d, TOKEN, &["jira", "pull"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        stdout(&out),
        "TASK-002: status doing → done (from DEMO-103 Done)\n"
    );
    let t2 = std::fs::read_to_string(d.join("design/tasks/TASK-002.md")).unwrap();
    assert!(t2.contains("status = \"done\""), "{t2}");
}

/// The `toml` blocks of the wiki's Presets page marked `<!-- MARK -->`
/// (`added: arc42`, `tested: arc42`), in order: what the page says a
/// preset adds, and the options it tells people to paste.
fn wiki_blocks(page: &str, mark: &str) -> Vec<String> {
    let page = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../docs/wiki/{page}.md")),
    )
    .expect("the wiki page is there");
    let marker = format!("<!-- {mark} -->");
    let mut out = Vec::new();
    let mut rest = page.as_str();
    while let Some(at) = rest.find(&marker) {
        rest = &rest[at + marker.len()..];
        let body = rest
            .trim_start()
            .strip_prefix("```toml\n")
            .expect("a marker is followed by a toml block");
        let end = body.find("\n```").expect("the toml block ends");
        out.push(body[..=end].to_owned());
    }
    assert!(!out.is_empty(), "no `{marker}` blocks");
    out
}

/// Pastes `snippet` into `config` as the wiki says: a `[types.*]` or
/// `[links.*]` table the file already has is replaced (the file's tables
/// end at a blank line).
fn paste(config: &str, snippet: &str) -> String {
    let mut config = config.to_owned();
    for header in snippet
        .lines()
        .filter(|l| l.starts_with("[types.") || l.starts_with("[links."))
    {
        let needle = format!("\n{header}\n");
        if let Some(at) = config.find(&needle) {
            let end = config[at + 1..]
                .find("\n\n")
                .map_or(config.len(), |e| at + 1 + e);
            config.replace_range(at..end, "");
        }
    }
    format!("{}\n\n{snippet}", config.trim_end())
}

#[test]
fn presets_populate_kiln_toml_once_and_the_wiki_says_what_they_add() {
    let out = kiln(&["preset", "list"]);
    assert!(out.status.success());
    let names: Vec<String> = stdout(&out)
        .lines()
        .map(|l| l.split_whitespace().next().unwrap().to_owned())
        .collect();
    assert_eq!(names, ["arc42", "srs", "design-doc"]);

    // An unknown preset writes nothing, not even the kiln.toml.
    let dir = tempfile::tempdir().unwrap();
    let out = kiln_in(dir.path(), &["init", "--preset", "arc43"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("no preset `arc43`"),
        "{}",
        stderr(&out)
    );
    assert!(!dir.path().join("kiln.toml").exists());

    // Each preset alone writes what the wiki says it adds.
    for name in ["arc42", "srs", "design-doc"] {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let written = ok(d, &["init", "--name", "Shop", "--preset", name]);
        assert!(
            written.contains(&format!("design/templates/{name}.md")),
            "{written}"
        );
        let config = std::fs::read_to_string(d.join("kiln.toml")).unwrap();
        for block in wiki_blocks("Presets", &format!("added: {name}")) {
            for line in block
                .lines()
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
            {
                assert!(
                    config.contains(line),
                    "{name}: the wiki says it adds {line:?}\n{config}"
                );
            }
        }
        ok(d, &["check"]);
        ok(d, &["export", name]);
    }

    // All three in one project: nothing twice.
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--preset", "none", "--name", "Shop"]);
    for name in ["arc42", "srs", "design-doc"] {
        ok(d, &["preset", "add", name]);
    }
    let config = std::fs::read_to_string(d.join("kiln.toml")).unwrap();
    let mut seen = std::collections::HashSet::new();
    for line in config.lines().filter(|l| {
        l.starts_with("[types.") || l.starts_with("[links.") || l.starts_with("name = ")
    }) {
        assert!(seen.insert(line), "{line} twice\n{config}");
    }
    // Again: refused, nothing changes; a dry run writes nothing.
    let out = kiln_in(d, &["preset", "add", "srs"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("already has an export named `srs`"));
    assert_eq!(
        std::fs::read_to_string(d.join("kiln.toml")).unwrap(),
        config
    );

    // The wiki's arc42 options on top, then items linked by every role.
    let mut config = config;
    for snippet in wiki_blocks("Presets", "tested: arc42") {
        config = paste(&config, &snippet);
    }
    std::fs::write(d.join("kiln.toml"), &config).unwrap();
    ok(d, &["new", "quality-goal", "--title", "Fast pages"]);
    ok(
        d,
        &[
            "new",
            "quality-scenario",
            "--title",
            "Search under load",
            "--parent",
            "QG-001",
        ],
    );
    ok(d, &["new", "constraint", "--title", "Runs on Kubernetes"]);
    ok(d, &["new", "block", "--title", "Front end"]);
    ok(
        d,
        &["new", "block", "--title", "Basket", "--parent", "BB-001"],
    );
    ok(d, &["new", "decision", "--title", "Server-side rendering"]);
    ok(d, &["new", "term", "--title", "Basket"]);
    ok(d, &["new", "concept", "--title", "Error handling"]);
    ok(d, &["new", "runtime", "--title", "Checkout"]);
    ok(d, &["new", "test", "--title", "Load test"]);
    for (id, role, target) in [
        ("ADR-001", "addresses", "QG-001"),
        ("ADR-001", "affects", "BB-001"),
        ("ADR-001", "respects", "CON-001"),
        ("ADR-001", "mitigates", "RISK-001"),
        ("BB-002", "uses", "BB-001"),
        ("BB-002", "applies", "CPT-001"),
        ("RT-001", "involves", "BB-002"),
        ("TEST-001", "verifies", "QS-001"),
    ] {
        if id == "ADR-001" && role == "mitigates" {
            ok(d, &["new", "risk", "--title", "Peak load"]);
        }
        ok(d, &["link", id, role, target]);
    }
    let scenario = std::fs::read_dir(d.join("design/quality-scenarios"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(std::fs::read_to_string(scenario)
        .unwrap()
        .contains("**Stimulus:**"));
    std::fs::create_dir_all(d.join("src")).unwrap();
    std::fs::write(
        d.join("src/basket.rs"),
        format!("// {}: BB-002\n", "implements"),
    )
    .unwrap();
    let config = std::fs::read_to_string(d.join("kiln.toml")).unwrap();
    std::fs::write(
        d.join("kiln.toml"),
        config.replace("# [code]\n# paths = [\"src\"]", "[code]\npaths = [\"src\"]"),
    )
    .unwrap();
    ok(d, &["check"]);
    ok(d, &["export", "arc42", "srs", "design-doc"]);
    let read = |f: &str| std::fs::read_to_string(d.join("docs/arc42").join(f)).unwrap();
    assert!(read("glossary.md").contains("**TERM-001 Basket**"));
    let blocks = read("building-block-view.md");
    assert!(
        blocks.contains("src/basket.rs"),
        "code implements a block: {blocks}"
    );
    assert!(blocks.contains("**Used by:**"), "{blocks}");
    assert!(read("quality-requirements.md").contains("**Verified by:**"));
    assert!(
        !read("introduction-and-goals.md").contains("<!--"),
        "the template's notes stay out of the export"
    );
    let srs = std::fs::read_to_string(d.join("docs/SRS.md")).unwrap();
    assert!(
        srs.contains("**TERM-001 Basket**"),
        "the shared term type serves both"
    );
}

#[test]
fn the_wiki_s_requirement_levels_work_as_pasted() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--preset", "none", "--name", "Demo"]);
    let config = d.join("kiln.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    for snippet in wiki_blocks("Configuration", "tested: levels") {
        text = paste(&text, &snippet);
    }
    std::fs::write(&config, text).unwrap();
    ok(d, &["agents"]);
    let check = kiln_in(d, &["check"]);
    assert_eq!(check.status.code(), Some(0), "{}", stdout(&check));
    assert_eq!(stderr(&check).trim(), "kiln: no findings");

    ok(d, &["new", "stakeholder", "--title", "Faults are visible"]);
    ok(d, &["new", "system", "--title", "Fault telemetry"]);
    ok(d, &["new", "software", "--title", "Poll the register"]);
    ok(d, &["link", "SYS-001", "refines", "STK-001"]);
    ok(d, &["link", "SW-001", "refines", "SYS-001"]);
    for id in ["STK-001", "SYS-001", "SW-001"] {
        ok(d, &["set", id, "status=approved"]);
    }
    let check = stdout(&kiln_in(d, &["check"]));
    // Nothing verifies them yet, and only the software level is
    // implemented; refinement is complete.
    assert!(check.contains("K201 warning: STK-001"), "{check}");
    assert!(check.contains("K202 warning: SW-001"), "{check}");
    assert!(!check.contains("K202 warning: SYS-001"), "{check}");
    assert!(
        !check.contains("K208") && !check.contains("K210"),
        "{check}"
    );
    let csv = ok(d, &["trace", "--levels", "--format", "csv"]);
    assert!(csv.contains("\nSTK-001,SYS-001,SW-001,"), "{csv}");
}

#[test]
// verifies: REQ-019
fn the_agent_guide_is_written_by_init_and_kept_current() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let init = ok(d, &["init", "--preset", "none", "--name", "Demo"]);
    assert!(init.contains("design/AGENTS.md"), "{init}");
    let guide = d.join("design/AGENTS.md");
    let text = std::fs::read_to_string(&guide).unwrap();
    assert!(text.starts_with("# Working on Demo's design"), "{text}");
    assert!(text.contains("| requirement (Requirement) | `REQ-001` | `design/requirements/`"));
    assert_eq!(stderr(&kiln_in(d, &["check"])).trim(), "kiln: no findings");

    // The project's own words survive; kiln.toml changes show up as K113
    // until `kiln agents` runs.
    std::fs::write(&guide, text.replace("## Rules\n", "## Rules\n\n- Ours.\n")).unwrap();
    let config = d.join("kiln.toml");
    let toml = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        &config,
        format!("{toml}\n[types.term]\nprefix = \"TERM\"\nstatuses = [\"draft\"]\n"),
    )
    .unwrap();
    let check = stdout(&kiln_in(d, &["check"]));
    assert!(
        check.contains("K113 warning: Kiln's section no longer matches kiln.toml"),
        "{check}"
    );
    assert_eq!(ok(d, &["agents"]).trim(), "design/AGENTS.md");
    let text = std::fs::read_to_string(&guide).unwrap();
    assert!(
        text.contains("- Ours.\n") && text.contains("| term (Term) | `TERM-001`"),
        "{text}"
    );
    assert_eq!(stderr(&kiln_in(d, &["check"])).trim(), "kiln: no findings");
    let again = kiln_in(d, &["agents"]);
    assert_eq!(stdout(&again), "");
    assert!(stderr(&again).contains("up to date"));

    // A preset's types reach the guide as they reach kiln.toml.
    let added = ok(d, &["preset", "add", "srs"]);
    assert!(added.contains("design/AGENTS.md"), "{added}");
    assert_eq!(stderr(&kiln_in(d, &["check"])).trim(), "kiln: no findings");
}

#[test]
// verifies: REQ-020
fn an_agent_works_through_the_plan_from_the_command_line() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let init = kiln_in(d, &["init", "--preset", "none", "--name", "Demo"]);
    assert!(
        stderr(&init).contains("add this line to the repository's AGENTS.md or CLAUDE.md"),
        "{}",
        stderr(&init)
    );
    let guide = std::fs::read_to_string(d.join("design/AGENTS.md")).unwrap();
    assert!(guide.contains("### Planning") && guide.contains("`kiln next`"));
    ok(d, &["new", "milestone", "--title", "First release"]);
    ok(d, &["new", "requirement", "--title", "Export as CSV"]);

    // One command creates a task whole: links, tag, body.
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args([
            "new",
            "task",
            "--title",
            "Write the CSV writer",
            "--link",
            "part-of=MS-001",
            "--link",
            "implements=REQ-001",
            "--tag",
            "csv",
            "--body-file",
            "-",
            "--format",
            "json",
        ])
        .current_dir(d)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(child.stdin.as_mut().unwrap(), b"One row per item.\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let created: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(created["id"], "TASK-001");
    ok(
        d,
        &[
            "new",
            "task",
            "--title",
            "Wire the button",
            "--link",
            "part-of=MS-001",
            "--link",
            "depends-on=TASK-001",
        ],
    );

    let show = ok(d, &["show", "TASK-001"]);
    assert!(show.ends_with("\nOne row per item.\n"), "{show}");
    let json: serde_json::Value =
        serde_json::from_str(&ok(d, &["show", "TASK-001", "--format", "json"])).unwrap();
    assert_eq!(json["links"]["part-of"][0]["id"], "MS-001");
    assert_eq!(json["tags"][0], "csv");
    assert_eq!(json["body"], "\nOne row per item.\n");
    let list: serde_json::Value = serde_json::from_str(&ok(
        d,
        &["list", "--type", "task", "--format", "json", "--body"],
    ))
    .unwrap();
    assert_eq!(list[1]["links"]["depends-on"][0], "TASK-001");
    assert_eq!(list[0]["body"], "\nOne row per item.\n");

    assert_eq!(
        ok(d, &["next"]),
        "TASK-001  todo  Write the CSV writer  (MS-001)\n"
    );
    let status = ok(d, &["status"]);
    assert!(
        status.contains("MS-001 First release · planned · 0 of 2 done"),
        "{status}"
    );
    assert!(
        status.contains("waiting      TASK-002  todo  Wire the button  (on TASK-001)"),
        "{status}"
    );

    ok(d, &["set", "TASK-001", "tags=csv,io", "jira=DEMO-4"]);
    let shown = ok(d, &["show", "TASK-001"]);
    assert!(
        shown.contains("tags        csv, io") && shown.contains("jira        DEMO-4"),
        "{shown}"
    );

    // Marking the plan done ahead of the work is reported.
    ok(d, &["set", "TASK-002", "status=done"]);
    ok(d, &["set", "MS-001", "status=done"]);
    let check = ok(d, &["check"]);
    assert!(
        check.contains("K211 warning: MS-001 is done, but TASK-001 (todo) is part of it"),
        "{check}"
    );
    assert!(
        check.contains("K212 warning: TASK-002 is done, but it depends on TASK-001 (todo)"),
        "{check}"
    );
}

#[test]
// verifies: REQ-021
fn a_first_project_explains_itself() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--name", "Demo"]);
    // Each default type starts from a body template.
    let path = ok(d, &["new", "decision", "--title", "Use CSV"]);
    let text = std::fs::read_to_string(d.join(path.trim())).unwrap();
    assert!(
        text.contains("## Context") && text.contains("## Consequences"),
        "{text}"
    );
    // The design-doc export works on day one.
    let export = kiln_in(d, &["export"]);
    assert_eq!(export.status.code(), Some(0), "{}", stderr(&export));
    assert!(d.join("docs/DESIGN-DOCUMENT.md").exists());
    // It places the plan and the tests, so a first export leaves nothing out (K17).
    ok(d, &["new", "task", "--title", "Placed"]);
    ok(d, &["new", "test", "--title", "Placed too"]);
    let dry = kiln_in(d, &["export", "--dry-run"]);
    assert!(!stdout(&dry).contains("leaves out"), "{}", stdout(&dry));
    // Filters take lists, and a status nothing has is said aloud.
    ok(d, &["new", "task", "--title", "One"]);
    ok(d, &["set", "TASK-001", "status=doing"]);
    let both = ok(d, &["list", "--status", "proposed,doing"]);
    assert_eq!(both.lines().count(), 2, "{both}");
    let typo = kiln_in(d, &["list", "--type", "task", "--status", "in-progress"]);
    assert_eq!(typo.status.code(), Some(2));
    assert!(
        stderr(&typo).contains(
            "`in-progress` isn't a status of a task (blocked, doing, done, dropped, todo)"
        ),
        "{}",
        stderr(&typo)
    );
    // The help shows the everyday commands first.
    let help = ok(d, &["--help"]);
    assert!(
        help.find("Everyday:") < help.find("Integrations:"),
        "{help}"
    );

    // `--preset none` starts bare.
    let bare = tempfile::tempdir().unwrap();
    ok(bare.path(), &["init", "--preset", "none"]);
    assert!(!std::fs::read_to_string(bare.path().join("kiln.toml"))
        .unwrap()
        .contains("[[export]]"));
}

#[test]
// verifies: REQ-022
fn claimed_work_versioned_json_and_template_text() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--name", "Demo"]);
    ok(d, &["new", "milestone", "--title", "First"]);
    ok(
        d,
        &["new", "task", "--title", "One", "--link", "part-of=MS-001"],
    );
    ok(
        d,
        &["new", "task", "--title", "Two", "--link", "part-of=MS-001"],
    );

    // Claiming: the owner shows, and --unowned leaves claimed work out.
    ok(d, &["set", "TASK-001", "owner=ana"]);
    let next = ok(d, &["next"]);
    assert!(
        next.contains("TASK-001  todo  One  (MS-001)  · ana"),
        "{next}"
    );
    assert_eq!(
        ok(d, &["next", "--unowned"]),
        "TASK-002  todo  Two  (MS-001)\n"
    );
    // Started work is in progress, not offered again (K18).
    ok(d, &["set", "TASK-001", "status=doing"]);
    assert_eq!(ok(d, &["next"]), "TASK-002  todo  Two  (MS-001)\n");
    assert!(ok(d, &["status"]).contains("in progress  TASK-001  doing  One  · ana"));
    let status = ok(d, &["status"]);
    assert!(
        status.contains("MS-001 First · planned · 0 of 2 done"),
        "{status}"
    );
    assert!(status.contains("One  · ana"), "{status}");

    // Every JSON object says which shape it is.
    let show: serde_json::Value =
        serde_json::from_str(&ok(d, &["show", "TASK-001", "--format", "json"])).unwrap();
    assert_eq!(show["kiln_json"], 1);
    let status: serde_json::Value =
        serde_json::from_str(&ok(d, &["status", "--format", "json"])).unwrap();
    assert_eq!(status["kiln_json"], 1);
    assert_eq!(status["milestones"][0]["in_progress"][0]["owner"], "ana");

    // TASK-001 is past its first status with the template's prompts in it.
    let check = ok(d, &["check"]);
    assert!(
        check.contains("K215 warning: TASK-001 is doing but still has design/templates/task.md's text \"**What:** the change, in a sentence or two.\" and 1 more of its lines"),
        "{check}"
    );
    assert!(
        !check.contains("TASK-002 is"),
        "a todo task may keep them: {check}"
    );
}

#[test]
// verifies: REQ-023
fn test_results_and_what_a_change_touches() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    git(d, &["init", "-q"]);
    ok(d, &["init", "--name", "Demo", "--preset", "none"]);
    let toml = std::fs::read_to_string(d.join("kiln.toml")).unwrap();
    std::fs::write(
        d.join("kiln.toml"),
        format!("{toml}\n[code]\npaths = [\"src\"]\n"),
    )
    .unwrap();
    ok(d, &["new", "requirement", "--title", "Fast"]);
    ok(d, &["new", "requirement", "--title", "Small"]);
    let tag = |role: &str, id: &str| format!("// {role}: {id}\n");
    std::fs::create_dir_all(d.join("src")).unwrap();
    std::fs::write(
        d.join("src/lib.rs"),
        format!(
            "{}fn fast() {{}}\n{}fn small() {{}}\n{}#[test]\nfn is_fast() {{}}\n{}#[test]\nfn is_small() {{}}\n",
            tag("implements", "REQ-001"),
            tag("implements", "REQ-002"),
            tag("verifies", "REQ-001"),
            tag("verifies", "REQ-002"),
        ),
    )
    .unwrap();
    std::fs::write(
        d.join("junit.xml"),
        r#"<testsuite><testcase name="tests::is_fast"/><testcase name="tests::is_small"><failure/></testcase></testsuite>"#,
    )
    .unwrap();

    // Results: only the passing test verifies.
    let trace = ok(d, &["trace", "--results", "junit.xml"]);
    assert!(
        trace.contains("verified by: src/lib.rs:5 (passed)"),
        "{trace}"
    );
    assert!(
        trace.contains("verified by: src/lib.rs:8 (failed) — gap"),
        "{trace}"
    );
    ok(d, &["set", "REQ-002", "status=approved"]);
    let check = ok(d, &["check", "--results", "junit.xml"]);
    assert!(
        check.contains(
            "src/lib.rs:8:14: K216 warning: the test verifying REQ-002 here (`is_small`) failed"
        ),
        "{check}"
    );
    let bad = kiln_in(d, &["check", "--results", "nope.xml"]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(stderr(&bad).contains("--results nope.xml: can't read it"));

    // Affected: one changed line under REQ-002's reference.
    git(d, &["add", "-A"]);
    git(d, &["commit", "-qm", "base"]);
    let lib = std::fs::read_to_string(d.join("src/lib.rs")).unwrap();
    std::fs::write(
        d.join("src/lib.rs"),
        lib.replace("fn small() {}", "fn small() { 1; }"),
    )
    .unwrap();
    let affected = ok(d, &["affected"]);
    assert!(affected.starts_with("REQ-002 Small  (approved)\n  changed   src/lib.rs:4 (under `implements` at line 3)\n  re-check  src/lib.rs:8\n"), "{affected}");
    assert!(!affected.contains("REQ-001"), "{affected}");
    let github = ok(d, &["affected", "HEAD", "--format", "github"]);
    assert!(github.starts_with("::notice file=src/lib.rs,line=4,title=Kiln affected::changes code that `implements` REQ-002 Small"), "{github}");
}

#[test]
// verifies: REQ-024
fn an_items_history_and_its_sign_off_come_from_git() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    git(d, &["init", "-q"]);
    ok(d, &["init", "--name", "Demo", "--preset", "none"]);
    ok(d, &["new", "requirement", "--title", "Fast"]);
    ok(d, &["new", "test", "--title", "Timing"]);
    git(d, &["add", "-A"]);
    git(d, &["commit", "-qm", "draft"]);
    git(d, &["tag", "base"]);
    ok(d, &["set", "REQ-001", "status=approved"]);
    git(d, &["add", "-A"]);
    git(d, &["commit", "-qm", "approve"]);
    ok(d, &["link", "TEST-001", "verifies", "REQ-001"]);
    ok(d, &["set", "REQ-001", "owner=ana"]);

    let log = ok(d, &["log", "REQ-001"]);
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(lines[0], "REQ-001 Fast");
    assert!(lines[1].ends_with("Kiln  created (draft)"), "{log}");
    assert!(lines[2].ends_with("Kiln  status draft → approved"), "{log}");
    assert!(
        lines[3].starts_with("uncommitted") && lines[3].ends_with("owner (none) → ana"),
        "{log}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&ok(d, &["log", "REQ-001", "--format", "json"])).unwrap();
    assert_eq!(json["events"][1]["changes"][0], "status draft → approved");
    assert_eq!(json["events"][1]["author"], "Kiln");

    let signoff = kiln_in(d, &["signoff", "base"]);
    assert_eq!(signoff.status.code(), Some(0));
    let text = stdout(&signoff);
    assert!(
        text.contains("K218 warning: REQ-001 Fast: status draft → approved, in "),
        "{text}"
    );
    assert!(text.contains(" by Kiln ("), "{text}");
    assert!(!text.contains("TEST-001"), "a test isn't sign-off: {text}");
    let github = ok(d, &["signoff", "base", "--format", "github"]);
    assert!(
        github.starts_with("::warning file=design/requirements/REQ-001-fast.md,line=5"),
        "{github}"
    );

    // [signoff] replaces the defaults; [check] K218 = "error" makes it fail.
    let toml = std::fs::read_to_string(d.join("kiln.toml")).unwrap();
    std::fs::write(
        d.join("kiln.toml"),
        format!("{toml}\n[signoff]\ntest = [\"passing\"]\n\n[check]\nK218 = \"error\"\n"),
    )
    .unwrap();
    assert_eq!(
        stdout(&kiln_in(d, &["signoff", "base"])),
        "",
        "requirement is no longer listed"
    );
    ok(d, &["set", "TEST-001", "status=passing"]);
    let strict = kiln_in(d, &["signoff", "base"]);
    assert_eq!(strict.status.code(), Some(1));
    assert!(stdout(&strict)
        .contains("K218 error: TEST-001 Timing: status draft → passing, not committed yet"));
}
