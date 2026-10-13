//! The pages and the keys, over a small in-memory project, read as text
//! (GUI.md).

use std::path::PathBuf;

use fenix_keymap::{KeyPress, NamedKey};
use kiln_index::{MemorySource, Project};

use crate::app::{App, Baseline, Effect};
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
