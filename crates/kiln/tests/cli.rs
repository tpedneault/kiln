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
fn no_arguments_prints_help_and_exits_2() {
    let out = kiln(&[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("Usage: kiln"));
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
    ok(d, &["init", "--name", "Demo"]);
    assert_eq!(
        kiln_in(d, &["init"]).status.code(),
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
    assert_eq!(stderr(&check).trim(), "kiln: 2 warnings");

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
    assert_eq!(json["verified by"][0], "TEST-001 Benchmark");
    assert_eq!(json["children"][0], "REQ-002 Untitled requirement");
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
    assert!(stdout(&check).contains("::error file=design/tests/TEST-001-benchmark.md,line=11,col=7,title=K102::reference REQ-099 doesn't exist"), "{}", stdout(&check));
}

#[test]
// verifies: REQ-015
fn renumber_moves_an_id_everywhere() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init"]);
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
    ok(d, &["init", "--name", "Demo"]);
    let config = std::fs::read_to_string(d.join("kiln.toml")).unwrap();
    std::fs::write(
        d.join("kiln.toml"),
        format!("{config}\n[[export]]\nname = \"doc\"\nformat = \"markdown\"\npath = \"docs/DESIGN.md\"\n\n[[export]]\nname = \"page\"\nformat = \"html\"\nlayout = \"sections\"\npath = \"out/html\"\n"),
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
    assert_eq!(kiln_in(d, &["export", "--check"]).status.code(), Some(0));

    ok(d, &["set", "REQ-001", "status=approved"]);
    assert_eq!(
        kiln_in(d, &["export", "--check", "doc"]).status.code(),
        Some(1)
    );
    let unknown = kiln_in(d, &["export", "nope"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(stderr(&unknown).contains("no export target `nope` (doc, page)"));
}

#[test]
fn confluence_targets_plan_from_a_snapshot_and_never_write() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--name", "Demo"]);
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
        plan.contains("spec (confluence, document) → space DEMO, page 42"),
        "{plan}"
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
    ok(d, &["init", "--name", "Demo"]);
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
    ok(d, &["init", "--name", "Demo", "--fenix"]);
    let tools: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(d.join(".fenix/tools.json")).unwrap())
            .unwrap();
    assert_eq!(tools["lsp"]["markdown"]["executable"], "kiln");
    assert_eq!(tools["lsp"]["markdown"]["args"][0], "lsp");
    assert_eq!(
        tools["tasks"]["build"]["executable"], "cargo",
        "the rest is kept"
    );

    // An existing project gets only the editor setup; another Markdown
    // server is left alone.
    std::fs::write(
        d.join(".fenix/tools.json"),
        "{\"lsp\": {\"markdown\": {\"executable\": \"marksman\"}}}\n",
    )
    .unwrap();
    let out = kiln_in(d, &["init", "--fenix"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(stderr(&out).contains("already runs `marksman` for Markdown; left it alone"));
    assert!(std::fs::read_to_string(d.join(".fenix/tools.json"))
        .unwrap()
        .contains("marksman"));
    assert_eq!(
        kiln_in(d, &["init"]).status.code(),
        Some(2),
        "kiln.toml exists"
    );
}

#[test]
fn lsp_speaks_the_protocol_on_stdio() {
    use std::io::{BufRead, BufReader, Read, Write};
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    ok(d, &["init", "--name", "Demo"]);
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
    ok(d, &["init", "--name", "Demo"]);
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
    ok(d, &["init", "--name", "Demo"]);
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
    ok(d, &["init", "--name", "Demo"]);
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
