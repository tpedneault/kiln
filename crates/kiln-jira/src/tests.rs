//! The mirror against the fake Jira, and K207 over a small project.

use kiln_index::{MemorySource, Project};

use crate::fake::{Fake, State, TOKEN};
use crate::*;

fn item(id: &str, ty: &str, status: &str, jira: &str) -> String {
    format!("+++\nid = \"{id}\"\ntype = \"{ty}\"\ntitle = \"{id} title\"\nstatus = \"{status}\"\njira = {jira}\n+++\n")
}

fn project() -> Project {
    let source = MemorySource::new()
        .with("kiln.toml", kiln_model::default_config("T"))
        .with(
            "design/TASK-001.md",
            item("TASK-001", "task", "done", "\"FLINT-1\""),
        )
        .with(
            "design/TASK-002.md",
            item("TASK-002", "task", "doing", "\"FLINT-2\""),
        )
        .with(
            "design/TASK-003.md",
            item("TASK-003", "task", "done", "[\"FLINT-3\", \"FLINT-1\"]"),
        )
        .with(
            "design/TASK-004.md",
            item("TASK-004", "task", "dropped", "\"FLINT-2\""),
        )
        .with(
            "design/TASK-005.md",
            item("TASK-005", "task", "doing", "\"NOPE-9\""),
        )
        .with(
            "design/ADR-001.md",
            item("ADR-001", "decision", "accepted", "\"FLINT-2\""),
        );
    let project = Project::load(&source).unwrap();
    assert!(project.findings.is_empty(), "{:?}", project.findings);
    project
}

fn fake() -> Fake {
    Fake::start(
        State::default()
            .issue("FLINT-1", "Done", "done", Some("Alice"))
            .issue("FLINT-2", "Closed", "done", None)
            .issue("FLINT-3", "In Review", "indeterminate", Some("Bob")),
    )
}

#[test]
fn keys_are_sorted_once_each() {
    assert_eq!(
        keys(&project()),
        ["FLINT-1", "FLINT-2", "FLINT-3", "NOPE-9"]
    );
}

#[test]
fn fetch_reads_every_key_and_names_the_unknown_ones() {
    let fake = fake();
    let mirror = fetch(&format!("{}/", fake.url), TOKEN, &keys(&project()), 1000).unwrap();
    assert_eq!(mirror.url, fake.url);
    assert_eq!(mirror.fetched, 1000);
    assert_eq!(mirror.tickets.len(), 3);
    assert_eq!(mirror.missing.iter().collect::<Vec<_>>(), ["NOPE-9"]);
    let t = mirror.ticket("FLINT-3").unwrap();
    assert_eq!((t.status.as_str(), t.resolved), ("In Review", false));
    assert_eq!(t.line(), "In Review · Bob");
    assert_eq!(mirror.ticket("FLINT-2").unwrap().line(), "Closed");
    // One search failed on the unknown key, then one per key.
    assert_eq!(fake.requests(), 1 + 4);

    // Fresh for ten minutes, for these keys of this Jira.
    let all = keys(&project());
    assert!(mirror.covers(&fake.url, &all, 1000 + FRESH_SECS - 1));
    assert!(!mirror.covers(&fake.url, &all, 1000 + FRESH_SECS));
    assert!(!mirror.covers("https://other.example.com", &all, 1000));
    assert!(!mirror.covers(&fake.url, &["FLINT-4".to_owned()], 1000));
    assert_eq!(Mirror::parse(&mirror.to_text()), Some(mirror));
    assert_eq!(Mirror::parse("not json"), None);
}

#[test]
fn a_wrong_token_is_one_line_naming_the_fix() {
    let fake = fake();
    let e = fetch(&fake.url, "wrong", &["FLINT-1".to_owned()], 0).unwrap_err();
    assert!(
        e.starts_with(&format!("Jira at {}: HTTP 401", fake.url)),
        "{e}"
    );
    assert!(e.contains("KILN_JIRA_TOKEN"), "{e}");
    assert!(!e.contains("wrong"), "the token never shows: {e}");
    assert_eq!(fake.requests(), 1);
    assert!(sign_in(&fake.url, TOKEN).is_ok());
    assert!(sign_in(&fake.url, "wrong").is_err());
}

#[test]
fn cloud_is_refused_by_name() {
    assert!(is_cloud("https://acme.atlassian.net"));
    assert!(is_cloud("https://ACME.atlassian.net:443/"));
    assert!(!is_cloud("https://jira.example.com/atlassian.net"));
    let e = fetch("https://acme.atlassian.net", TOKEN, &[], 0).unwrap_err();
    assert!(e.contains("G6"), "{e}");
}

#[test]
fn cache_names_are_file_names() {
    assert_eq!(
        cache_name("https://jira.example.com/jira/"),
        "jira/jira.example.com_jira.json"
    );
    assert_eq!(
        cache_name("http://127.0.0.1:8080"),
        "jira/127.0.0.1_8080.json"
    );
}

#[test]
fn k207_done_and_resolved_disagree() {
    let fake = fake();
    let project = project();
    let mirror = fetch(&fake.url, TOKEN, &keys(&project), 0).unwrap();
    let got: Vec<String> = k207(&project, &mirror)
        .iter()
        .map(|f| format!("{}:{} {}", f.path, f.line, f.message))
        .collect();
    assert_eq!(
        got,
        [
            // Its only ticket is resolved, the task isn't done.
            "design/TASK-002.md:5 TASK-002 is doing but its Jira ticket is resolved: FLINT-2; set it to done",
            // Done, with a ticket still in review.
            "design/TASK-003.md:5 TASK-003 is done but its Jira ticket isn't resolved: FLINT-3 (In Review)",
        ]
    );
}
