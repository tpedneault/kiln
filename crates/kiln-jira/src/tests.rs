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

// The sync (ADR-014): planning against the fake, applying, and planning
// again to nothing.

use crate::sync::{plan, Op, Plan};

fn task(id: &str, status: &str, rest: &str) -> String {
    format!("+++\nid = \"{id}\"\ntype = \"task\"\ntitle = \"{id} title\"\nstatus = \"{status}\"\n{rest}+++\n")
}

/// A project syncing tasks to DEMO; `jira` is `[jira]`'s extra lines,
/// `tasks` (path, text).
fn synced(url: &str, jira: &str, tasks: &[(&str, String)]) -> Project {
    let mut source = MemorySource::new().with(
        "kiln.toml",
        format!(
            "{}\n[jira]\nurl = \"{url}\"\nproject = \"DEMO\"\n{jira}",
            kiln_model::default_config("Demo")
        ),
    );
    for (path, text) in tasks {
        source.insert(path, text.as_bytes());
    }
    let project = Project::load(&source).unwrap();
    assert!(project.findings.is_empty(), "{:?}", project.findings);
    project
}

fn sync_plan(fake: &Fake, project: &Project) -> (crate::sync::Remote, Plan) {
    let sync = project.config.jira_sync.clone().unwrap();
    let remote = push::read(&fake.url, TOKEN, project, &sync).unwrap();
    let p = plan(project, &sync, &remote, &|i| format!("from {}", i.path));
    (remote, p)
}

#[test]
fn a_first_sync_creates_the_project_issue_and_a_sub_task_per_open_task() {
    let fake = Fake::start(State::default());
    let tasks = [
        (
            "design/TASK-001.md",
            task("TASK-001", "todo", "due = 2026-11-30\n"),
        ),
        ("design/TASK-002.md", task("TASK-002", "doing", "")),
        ("design/TASK-003.md", task("TASK-003", "dropped", "")),
    ];
    let project = synced(&fake.url, "", &tasks);
    let (remote, p) = sync_plan(&fake, &project);
    assert!(matches!(p.ops[0], Op::CreateParent { ref summary } if summary == "Kiln: Demo"));
    let creates: Vec<String> = p
        .ops
        .iter()
        .filter_map(|o| match o {
            Op::Create { item, .. } => Some(item.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(
        creates,
        ["TASK-001", "TASK-002"],
        "a dropped task gets no ticket"
    );
    assert!(
        fake.state.lock().unwrap().writes().is_empty(),
        "planning writes nothing"
    );

    let sync = project.config.jira_sync.clone().unwrap();
    let done = push::apply(&fake.url, TOKEN, &sync, &remote, &p);
    assert_eq!(done.error, None);
    assert_eq!(done.parent.as_deref(), Some("DEMO-101"));
    assert_eq!(
        done.created,
        [
            ("TASK-001".parse().unwrap(), "DEMO-102".to_owned()),
            ("TASK-002".parse().unwrap(), "DEMO-103".to_owned())
        ]
    );
    {
        let s = fake.state.lock().unwrap();
        let t1 = &s.issues["DEMO-102"];
        assert_eq!(t1.summary, "TASK-001 TASK-001 title");
        assert_eq!(t1.parent.as_deref(), Some("DEMO-101"));
        assert_eq!(t1.due.as_deref(), Some("2026-11-30"));
        assert_eq!(t1.status, "To Do");
        assert_eq!(
            s.issues["DEMO-103"].status, "In Progress",
            "doing: moved on"
        );
    }

    // With the keys written back, the next plan is empty.
    let tasks = [
        (
            "design/TASK-001.md",
            task(
                "TASK-001",
                "todo",
                "jira = \"DEMO-102\"\ndue = 2026-11-30\n",
            ),
        ),
        (
            "design/TASK-002.md",
            task("TASK-002", "doing", "jira = \"DEMO-103\"\n"),
        ),
        ("design/TASK-003.md", task("TASK-003", "dropped", "")),
    ];
    let project = synced(&fake.url, "parent = \"DEMO-101\"\n", &tasks);
    let (_, again) = sync_plan(&fake, &project);
    assert!(!again.changes(), "{:?}", again.ops);
    assert_eq!(again.unchanged, 2);
}

#[test]
fn changes_in_kiln_become_updates_and_transitions_or_refusals() {
    let state = State::default()
        .issue("DEMO-100", "To Do", "new", None)
        .subtask("DEMO-101", "DEMO-100", "TASK-001 old title", "To Do")
        .subtask(
            "DEMO-102",
            "DEMO-100",
            "TASK-002 TASK-002 title",
            "In Progress",
        )
        .subtask("DEMO-103", "DEMO-100", "TASK-003 TASK-003 title", "Done")
        .subtask("DEMO-104", "DEMO-100", "TASK-009 gone", "To Do")
        .issue("OTHER-1", "To Do", "new", None);
    let fake = Fake::start(state);
    let tasks = [
        (
            "design/TASK-001.md",
            task("TASK-001", "done", "jira = \"DEMO-101\"\n"),
        ),
        (
            "design/TASK-002.md",
            task("TASK-002", "blocked", "jira = \"DEMO-102\"\n"),
        ),
        (
            "design/TASK-003.md",
            task("TASK-003", "todo", "jira = \"DEMO-103\"\n"),
        ),
        (
            "design/TASK-004.md",
            task("TASK-004", "doing", "jira = \"OTHER-1\"\n"),
        ),
    ];
    let project = synced(
        &fake.url,
        "parent = \"DEMO-100\"\n\n[jira.status]\nblocked = \"Blocked\"\n",
        &tasks,
    );
    let (remote, p) = sync_plan(&fake, &project);
    let show: Vec<String> = p.ops.iter().map(|o| format!("{o:?}")).collect();
    assert!(
        p.ops
            .iter()
            .any(|o| matches!(o, Op::Update { key, summary: Some(_), .. } if key == "DEMO-101")),
        "{show:#?}"
    );
    assert!(
        p.ops.iter().any(
            |o| matches!(o, Op::Transition { key, to, .. } if key == "DEMO-101" && to == "Done")
        ),
        "{show:#?}"
    );
    assert!(
        p.ops.iter().any(
            |o| matches!(o, Op::Transition { key, to, .. } if key == "DEMO-102" && to == "Blocked")
        ),
        "[jira.status] names Blocked: {show:#?}"
    );
    let refused = p.ops.iter().find_map(|o| match o {
        Op::Refused { key, why, .. } if key == "DEMO-103" => Some(why.clone()),
        _ => None,
    });
    assert_eq!(
        refused.as_deref(),
        Some(
            "no transition from Done to a to-do status (the workflow offers: Reopen → In Progress)"
        )
    );
    assert!(p
        .ops
        .iter()
        .any(|o| matches!(o, Op::Orphan { key, .. } if key == "DEMO-104")));
    assert!(
        p.ops
            .iter()
            .any(|o| matches!(o, Op::LeftAlone { keys, .. } if keys == &["OTHER-1"])),
        "a ticket outside the project issue is never written: {show:#?}"
    );
    assert!(p.refused());

    let sync = project.config.jira_sync.clone().unwrap();
    let done = push::apply(&fake.url, TOKEN, &sync, &remote, &p);
    assert_eq!(done.error, None);
    assert_eq!((done.updated, done.moved), (1, 2));
    let s = fake.state.lock().unwrap();
    assert_eq!(s.issues["DEMO-101"].status, "Done");
    assert_eq!(s.issues["DEMO-101"].summary, "TASK-001 TASK-001 title");
    assert_eq!(s.issues["DEMO-102"].status, "Blocked");
    assert_eq!(s.issues["DEMO-103"].status, "Done", "refused: left alone");
    assert_eq!(s.issues["OTHER-1"].status, "To Do");
    assert!(
        s.issues.contains_key("DEMO-104"),
        "an orphan is never deleted"
    );
}

#[test]
fn required_fields_kiln_doesnt_set_refuse_creating() {
    let mut state = State::default().issue("DEMO-100", "To Do", "new", None);
    state
        .required
        .insert("Sub-task".into(), vec!["components".into()]);
    let fake = Fake::start(state);
    let tasks = [("design/TASK-001.md", task("TASK-001", "todo", ""))];
    let project = synced(&fake.url, "parent = \"DEMO-100\"\n", &tasks);
    let (_, p) = sync_plan(&fake, &project);
    assert_eq!(
        p.refusals,
        ["Sub-task needs fields Kiln doesn't set (components): give them defaults in Jira, or create the tickets by hand"]
    );
}

#[test]
fn statuses_map_back_for_pull() {
    let fake = Fake::start(State::default());
    let tasks = [("design/TASK-001.md", task("TASK-001", "todo", ""))];
    let project = synced(
        &fake.url,
        "\n[jira.status]\nblocked = \"Blocked\"\n",
        &tasks,
    );
    let sync = project.config.jira_sync.clone().unwrap();
    let item = &project.items[0];
    let back =
        |status: &str, category: &str| sync::status_back(&project, &sync, item, status, category);
    assert_eq!(back("Blocked", "indeterminate").as_deref(), Some("blocked"));
    assert_eq!(back("In Review", "indeterminate").as_deref(), Some("doing"));
    assert_eq!(back("Done", "done").as_deref(), Some("done"));
    assert_eq!(back("Open", "new").as_deref(), Some("todo"));
}
