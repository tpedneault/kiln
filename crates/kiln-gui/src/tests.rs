//! The pages and the keys, over a small in-memory project, read as text
//! (GUI.md).

use std::path::PathBuf;

use fenix_keymap::{KeyPress, NamedKey};
use kiln_index::{MemorySource, Project};

use crate::app::{App, Baseline, Effect};
use crate::edit::Edit;
use crate::host::{Comment, CommentGroup, Done, ExportReport, Job};
use crate::pages::Page;

fn item(id: &str, ty: &str, title: &str, status: &str, rest: &str) -> String {
    format!("+++\nid = \"{id}\"\ntype = \"{ty}\"\ntitle = \"{title}\"\nstatus = \"{status}\"\n{rest}+++\n")
}

fn source() -> MemorySource {
    MemorySource::new()
        .with(
            "kiln.toml",
            format!(
                "{}\n[code]\npaths = [\"src\"]\n",
                kiln_model::default_config("Demo")
            ),
        )
        .with(
            "design/REQ-001.md",
            item("REQ-001", "requirement", "Fast start", "approved", ""),
        )
        .with(
            "design/REQ-002.md",
            item(
                "REQ-002",
                "requirement",
                "Cold start",
                "draft",
                "parent = \"REQ-001\"\n",
            ),
        )
        .with(
            "design/REQ-003.md",
            item("REQ-003", "requirement", "Old idea", "obsolete", ""),
        )
        .with(
            "design/REQ-004.md",
            item("REQ-004", "requirement", "Small binary", "approved", ""),
        )
        .with(
            "design/TEST-001.md",
            item(
                "TEST-001",
                "test",
                "Timing",
                "ready",
                "\n[links]\nverifies = [\"REQ-002\"]\n",
            ),
        )
        .with(
            "design/MS-001.md",
            item(
                "MS-001",
                "milestone",
                "First release",
                "active",
                "due = 2026-11-30\n",
            ),
        )
        .with(
            "design/MS-002.md",
            item("MS-002", "milestone", "Later", "planned", ""),
        )
        .with(
            "design/TASK-001.md",
            item(
                "TASK-001",
                "task",
                "Write the loader",
                "done",
                "jira = \"FLINT-1\"\n\n[links]\npart-of = [\"MS-001\"]\n",
            ),
        )
        .with(
            "design/TASK-002.md",
            item(
                "TASK-002",
                "task",
                "Measure it",
                "todo",
                "\n[links]\npart-of = [\"MS-001\"]\n",
            ),
        )
        .with(
            "design/ADR-001.md",
            item("ADR-001", "decision", "YAML", "superseded", ""),
        )
        .with(
            "design/ADR-002.md",
            item(
                "ADR-002",
                "decision",
                "TOML",
                "accepted",
                "\n[links]\nsupersedes = [\"ADR-001\"]\n",
            ),
        )
        .with("src/lib.rs", format!("// {}: REQ-001\n", "implements"))
}

fn app() -> App {
    App::new(PathBuf::from("/p"), Project::load(&source()).unwrap(), None)
}

fn press(app: &mut App, keys: &str) -> Effect {
    let mut last = Effect::None;
    for c in keys.chars() {
        last = app.key(match c {
            '\n' => KeyPress::named(NamedKey::Enter),
            '\x1b' => KeyPress::named(NamedKey::Escape),
            '\t' => KeyPress::named(NamedKey::Tab),
            c => KeyPress::char(c),
        });
    }
    last
}

fn squash(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

#[test]
fn the_tree_nests_by_parent_and_hides_closed_items() {
    let mut a = app();
    assert_eq!(
        squash(a.text()),
        [
            "Requirements",
            "REQ-001 Fast start approved",
            "REQ-002 Cold start draft",
            "REQ-004 Small binary approved",
            "Decisions",
            "ADR-002 TOML accepted",
            "Milestones",
            "MS-001 First release active",
            "MS-002 Later planned",
            "Tasks",
            "TASK-001 Write the loader done",
            "TASK-002 Measure it todo",
            "Tests",
            "TEST-001 Timing ready",
        ]
    );
    assert!(
        a.text()[2].starts_with("  REQ-002"),
        "the child is indented"
    );
    press(&mut a, "a");
    assert!(squash(a.text())
        .iter()
        .any(|l| l.contains("REQ-003 Old idea")));
    let screen = a.screen().dump();
    assert!(screen.starts_with("1 [Tree]  2 Trace"), "{screen}");
    assert!(
        screen.contains("\n>REQ-001"),
        "the cursor starts on the first item, not the heading:\n{screen}"
    );
}

#[test]
fn moving_opening_and_filtering() {
    let mut a = app();
    press(&mut a, "jj");
    assert_eq!(
        press(&mut a, "\n"),
        Effect::Open {
            path: PathBuf::from("/p").join("design").join("REQ-004.md"),
            line: 4
        }
    );
    press(&mut a, "j");
    assert!(
        a.screen().dump().contains(">ADR-002"),
        "headings are skipped"
    );
    press(&mut a, "G");
    assert!(a.screen().dump().contains(">TEST-001"));
    press(&mut a, "gg");
    assert!(a.screen().dump().contains(">REQ-001"));
    // A filter keeps matches, the items they are nested under and their heading.
    press(&mut a, "/cold\n");
    assert_eq!(
        squash(a.text()),
        [
            "Requirements",
            "REQ-001 Fast start approved",
            "REQ-002 Cold start draft"
        ]
    );
    assert!(a.screen().dump().contains("-- /cold"));
    press(&mut a, "\x1b");
    assert_eq!(a.text().len(), 14);
}

#[test]
fn trace_marks_gaps_and_impact_follows_the_cursor() {
    let mut a = app();
    press(&mut a, " x");
    assert_eq!(a.page(), Page::Trace);
    let t = squash(a.text());
    assert_eq!(t[0], "Requirement implements verifies");
    assert_eq!(t[1], "REQ-001 Fast start src/lib.rs:1 through its children");
    assert_eq!(t[2], "REQ-002 Cold start gap TEST-001");
    press(&mut a, "m");
    assert_eq!(
        squash(a.text())[1..],
        [
            "REQ-002 Cold start gap TEST-001",
            "REQ-004 Small binary gap gap"
        ],
        "REQ-001 has none"
    );
    // From the Tree, `i` on REQ-001: what depends on it.
    press(&mut a, "1i");
    assert_eq!(a.page(), Page::Impact);
    assert_eq!(
        squash(a.text()),
        [
            "What depends on REQ-001 Fast start (3)",
            "REQ-002 Cold start draft ← child of REQ-001",
            "src/lib.rs:1 ← implements REQ-001",
            "TEST-001 Timing ready ← verifies REQ-002, child of REQ-001",
        ]
    );
    press(&mut a, "j");
    assert_eq!(
        press(&mut a, "\n"),
        Effect::Open {
            path: PathBuf::from("/p").join("src").join("lib.rs"),
            line: 1
        }
    );
}

#[test]
fn board_roadmap_and_decisions() {
    let mut a = app();
    press(&mut a, " b");
    assert_eq!(
        squash(a.text()),
        [
            "todo (1)",
            "TASK-002 Measure it todo under",
            "doing (0)",
            "blocked (0)",
            "done (1)",
            "TASK-001 Write the loader done"
        ]
        .map(|s| s.replace(" under", ""))
    );
    press(&mut a, " r");
    assert_eq!(
        squash(a.text()),
        [
            "Milestones",
            "MS-001 First release active due 2026-11-30 1/2 █████░░░░░",
            "TASK-001 Write the loader done",
            "TASK-002 Measure it todo",
            "MS-002 Later planned undated",
        ]
    );
    press(&mut a, " d");
    assert_eq!(
        squash(a.text()),
        [
            "Decision log",
            "ADR-001 YAML superseded superseded by ADR-002",
            "ADR-002 TOML accepted supersedes ADR-001"
        ]
    );
}

#[test]
fn changes_against_a_baseline_follow_reloads() {
    let old = Project::load(&source()).unwrap();
    let baseline = Baseline {
        label: "v1.0 (1a2b3c4)".into(),
        project: old,
    };
    let mut a = App::new(
        PathBuf::from("/p"),
        Project::load(&source()).unwrap(),
        Some(baseline),
    );
    press(&mut a, "7");
    assert_eq!(
        squash(a.text()),
        ["Since v1.0 (1a2b3c4): 0 added, 0 changed, 0 removed"]
    );
    let mut changed = source();
    changed.insert(
        "design/REQ-004.md",
        item("REQ-004", "requirement", "Small binary", "implemented", ""),
    );
    changed.insert(
        "design/REQ-005.md",
        item("REQ-005", "requirement", "New one", "draft", ""),
    );
    a.reload(Project::load(&changed).unwrap());
    assert_eq!(
        squash(a.text()),
        [
            "Since v1.0 (1a2b3c4): 1 added, 1 changed, 0 removed",
            "Added (1)",
            "REQ-005 New one (requirement)",
            "Changed (1)",
            "REQ-004 Small binary status approved → implemented",
        ]
    );
    let no_baseline = app();
    let mut n = no_baseline;
    press(&mut n, "7");
    assert!(n.text()[0].contains("kiln gui --since REV"));
}

#[test]
fn the_leader_shows_its_keys_and_reload_keeps_the_cursor() {
    let mut a = app();
    press(&mut a, " ");
    let popup = a.screen().dump();
    assert!(
        popup.contains("| t  Tree") && popup.contains("| q  Quit"),
        "{popup}"
    );
    assert_eq!(press(&mut a, "q"), Effect::Quit);
    press(&mut a, "jj");
    assert!(a.screen().dump().contains(">REQ-004"));
    // REQ-002 disappears: the cursor stays on REQ-004.
    let mut fewer = source();
    fewer.remove("design/REQ-002.md");
    a.reload(Project::load(&fewer).unwrap());
    assert!(
        a.screen().dump().contains(">REQ-004"),
        "{}",
        a.screen().dump()
    );
    assert!(a.select(&"TEST-001".parse().unwrap()));
    assert!(a.screen().dump().contains(">TEST-001"));
}

#[test]
fn tickets_show_once_the_jira_mirror_arrives() {
    let mut app = app();
    press(&mut app, "4");
    assert!(
        !app.text().join("\n").contains("FLINT-1"),
        "{:?}",
        app.text()
    );
    let mut mirror = kiln_jira::Mirror::default();
    mirror.tickets.insert(
        "FLINT-1".into(),
        kiln_jira::Ticket {
            key: "FLINT-1".into(),
            summary: "Loader".into(),
            status: "Done".into(),
            resolved: true,
            assignee: Some("Alice".into()),
        },
    );
    app.set_jira(mirror);
    let text = app.text();
    let line = text
        .iter()
        .find(|l| l.contains("TASK-001"))
        .cloned()
        .unwrap_or_default();
    assert!(line.ends_with("FLINT-1 Done · Alice"), "{text:?}");
}

fn ctrl(app: &mut App, c: char) -> Effect {
    app.key(KeyPress::char(c).with_ctrl())
}

fn id(s: &str) -> kiln_model::ItemId {
    s.parse().unwrap()
}

/// The line the cursor is on.
fn at(app: &App) -> String {
    app.screen()
        .dump()
        .lines()
        .find(|l| l.starts_with('>'))
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn edits_are_asked_for_checked_and_handed_to_the_window() {
    let mut a = app();
    // REQ-002: its status, picked from its type's statuses.
    press(&mut a, "j");
    press(&mut a, "s");
    let popup = a.screen().dump();
    assert!(popup.contains("-- Status of REQ-002:"), "{popup}");
    assert!(popup.contains("draft  (now)"), "{popup}");
    assert_eq!(
        press(&mut a, "approved\n"),
        Effect::Edit(Edit::Set {
            id: id("REQ-002"),
            key: "status".into(),
            value: "approved".into()
        })
    );
    // A bad date keeps the prompt open with the reason.
    press(&mut a, " ed");
    assert_eq!(press(&mut a, "soon\n"), Effect::None);
    let screen = a.screen().dump();
    assert!(screen.contains("isn't a date"), "{screen}");
    ctrl(&mut a, 'u');
    assert_eq!(
        press(&mut a, "2026-12-01\n"),
        Effect::Edit(Edit::Set {
            id: id("REQ-002"),
            key: "due".into(),
            value: "2026-12-01".into()
        })
    );
    // A link: the roles a test may start, then the items not yet linked.
    press(&mut a, "G");
    press(&mut a, "l");
    let roles = a.screen().dump();
    assert!(roles.contains("verifies  (to requirement)"), "{roles}");
    press(&mut a, "verifies\n");
    let targets = a.screen().dump();
    assert!(
        targets.contains("REQ-004 Small binary") && !targets.contains("REQ-002 Cold start"),
        "REQ-002 is already verified by TEST-001:\n{targets}"
    );
    assert_eq!(
        press(&mut a, "small\n"),
        Effect::Edit(Edit::Link {
            id: id("TEST-001"),
            role: "verifies".into(),
            target: id("REQ-004"),
            add: true
        })
    );
    // Unlinking asks first.
    press(&mut a, "U\n");
    assert!(a
        .screen()
        .dump()
        .contains("Remove `verifies REQ-002` from TEST-001? y/n"));
    assert_eq!(
        press(&mut a, "y"),
        Effect::Edit(Edit::Link {
            id: id("TEST-001"),
            role: "verifies".into(),
            target: id("REQ-002"),
            add: false
        })
    );
    // A child of REQ-001, then a renumbering after a confirm.
    press(&mut a, "gg");
    press(&mut a, "N");
    assert_eq!(
        press(&mut a, "requirement\nWarm start\n"),
        Effect::Edit(Edit::New {
            type_name: "requirement".into(),
            title: Some("Warm start".into()),
            parent: Some(id("REQ-001"))
        })
    );
    press(&mut a, " er");
    assert!(a.screen().dump().contains("Renumber REQ-001 to: REQ-005"));
    assert_eq!(press(&mut a, "\n"), Effect::None);
    assert_eq!(
        press(&mut a, "y"),
        Effect::Edit(Edit::Renumber {
            from: id("REQ-001"),
            to: id("REQ-005"),
            file: None
        })
    );
    // Esc leaves a prompt without an edit.
    press(&mut a, "s");
    assert_eq!(press(&mut a, "\x1b"), Effect::None);
    assert!(!a.screen().dump().contains("Status of"));
}

#[test]
fn the_board_moves_an_item_along_its_statuses() {
    let mut a = app();
    press(&mut a, "4");
    assert!(at(&a).contains("TASK-002"), "{}", a.screen().dump());
    assert_eq!(
        press(&mut a, "L"),
        Effect::Edit(Edit::Set {
            id: id("TASK-002"),
            key: "status".into(),
            value: "doing".into()
        })
    );
    assert_eq!(press(&mut a, "H"), Effect::None, "todo is the first status");
}

#[test]
fn the_item_page_shows_what_kiln_show_does_and_walks_the_graph() {
    let mut a = app();
    press(&mut a, "jv");
    assert_eq!(a.page(), Page::Item);
    let text = squash(a.text());
    assert_eq!(text[0], "REQ-002 Cold start");
    assert!(
        text.iter().any(|l| l == "parent REQ-001 Fast start"),
        "{text:?}"
    );
    assert!(
        text.iter().any(|l| l.ends_with("TEST-001 Timing")),
        "{text:?}"
    );
    // Down to TEST-001 and into it; C-o comes back, C-i goes forward.
    for _ in 0..20 {
        if at(&a).contains("TEST-001") {
            break;
        }
        press(&mut a, "j");
    }
    press(&mut a, "v");
    assert_eq!(squash(a.text())[0], "TEST-001 Timing");
    ctrl(&mut a, 'o');
    assert_eq!(squash(a.text())[0], "REQ-002 Cold start");
    ctrl(&mut a, 'i');
    assert_eq!(squash(a.text())[0], "TEST-001 Timing");
}

#[test]
fn check_findings_are_a_page_and_a_count_on_every_page() {
    let broken = source().with(
        "design/TEST-002.md",
        item(
            "TEST-002",
            "test",
            "Broken",
            "ready",
            "\n[links]\nverifies = [\"REQ-099\"]\n",
        ),
    );
    let project = Project::load(&broken).unwrap();
    let findings = kiln_check::check(&project, &broken, kiln_model::Date::from_unix_days(20_000));
    let mut a = App::new(PathBuf::from("/p"), project, None);
    a.set_findings(findings);
    let top = a.screen().dump();
    assert!(top.lines().next().unwrap().contains("1 error"), "{top}");
    press(&mut a, "9");
    assert_eq!(a.page(), Page::Check);
    let text = a.text().join("\n");
    assert!(text.contains("design/TEST-002.md"), "{text}");
    assert!(text.contains("K102 error"), "{text}");
    while !at(&a).contains("K102") {
        press(&mut a, "j");
    }
    match press(&mut a, "\n") {
        Effect::Open { path, .. } => assert!(path.ends_with("TEST-002.md"), "{path:?}"),
        other => panic!("{other:?}"),
    }
}

fn reports() -> Vec<ExportReport> {
    vec![
        ExportReport {
            name: "doc".into(),
            confluence: false,
            jira: false,
            lines: vec![
                "doc (markdown, document) → docs/DESIGN.md".into(),
                "  would update docs/DESIGN.md".into(),
            ],
            error: None,
        },
        ExportReport {
            name: "spec".into(),
            confluence: true,
            jira: false,
            lines: vec![
                "spec (confluence, document) → space FLINT, page 1 \"Flint\"".into(),
                "  update    \"Flint — design document\": REQ-014 changed (42 KiB)".into(),
                "  0 create, 1 update, 0 unchanged, 0 drift, 0 retire".into(),
            ],
            error: None,
        },
        ExportReport {
            name: "edited".into(),
            confluence: true,
            jira: false,
            lines: vec![
                "edited (confluence, document) → space FLINT, page 2 \"Other\"".into(),
                "  refused: \"Other\" was edited in Confluence by Alice".into(),
            ],
            error: None,
        },
    ]
}

#[test]
fn exports_are_planned_on_a_thread_and_published_only_after_y() {
    let mut a = app();
    press(&mut a, " p");
    assert_eq!(a.page(), Page::Exports);
    assert_eq!(a.next_job(), Some(Job::PlanExports));
    assert!(a.screen().dump().contains("Planning the exports…"));
    assert_eq!(a.next_job(), None, "one job at a time");
    a.done(Done::Exports(reports()));
    assert!(a.text()[0].starts_with("doc (markdown"));
    // w writes the file target under the cursor.
    press(&mut a, "w");
    assert_eq!(a.next_job(), Some(Job::WriteExports(vec!["doc".into()])));
    a.done(Done::Wrote(Ok(vec![
        "doc (markdown, document) → docs/DESIGN.md".into(),
        "  update docs/DESIGN.md".into(),
    ])));
    assert!(a.screen().dump().contains("-- update docs/DESIGN.md"));
    assert_eq!(a.next_job(), Some(Job::PlanExports), "planned again");
    a.done(Done::Exports(reports()));
    // P on the Confluence target asks; n publishes nothing.
    press(&mut a, "j");
    press(&mut a, "P");
    let ask = a.screen().dump();
    assert!(
        ask.contains("-- Publish spec (confluence, document) → space FLINT, page 1 \"Flint\"? y/n"),
        "{ask}"
    );
    press(&mut a, "n");
    assert_eq!(a.next_job(), None);
    press(&mut a, "Py");
    assert_eq!(
        a.next_job(),
        Some(Job::Publish {
            target: "spec".into(),
            shown: reports()[1].plan()
        })
    );
    a.done(Done::Published(Ok(vec![
        "spec: published 0 created, 1 updated, 0 retired, 0 pruned".into(),
    ])));
    assert!(a
        .screen()
        .dump()
        .contains("-- spec: published 0 created, 1 updated"));
    a.next_job();
    a.done(Done::Exports(reports()));
    // A refused plan is the command line's to override.
    press(&mut a, "G");
    press(&mut a, "P");
    let refused = a.screen().dump();
    assert!(refused.contains("kiln export edited --force"), "{refused}");
    assert_eq!(a.next_job(), None);
    // w on a Confluence target says what to press instead.
    press(&mut a, "w");
    assert!(a.screen().dump().contains("P publishes it"));
}

#[test]
fn comments_open_in_the_browser() {
    let mut a = app();
    press(&mut a, " m");
    assert_eq!(a.next_job(), Some(Job::Comments));
    a.done(Done::Comments(Ok(vec![CommentGroup {
        target: "spec".into(),
        space: "FLINT".into(),
        published: true,
        comments: vec![Comment {
            item: Some("REQ-001".into()),
            page_title: "Flint".into(),
            author: "Alice Martin".into(),
            date: "2026-10-12".into(),
            text: "Is that measured cold?".into(),
            url: "https://confluence.example.com/c/456".into(),
        }],
    }])));
    let text = squash(a.text());
    assert_eq!(text[0], "spec → space FLINT: 1 unresolved comment");
    assert_eq!(
        text[1],
        "REQ-001 Alice Martin, 2026-10-12: Is that measured cold?"
    );
    assert_eq!(
        press(&mut a, "\n"),
        Effect::OpenUrl("https://confluence.example.com/c/456".into())
    );
    press(&mut a, "v");
    assert_eq!(squash(a.text())[0], "REQ-001 Fast start");
}

#[test]
fn another_baseline_is_picked_from_the_tags() {
    let mut a = app();
    press(&mut a, "7b");
    assert_eq!(a.next_job(), Some(Job::Tags));
    a.done(Done::Tags(Ok(vec!["v1.1".into(), "v1.0".into()])));
    assert!(a.screen().dump().contains("| > v1.1"));
    press(&mut a, "1.0\n");
    assert_eq!(a.next_job(), Some(Job::Baseline("v1.0".into())));
    let mut old = source();
    old.remove("design/REQ-004.md");
    a.done(Done::Baseline(Ok(Box::new(Baseline {
        label: "v1.0 (1a2b3c4)".into(),
        project: Project::load(&old).unwrap(),
    }))));
    assert!(
        a.text()[0].starts_with("Since v1.0 (1a2b3c4): 1 added"),
        "{:?}",
        a.text()
    );
}

#[test]
// verifies: REQ-013
fn edits_go_to_disk_the_way_the_commands_write_them() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("kiln.toml"), kiln_model::default_config("Demo")).unwrap();
    std::fs::create_dir_all(root.join("design")).unwrap();
    let req = "+++\r\nid = \"REQ-001\" # keep me\r\ntype = \"requirement\"\r\ntitle = \"Fast start\"\r\nstatus = \"draft\"\r\n+++\r\n\r\nThe body.\r\n";
    std::fs::write(root.join("design/REQ-001.md"), req).unwrap();
    std::fs::write(
        root.join("design/TEST-001.md"),
        item("TEST-001", "test", "Timing", "ready", ""),
    )
    .unwrap();

    let set = crate::edit::apply(
        root,
        &Edit::Set {
            id: id("REQ-001"),
            key: "status".into(),
            value: "approved".into(),
        },
    )
    .unwrap();
    assert_eq!(set.message, "REQ-001: status draft → approved");
    assert_eq!(
        std::fs::read_to_string(root.join("design/REQ-001.md")).unwrap(),
        req.replace("\"draft\"", "\"approved\""),
        "only the status changed: the comment, CRLF and body are kept"
    );

    let refused = crate::edit::apply(
        root,
        &Edit::Set {
            id: id("REQ-001"),
            key: "status".into(),
            value: "nonsense".into(),
        },
    )
    .unwrap_err();
    assert!(
        refused.starts_with("`nonsense` isn't a requirement status"),
        "{refused}"
    );

    let link = crate::edit::apply(
        root,
        &Edit::Link {
            id: id("TEST-001"),
            role: "verifies".into(),
            target: id("REQ-001"),
            add: true,
        },
    )
    .unwrap();
    assert_eq!(link.message, "TEST-001 verifies REQ-001");

    let new = crate::edit::apply(
        root,
        &Edit::New {
            type_name: "requirement".into(),
            title: Some("Warm start".into()),
            parent: Some(id("REQ-001")),
        },
    )
    .unwrap();
    assert_eq!(new.item, Some(id("REQ-002")));
    let (path, line) = new.created.unwrap();
    assert!(path.is_file() && line > 0, "{path:?}");

    let renumbered = crate::edit::apply(
        root,
        &Edit::Renumber {
            from: id("REQ-001"),
            to: id("REQ-010"),
            file: None,
        },
    )
    .unwrap();
    assert!(
        renumbered.message.starts_with("REQ-001 → REQ-010"),
        "{}",
        renumbered.message
    );
    let test = std::fs::read_to_string(root.join("design/TEST-001.md")).unwrap();
    assert!(test.contains("REQ-010"), "{test}");
}

#[test]
fn the_check_page_says_when_a_check_is_still_running() {
    let mut a = app();
    a.set_checking();
    press(&mut a, "9");
    assert_eq!(
        squash(a.text()),
        [
            "kiln check: 0 errors, 0 warnings (checking again…)",
            "Running kiln check…"
        ]
    );
    a.set_findings(Vec::new());
    assert_eq!(
        squash(a.text()),
        ["kiln check: 0 errors, 0 warnings", "No findings."]
    );
}

#[test]
fn jira_syncs_from_exports_and_pulls_a_status_from_any_item() {
    let mut a = app();
    press(&mut a, " p");
    assert_eq!(a.next_job(), Some(Job::PlanExports));
    let jira = ExportReport {
        name: "jira".into(),
        confluence: false,
        jira: true,
        lines: vec![
            "jira (DEMO) → DEMO-100 \"Kiln: Demo\"".into(),
            "  move       DEMO-101 TASK-001: To Do → Done".into(),
            "  0 create, 0 update, 1 move, 2 unchanged".into(),
        ],
        error: None,
    };
    a.done(Done::Exports(vec![jira.clone()]));
    press(&mut a, "w");
    assert!(a
        .screen()
        .dump()
        .contains("isn't a file target: P publishes it"));
    press(&mut a, "P");
    assert!(a
        .screen()
        .dump()
        .contains("-- Publish jira (DEMO) → DEMO-100 \"Kiln: Demo\"? y/n"));
    press(&mut a, "y");
    assert_eq!(
        a.next_job(),
        Some(Job::Publish {
            target: "jira".into(),
            shown: jira.plan()
        })
    );
    a.done(Done::Published(Ok(vec![
        "jira: 0 created, 0 updated, 1 moved".into(),
    ])));
    assert!(a
        .screen()
        .dump()
        .contains("-- jira: 0 created, 0 updated, 1 moved"));

    // J on an item takes its ticket's status.
    press(&mut a, "1");
    a.next_job();
    press(&mut a, "J");
    assert_eq!(a.next_job(), Some(Job::JiraPull("REQ-001".into())));
    a.done(Done::Pulled(Err(
        "REQ-001 has no ticket under the project issue".into(),
    )));
    assert!(a
        .screen()
        .dump()
        .contains("-- REQ-001 has no ticket under the project issue"));
}
