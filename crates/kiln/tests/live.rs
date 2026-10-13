//! Live tests against a real Confluence (CONFLUENCE.md, "Testing"),
//! ignored by default: they need a site, a token and a scratch page.
//! `dev/confluence/README.md` sets up both flavours. Run with
//!
//! ```text
//! KILN_TEST_CONFLUENCE_URL=… KILN_TEST_CONFLUENCE_SPACE=… \
//! KILN_TEST_CONFLUENCE_PARENT=<page id> KILN_TEST_CONFLUENCE_TOKEN=… \
//! [KILN_TEST_CONFLUENCE_USER=<e-mail, Cloud>] [KILN_TEST_CONFLUENCE_FLAVOUR=cloud|datacenter] \
//! cargo test -p kiln --test live -- --ignored --test-threads 1
//! ```
//!
//! Each run makes its own pages under the parent (titles carry the run's
//! time), so runs don't collide and a failed run leaves evidence; delete
//! the parent's children by hand now and then.

use std::path::Path;
use std::process::{Command, Output};

struct Live {
    url: String,
    space: String,
    parent: String,
    token: String,
    user: Option<String>,
    flavour: Option<String>,
}

fn live() -> Option<Live> {
    let var = |k: &str| {
        std::env::var(format!("KILN_TEST_CONFLUENCE_{k}"))
            .ok()
            .filter(|v| !v.is_empty())
    };
    Some(Live {
        url: var("URL")?,
        space: var("SPACE")?,
        parent: var("PARENT")?,
        token: var("TOKEN")?,
        user: var("USER"),
        flavour: var("FLAVOUR"),
    })
}

fn kiln(l: &Live, dir: &Path, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_kiln"));
    cmd.args(args)
        .current_dir(dir)
        .env("FENIX_HOME", dir.join("no-fenix"))
        .env("KILN_HOME", dir.join("kiln-home"))
        .env("KILN_CONFLUENCE_TOKEN", &l.token);
    match &l.user {
        Some(user) => cmd.env("KILN_CONFLUENCE_USER", user),
        None => cmd.env_remove("KILN_CONFLUENCE_USER"),
    };
    cmd.output().expect("the kiln binary runs")
}

fn ok(l: &Live, dir: &Path, args: &[&str]) -> String {
    let out = kiln(l, dir, args);
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert_eq!(
        out.status.code(),
        Some(0),
        "kiln {args:?}\n{text}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    text
}

/// A project with four requirements (one a child) and a decision, publishing as
/// `layout` under the scratch parent.
fn project(l: &Live, dir: &Path, stamp: &str, layout: &str) {
    ok(l, dir, &["init", "--name", &format!("Kiln live {stamp}")]);
    write_config(l, dir, stamp, layout);
    for title in ["Fast start", "Small binary", "Works offline"] {
        ok(
            l,
            dir,
            &["new", "requirement", "--title", &format!("{title} {stamp}")],
        );
    }
    ok(
        l,
        dir,
        &[
            "new",
            "decision",
            "--title",
            &format!("Plain files {stamp}"),
        ],
    );
    // A child, so `tree` nests a page.
    ok(
        l,
        dir,
        &[
            "new",
            "requirement",
            "--parent",
            "REQ-001",
            "--title",
            &format!("Cold start {stamp}"),
        ],
    );
}

fn write_config(l: &Live, dir: &Path, stamp: &str, layout: &str) {
    let base = std::fs::read_to_string(dir.join("kiln.toml")).expect("kiln init wrote kiln.toml");
    let base = base
        .split("\n[confluence]")
        .next()
        .unwrap_or(&base)
        .to_owned();
    let flavour = l
        .flavour
        .as_deref()
        .map(|f| format!("flavour = \"{f}\"\n"))
        .unwrap_or_default();
    std::fs::write(
        dir.join("kiln.toml"),
        format!(
            "{base}\n[confluence]\nurl = \"{}\"\nspace = \"{}\"\n{flavour}\n[[export]]\nname = \"spec\"\nformat = \"confluence\"\nlayout = \"{layout}\"\ntitle = \"Kiln live {stamp}\"\n[export.confluence]\nparent = {}\n",
            l.url, l.space, l.parent
        ),
    )
    .expect("kiln.toml is writable");
}

fn stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

fn client(
    l: &Live,
    dir: &Path,
) -> (
    Box<dyn kiln_confluence::Confluence>,
    kiln_model::ConfluenceConfig,
) {
    let source = kiln_index::DiskSource::new(dir);
    let project = kiln_index::Project::load(&source).expect("the project loads");
    let config = project
        .config
        .confluence
        .clone()
        .expect("kiln.toml has [confluence]");
    let token = kiln_secrets::Token::new(l.token.clone());
    let user = l.user.clone().unwrap_or_default();
    (kiln_confluence::connect(&config, &token, &user), config)
}

#[test]
#[ignore = "needs a live Confluence: KILN_TEST_CONFLUENCE_*"]
fn each_layout_publishes_then_is_unchanged() {
    let Some(l) = live() else {
        panic!("set KILN_TEST_CONFLUENCE_URL, _SPACE, _PARENT and _TOKEN");
    };
    for layout in ["document", "sections", "tree"] {
        let dir = tempfile::tempdir().unwrap();
        let stamp = format!("{} {layout}", stamp());
        project(&l, dir.path(), &stamp, layout);
        let first = ok(&l, dir.path(), &["export", "--yes"]);
        assert!(first.contains("spec: published"), "{first}");
        let second = ok(&l, dir.path(), &["export", "--dry-run"]);
        assert!(
            second.contains("0 create, 0 update"),
            "{layout}: a second publish changes nothing\n{second}"
        );
        assert!(second.contains(" 0 drift, 0 retire"), "{layout}\n{second}");
    }
}

#[test]
#[ignore = "needs a live Confluence: KILN_TEST_CONFLUENCE_*"]
fn an_edit_is_drift_and_a_removed_item_retires() {
    let Some(l) = live() else {
        panic!("set KILN_TEST_CONFLUENCE_URL, _SPACE, _PARENT and _TOKEN");
    };
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let stamp = format!("{} drift", stamp());
    project(&l, d, &stamp, "tree");
    ok(&l, d, &["export", "--yes"]);

    // A person edits the root page.
    let (c, config) = client(&l, d);
    let title = format!("Kiln live {stamp}");
    let id = c
        .find_title(&config.space, &title)
        .unwrap()
        .expect("the root page exists");
    let page = c.page(id).unwrap().unwrap();
    let body = c.body(id).unwrap();
    c.update_page(
        id,
        None,
        &title,
        &format!("{body}<p>Edited by hand.</p>"),
        page.version + 1,
        "a person's edit",
    )
    .unwrap();
    let drift = kiln(&l, d, &["export", "--yes"]);
    let text = String::from_utf8_lossy(&drift.stdout);
    assert_eq!(drift.status.code(), Some(1), "{text}");
    assert!(text.contains("was edited in Confluence"), "{text}");
    ok(&l, d, &["export", "--yes", "--force"]);

    // An item leaves the repository: its page retires.
    let req3 = std::fs::read_dir(d.join("design/requirements"))
        .unwrap()
        .filter_map(Result::ok)
        .find(|e| e.file_name().to_string_lossy().starts_with("REQ-003"))
        .unwrap();
    std::fs::remove_file(req3.path()).unwrap();
    let retired = ok(&l, d, &["export", "--yes"]);
    assert!(
        retired.contains("retire") && retired.contains("1 retired"),
        "{retired}"
    );
    let again = ok(&l, d, &["export", "--dry-run"]);
    assert!(again.contains("0 create, 0 update"), "{again}");
}
