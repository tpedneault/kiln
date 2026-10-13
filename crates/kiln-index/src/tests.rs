//! Graph queries over small in-memory projects.

use kiln_model::ItemId;

use crate::{plan_renumber, MemorySource, Project, Source};

pub(crate) fn id(s: &str) -> ItemId {
    s.parse().unwrap()
}

/// An item file. `rest` is extra front-matter lines and, after a `+++`,
/// body lines.
pub(crate) fn item(id: &str, ty: &str, status: &str, rest: &str) -> String {
    let (fm, body) = rest.split_once("+++").unwrap_or((rest, ""));
    format!("+++\nid = \"{id}\"\ntype = \"{ty}\"\ntitle = \"{id} title\"\nstatus = \"{status}\"\n{fm}+++\n{body}")
}

/// A code comment making a code reference, assembled at run time so this
/// file holds none for Kiln's own check.
fn code_ref(role: &str, ids: &str) -> String {
    format!("// {role}: {ids}\n")
}

fn config() -> String {
    format!(
        "{}\n[code]\npaths = [\"src\"]\nexclude = [\"src/fixtures\"]\n",
        kiln_model::default_config("Test")
    )
}

fn project() -> Project {
    Project::load(&source()).unwrap()
}

fn source() -> MemorySource {
    MemorySource::new()
        .with("kiln.toml", config())
        .with(
            "design/REQ-001.md",
            item("REQ-001", "requirement", "approved", ""),
        )
        .with(
            "design/REQ-002.md",
            item(
                "REQ-002",
                "requirement",
                "approved",
                "parent = \"REQ-001\"\n",
            ),
        )
        .with(
            "design/REQ-003.md",
            item(
                "REQ-003",
                "requirement",
                "approved",
                "parent = \"REQ-001\"\n",
            ),
        )
        .with(
            "design/REQ-004.md",
            item(
                "REQ-004",
                "requirement",
                "obsolete",
                "parent = \"REQ-001\"\n",
            ),
        )
        .with(
            "design/TEST-001.md",
            item(
                "TEST-001",
                "test",
                "passing",
                "[links]\nverifies = [\"REQ-002\"]\n+++\nSee [[REQ-003]] and REQ-001.\n",
            ),
        )
        .with(
            "design/TEST-002.md",
            item(
                "TEST-002",
                "test",
                "obsolete",
                "[links]\nverifies = [\"REQ-003\"]\n",
            ),
        )
        .with(
            "design/MS-001.md",
            item(
                "MS-001",
                "milestone",
                "active",
                "[links]\ndepends-on = [\"MS-002\"]\n",
            ),
        )
        .with(
            "design/MS-002.md",
            item("MS-002", "milestone", "planned", ""),
        )
        .with(
            "design/TASK-001.md",
            item(
                "TASK-001",
                "task",
                "done",
                "[links]\npart-of = [\"MS-001\"]\n",
            ),
        )
        .with(
            "design/TASK-002.md",
            item(
                "TASK-002",
                "task",
                "todo",
                "[links]\npart-of = [\"MS-002\"]\n",
            ),
        )
        .with(
            "design/TASK-003.md",
            item(
                "TASK-003",
                "task",
                "dropped",
                "[links]\npart-of = [\"MS-001\"]\n",
            ),
        )
        .with("design/README.md", "# Not an item\n")
        .with(
            "src/lib.rs",
            format!("fn a() {{}}\n{}", code_ref("verifies", "REQ-003")),
        )
        .with("src/fixtures/f.rs", code_ref("verifies", "REQ-999"))
}

#[test]
fn loads_items_in_type_order_and_code_refs() {
    let p = project();
    assert_eq!(p.findings, vec![]);
    let ids: Vec<String> = p.items.iter().map(|i| i.id.to_string()).collect();
    assert_eq!(
        ids,
        [
            "REQ-001", "REQ-002", "REQ-003", "REQ-004", "MS-001", "MS-002", "TASK-001", "TASK-002",
            "TASK-003", "TEST-001", "TEST-002"
        ]
    );
    assert_eq!(p.code_refs.len(), 1, "fixtures are excluded");
    assert_eq!(p.code_refs[0].location(), "src/lib.rs:2");
}

#[test]
fn inverse_links_children_and_mentions() {
    let p = project();
    let inbound: Vec<String> = p
        .inbound(&id("REQ-003"))
        .iter()
        .map(|l| format!("{} {}", l.role, p.describe(l.source)))
        .collect();
    assert_eq!(inbound, ["verifies TEST-002", "verifies src/lib.rs:2"]);
    let children: Vec<String> = p
        .children(&id("REQ-001"))
        .iter()
        .map(|i| i.id.to_string())
        .collect();
    assert_eq!(children, ["REQ-002", "REQ-003", "REQ-004"]);
    assert_eq!(p.ancestors(&id("REQ-002"))[0].id, id("REQ-001"));
    let mentioned: Vec<String> = p
        .mentioned_by(&id("REQ-001"))
        .iter()
        .map(|i| i.id.to_string())
        .collect();
    assert_eq!(mentioned, ["TEST-001"]);
}

#[test]
fn coverage_counts_open_sources_and_complete_children() {
    let p = project();
    assert!(p.covered(&id("REQ-002"), "verifies"), "by a passing test");
    assert!(
        p.covered(&id("REQ-003"), "verifies"),
        "by code, though its test is obsolete"
    );
    assert!(
        p.covered(&id("REQ-001"), "verifies"),
        "all open children are; REQ-004 is closed"
    );
    assert!(!p.covered(&id("REQ-001"), "implements"));
}

#[test]
fn progress_follows_depends_on_and_skips_closed() {
    let p = project();
    let ms1 = p.progress(&id("MS-001")).unwrap();
    assert_eq!(
        (ms1.done, ms1.total),
        (1, 2),
        "TASK-001 done, TASK-002 via MS-002, TASK-003 dropped"
    );
    assert_eq!(p.progress(&id("TASK-001")), None);
}

#[test]
fn impact_is_breadth_first_with_paths() {
    let p = project();
    let lines: Vec<String> = p
        .impact(&id("REQ-001"))
        .iter()
        .map(|i| {
            let path: Vec<String> = i
                .path
                .iter()
                .map(|s| format!("{} {}", s.relation, s.to))
                .collect();
            format!("{}: {}", p.describe(i.source), path.join(", "))
        })
        .collect();
    assert_eq!(
        lines,
        [
            "REQ-002: parent REQ-001",
            "REQ-003: parent REQ-001",
            "REQ-004: parent REQ-001",
            "TEST-001: verifies REQ-002, parent REQ-001",
            "TEST-002: verifies REQ-003, parent REQ-001",
            "src/lib.rs:2: verifies REQ-003, parent REQ-001",
        ]
    );
    assert!(matches!(
        p.impact(&id("REQ-003"))[1].source,
        Source::Code(0)
    ));
}

#[test]
fn next_id_is_above_the_highest_including_closed() {
    let p = project();
    assert_eq!(p.next_id("REQ").to_string(), "REQ-005");
    assert_eq!(p.next_id("RISK").to_string(), "RISK-001");
}

#[test]
fn renumber_rewrites_every_reference_and_the_file_name() {
    let p = project();
    let plan = plan_renumber(&p, &id("REQ-003"), &id("REQ-010"), None).unwrap();
    assert_eq!(plan.new_path.as_deref(), Some("design/REQ-010.md"));
    let files: Vec<(&str, usize, bool)> = plan
        .files
        .iter()
        .map(|(k, v)| (k.as_str(), v.edits.len(), v.code))
        .collect();
    assert_eq!(
        files,
        [
            ("design/REQ-003.md", 1, false),
            ("design/TEST-001.md", 1, false),
            ("design/TEST-002.md", 1, false),
            ("src/lib.rs", 1, true)
        ]
    );
    assert!(plan.left_alone.is_empty());
    assert!(plan_renumber(&p, &id("REQ-003"), &id("REQ-002"), None)
        .unwrap_err()
        .contains("already exists"));
    assert!(plan_renumber(&p, &id("REQ-003"), &id("DES-001"), None)
        .unwrap_err()
        .contains("another prefix"));
}

#[test]
// verifies: REQ-015
fn renumbering_a_duplicate_moves_one_and_lists_references() {
    let source = MemorySource::new()
        .with("kiln.toml", config())
        .with(
            "design/REQ-001-a.md",
            item("REQ-001", "requirement", "draft", ""),
        )
        .with(
            "design/REQ-001-b.md",
            item("REQ-001", "requirement", "draft", ""),
        )
        .with(
            "design/TEST-001.md",
            item(
                "TEST-001",
                "test",
                "draft",
                "[links]\nverifies = [\"REQ-001\"]\n",
            ),
        );
    let p = Project::load(&source).unwrap();
    assert!(plan_renumber(&p, &id("REQ-001"), &id("REQ-002"), None)
        .unwrap_err()
        .contains("pick one with --file"));
    let plan = plan_renumber(
        &p,
        &id("REQ-001"),
        &id("REQ-002"),
        Some("design/REQ-001-b.md"),
    )
    .unwrap();
    assert_eq!(
        plan.files.keys().collect::<Vec<_>>(),
        ["design/REQ-001-b.md"]
    );
    assert_eq!(plan.new_path.as_deref(), Some("design/REQ-002-b.md"));
    assert_eq!(
        plan.left_alone,
        ["design/TEST-001.md:7 (may mean either REQ-001)"]
    );
}

#[test]
fn a_missing_folder_and_a_broken_config_are_reported() {
    let source = MemorySource::new().with("kiln.toml", kiln_model::default_config("x"));
    let p = Project::load(&source).unwrap();
    assert_eq!(p.findings[0].message, "paths: `design` doesn't exist");
    let source = MemorySource::new().with("kiln.toml", "format = \n");
    assert!(matches!(
        Project::load(&source),
        Err(crate::LoadError::Config(_))
    ));
    assert!(matches!(
        Project::load(&MemorySource::new()),
        Err(crate::LoadError::Read(_))
    ));
}

/// Everything a query sees, for comparing an updated project with a
/// fresh load.
fn summary(p: &Project) -> Vec<String> {
    let mut out: Vec<String> = p
        .items
        .iter()
        .map(|i| format!("{} {} {}", i.path, i.id, i.title))
        .collect();
    out.extend(
        p.code_refs
            .iter()
            .map(|r| format!("{} {}", r.location(), r.target.raw)),
    );
    out.extend(p.findings.iter().map(ToString::to_string));
    let mut ids: Vec<&ItemId> = p.ids().collect();
    ids.sort();
    for id in ids {
        let inbound: Vec<String> = p
            .inbound(id)
            .iter()
            .map(|l| format!("{} {}", l.role, p.describe(l.source)))
            .collect();
        let children: Vec<String> = p.children(id).iter().map(|i| i.id.to_string()).collect();
        let mentions: Vec<String> = p
            .mentioned_by(id)
            .iter()
            .map(|i| i.id.to_string())
            .collect();
        out.push(format!(
            "{id}: in {inbound:?} children {children:?} mentioned {mentions:?}"
        ));
    }
    out
}

#[test]
// verifies: REQ-008
fn updating_one_file_equals_loading_again() {
    let mut src = source();
    let mut p = Project::load(&src).unwrap();
    let edits: Vec<(&str, Option<String>)> = vec![
        // A link moves, a mention appears.
        (
            "design/TEST-001.md",
            Some(item(
                "TEST-001",
                "test",
                "passing",
                "[links]\nverifies = [\"REQ-004\"]\n+++\nNow [[REQ-002]].\n",
            )),
        ),
        // A new item, a broken one, a deleted one.
        (
            "design/REQ-005.md",
            Some(item(
                "REQ-005",
                "requirement",
                "draft",
                "parent = \"REQ-002\"\n",
            )),
        ),
        (
            "design/REQ-002.md",
            Some("+++\nid = \"REQ-002\"\ntitle = \n+++\n".to_owned()),
        ),
        ("design/TASK-003.md", None),
        // Code changes, and a file outside every path is ignored.
        (
            "src/lib.rs",
            Some(code_ref("implements", "REQ-001, REQ-005")),
        ),
        (
            "notes/x.md",
            Some(item("REQ-077", "requirement", "draft", "")),
        ),
        // A README becomes an item.
        (
            "design/README.md",
            Some(item("REQ-006", "requirement", "draft", "")),
        ),
    ];
    for (path, text) in edits {
        match &text {
            Some(t) => src.insert(path, t),
            None => src.remove(path),
        }
        p.update_file(path, text.as_deref().map(str::as_bytes));
        let fresh = Project::load(&src).unwrap();
        assert_eq!(summary(&p), summary(&fresh), "after {path}");
    }
    assert!(p.item(&id("REQ-077")).is_none());
}
