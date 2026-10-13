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
