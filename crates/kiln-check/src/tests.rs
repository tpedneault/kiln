//! One small fixture project per rule (DESIGN.md, "Testing"): each test
//! builds the least project that triggers its rule and asserts exactly
//! the findings, as `path:line:col code`.

use kiln_index::{MemorySource, Project};
use kiln_model::Date;

use crate::check;

const TODAY: Date = Date {
    year: 2026,
    month: 10,
    day: 13,
};

fn item(id: &str, ty: &str, status: &str, rest: &str) -> String {
    let (fm, body) = rest.split_once("+++").unwrap_or((rest, ""));
    format!("+++\nid = \"{id}\"\ntype = \"{ty}\"\ntitle = \"{id} title\"\nstatus = \"{status}\"\n{fm}+++\n{body}")
}

/// Runs the checks on `kiln.toml` defaults plus `extra_config` and the
/// files, returning `path:line:col code` per finding.
fn run(extra_config: &str, files: &[(&str, String)]) -> Vec<String> {
    let mut source = MemorySource::new().with(
        "kiln.toml",
        format!("{}{extra_config}", kiln_model::default_config("Test")),
    );
    for (path, text) in files {
        source.insert(path, text);
    }
    let project = Project::load(&source).unwrap();
    check(&project, &source, TODAY)
        .iter()
        .map(|f| format!("{}:{}:{} {}", f.path, f.line, f.column, f.rule))
        .collect()
}

/// The full text of each finding, for checking messages.
fn messages(files: &[(&str, String)]) -> Vec<String> {
    let mut source = MemorySource::new().with("kiln.toml", kiln_model::default_config("Test"));
    for (path, text) in files {
        source.insert(path, text);
    }
    let project = Project::load(&source).unwrap();
    check(&project, &source, TODAY)
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn req(id: &str, status: &str, rest: &str) -> (String, String) {
    (
        format!("design/{id}.md"),
        item(id, "requirement", status, rest),
    )
}

fn files(list: Vec<(String, String)>) -> Vec<(&'static str, String)> {
    list.into_iter()
        .map(|(p, t)| (&*Box::leak(p.into_boxed_str()), t))
        .collect()
}

fn none() -> Vec<String> {
    Vec::new()
}

#[test]
fn a_clean_project_has_no_findings() {
    let f = files(vec![
        req("REQ-001", "draft", ""),
        (
            "design/TEST-001.md".into(),
            item(
                "TEST-001",
                "test",
                "passing",
                "[links]\nverifies = [\"REQ-001\"]\n",
            ),
        ),
    ]);
    assert_eq!(run("", &f), none());
}

#[test]
fn k002_k003_k004_come_from_the_configuration() {
    // Lines after the defaults: the extra configuration starts on the
    // line after their last one.
    let base = kiln_model::default_config("Test").lines().count() as u32;
    let got = run(
        "\n[extras]\nx = 1\n\n[jira]\nurl = \"https://j\"\npassword = \"p\"\n",
        &files(vec![req("REQ-001", "draft", "")]),
    );
    let at = |line: u32, col: u32, code: &str| format!("kiln.toml:{}:{col} {code}", base + line);
    assert_eq!(got, [at(2, 2, "K003"), at(7, 1, "K003"), at(7, 1, "K004")]);
    let got = run(
        "\n[check]\nK000 = \"off\"\n",
        &files(vec![req("REQ-001", "draft", "")]),
    );
    assert_eq!(got, [at(3, 1, "K002")]);
}

#[test]
fn k100_unparseable_front_matter() {
    let f = files(vec![(
        "design/REQ-001.md".into(),
        "+++\nid = \"REQ-001\nx\n+++\n".into(),
    )]);
    assert_eq!(run("", &f), ["design/REQ-001.md:2:14 K100"]);
}

#[test]
// verifies: REQ-015
fn k101_duplicates_name_each_other() {
    let f = files(vec![
        (
            "design/REQ-001-a.md".into(),
            item("REQ-001", "requirement", "draft", ""),
        ),
        (
            "design/REQ-001-b.md".into(),
            item("REQ-001", "requirement", "draft", ""),
        ),
    ]);
    assert_eq!(
        run("", &f),
        [
            "design/REQ-001-a.md:2:7 K101",
            "design/REQ-001-b.md:2:7 K101"
        ]
    );
    let m = messages(&f);
    assert!(
        m[0].contains("is also the ID of design/REQ-001-b.md"),
        "{}",
        m[0]
    );
    assert!(
        m[0].contains("kiln renumber REQ-001 --to REQ-002 --file design/REQ-001-a.md"),
        "{}",
        m[0]
    );
}

#[test]
fn k102_missing_and_misspelt_references_everywhere() {
    let f = files(vec![
        req(
            "REQ-001",
            "draft",
            "parent = \"REQ-009\"\n+++\nSee [[REQ-008]], REQ-7 and `REQ-006`.\n",
        ),
        (
            "design/TEST-001.md".into(),
            item(
                "TEST-001",
                "test",
                "draft",
                "[links]\nverifies = [\"REQ-005\"]\n",
            ),
        ),
        ("src/lib.rs".into(), format!("// {}: REQ-004\n", "verifies")),
    ]);
    let got = run("\n[code]\npaths = [\"src\"]\n", &f);
    assert_eq!(
        got,
        [
            "design/REQ-001.md:6:11 K102",
            "design/REQ-001.md:9:7 K102",
            "design/REQ-001.md:9:18 K102",
            "design/TEST-001.md:7:14 K102",
            "src/lib.rs:1:14 K102"
        ]
    );
    let m = messages(&f[..1]);
    assert!(
        m[2].contains("`REQ-7` isn't how IDs are written: REQ-007"),
        "{}",
        m[2]
    );
}

#[test]
fn k103_parent_cycles_on_each_member() {
    let f = files(vec![
        req("REQ-001", "draft", "parent = \"REQ-002\"\n"),
        req("REQ-002", "draft", "parent = \"REQ-001\"\n"),
    ]);
    assert_eq!(
        run("", &f),
        ["design/REQ-001.md:6:11 K103", "design/REQ-002.md:6:11 K103"]
    );
    assert!(messages(&f)[0].ends_with("parent cycle: REQ-001 → REQ-002 → REQ-001"));
}

#[test]
fn k104_unknown_type_and_keys() {
    let f = files(vec![
        (
            "design/REQ-001.md".into(),
            item("REQ-001", "epic", "draft", ""),
        ),
        (
            "design/REQ-002.md".into(),
            item("REQ-002", "requirement", "draft", "colour = \"red\"\n"),
        ),
    ]);
    assert_eq!(
        run("", &f),
        ["design/REQ-001.md:3:8 K104", "design/REQ-002.md:6:1 K104"]
    );
}

#[test]
fn k105_file_name_and_k106_prefix() {
    let f = files(vec![(
        "design/requirement.md".into(),
        item("DES-001", "requirement", "draft", ""),
    )]);
    assert_eq!(
        run("", &f),
        [
            "design/requirement.md:2:7 K105",
            "design/requirement.md:2:7 K106"
        ]
    );
}

#[test]
fn k107_status_not_allowed() {
    let f = files(vec![(
        "design/TASK-001.md".into(),
        item("TASK-001", "task", "finished", ""),
    )]);
    assert_eq!(run("", &f), ["design/TASK-001.md:5:10 K107"]);
}

#[test]
fn k108_disallowed_roles_targets_and_parents() {
    let f = files(vec![
        req(
            "REQ-001",
            "draft",
            "parent = \"TEST-001\"\n\n[links]\nverifies = [\"REQ-002\"]\ncolours = [\"REQ-002\"]\n",
        ),
        req("REQ-002", "draft", ""),
        (
            "design/TEST-001.md".into(),
            item(
                "TEST-001",
                "test",
                "draft",
                "[links]\nverifies = [\"TEST-001\"]\n",
            ),
        ),
        (
            "src/lib.rs".into(),
            format!("// {}: REQ-002\n", "implements"),
        ),
    ]);
    let got = run("\n[code]\npaths = [\"src\"]\npatterns = ['(?:implements|verifies|satisfies):\\s*(?P<ids>[A-Z]+-\\d+)']\n", &f);
    assert_eq!(
        got,
        [
            "design/REQ-001.md:6:11 K108",
            "design/REQ-001.md:9:1 K108",
            "design/REQ-001.md:10:1 K108",
            "design/TEST-001.md:7:14 K108"
        ]
    );
    let f = files(vec![
        req("REQ-001", "draft", ""),
        (
            "src/lib.rs".into(),
            format!("// {}: REQ-001\n", "satisfies"),
        ),
    ]);
    let got = run(
        "\n[code]\npaths = [\"src\"]\npatterns = ['(?:satisfies):\\s*(?P<ids>[A-Z]+-\\d+)']\n",
        &f,
    );
    assert_eq!(got, ["src/lib.rs:1:4 K108"]);
}

#[test]
fn k109_link_on_the_inverse_side() {
    let f = files(vec![
        req(
            "REQ-001",
            "draft",
            "[links]\nverified-by = [\"TEST-001\"]\n",
        ),
        (
            "design/TEST-001.md".into(),
            item("TEST-001", "test", "draft", ""),
        ),
    ]);
    assert_eq!(run("", &f), ["design/REQ-001.md:7:1 K109"]);
    assert!(messages(&f)[0].contains("write `verifies = [\"REQ-001\"]` in the other item"));
}

#[test]
fn k110_relative_links_must_exist() {
    let f = files(vec![
        req(
            "REQ-001",
            "draft",
            "+++\n![a](img/a.png) ![b](img/b.png) [c](../../x.md) [d](/docs/d.md)\n",
        ),
        ("design/img/a.png".into(), String::new()),
        ("docs/d.md".into(), String::new()),
    ]);
    assert_eq!(
        run("", &f),
        ["design/REQ-001.md:8:22 K110", "design/REQ-001.md:8:37 K110"]
    );
}

#[test]
fn k201_k202_requirements_without_coverage() {
    let f = files(vec![
        req("REQ-001", "approved", ""),
        req("REQ-002", "draft", ""),
        req("REQ-003", "obsolete", ""),
        req("REQ-004", "approved", ""),
        (
            "src/lib.rs".into(),
            format!("// {}: REQ-004\n// {}: REQ-004\n", "implements", "verifies"),
        ),
    ]);
    assert_eq!(
        run("\n[code]\npaths = [\"src\"]\n", &f),
        ["design/REQ-001.md:2:7 K201", "design/REQ-001.md:2:7 K202"]
    );
}

#[test]
fn k203_links_to_closed_items() {
    let f = files(vec![
        req("REQ-001", "obsolete", ""),
        (
            "design/TEST-001.md".into(),
            item(
                "TEST-001",
                "test",
                "ready",
                "[links]\nverifies = [\"REQ-001\"]\n",
            ),
        ),
        (
            "design/TEST-002.md".into(),
            item(
                "TEST-002",
                "test",
                "obsolete",
                "[links]\nverifies = [\"REQ-001\"]\n",
            ),
        ),
    ]);
    assert_eq!(run("", &f), ["design/TEST-001.md:7:14 K203"]);
}

#[test]
fn k204_superseded_by_nothing() {
    let f = files(vec![
        (
            "design/ADR-001.md".into(),
            item("ADR-001", "decision", "superseded", ""),
        ),
        (
            "design/ADR-002.md".into(),
            item("ADR-002", "decision", "superseded", ""),
        ),
        (
            "design/ADR-003.md".into(),
            item(
                "ADR-003",
                "decision",
                "accepted",
                "[links]\nsupersedes = [\"ADR-002\"]\n",
            ),
        ),
    ]);
    assert_eq!(
        run("", &f),
        ["design/ADR-001.md:5:10 K204", "design/ADR-003.md:7:16 K203"]
    );
}

#[test]
fn k205_verified_without_a_passing_source() {
    let f = files(vec![
        req("REQ-001", "verified", ""),
        req("REQ-002", "verified", ""),
        (
            "design/TEST-001.md".into(),
            item(
                "TEST-001",
                "test",
                "failing",
                "[links]\nverifies = [\"REQ-001\"]\n",
            ),
        ),
        (
            "design/TEST-002.md".into(),
            item(
                "TEST-002",
                "test",
                "passing",
                "[links]\nverifies = [\"REQ-002\"]\n",
            ),
        ),
    ]);
    let got = run("\n[check]\nK202 = \"off\"\n", &f);
    assert_eq!(got, ["design/REQ-001.md:5:10 K205"]);
}

#[test]
fn k206_overdue_milestones() {
    let f = files(vec![
        (
            "design/MS-001.md".into(),
            item("MS-001", "milestone", "active", "due = 2026-10-01\n"),
        ),
        (
            "design/MS-002.md".into(),
            item("MS-002", "milestone", "done", "due = 2026-10-01\n"),
        ),
        (
            "design/MS-003.md".into(),
            item("MS-003", "milestone", "active", "due = 2026-10-13\n"),
        ),
    ]);
    assert_eq!(run("", &f), ["design/MS-001.md:2:7 K206"]);
}

#[test]
fn severities_are_overridable() {
    let f = files(vec![req("REQ-001", "approved", "")]);
    let got = run("\n[check]\nK201 = \"error\"\nK202 = \"off\"\n", &f);
    assert_eq!(got, ["design/REQ-001.md:2:7 K201"]);
    let mut source = MemorySource::new().with(
        "kiln.toml",
        format!(
            "{}\n[check]\nK201 = \"error\"\n",
            kiln_model::default_config("T")
        ),
    );
    for (p, t) in &f {
        source.insert(p, t);
    }
    let project = Project::load(&source).unwrap();
    let findings = check(&project, &source, TODAY);
    assert!(crate::has_errors(&findings));
}

#[test]
fn k111_diagrams_that_dont_draw() {
    let f = files(vec![req("REQ-001", "draft", "+++\n\n```mermaid\nflowchart TD\n  a --> b\n```\n\n```mermaid\nflowchart TD\n  a -->\n```\n")]);
    assert_eq!(run("", &f), ["design/REQ-001.md:16:3 K111"]);
}

#[test]
fn k112_templates_and_k005_targets() {
    let config = "\n[[export]]\nname = \"spec\"\nformat = \"markdown\"\npath = \"out/spec.md\"\ntemplate = \"design/templates/spec.md\"\n\n[[export]]\nname = \"other\"\nformat = \"html\"\npath = \"out/o.html\"\ntemplate = \"design/templates/missing.md\"\n";
    let f = files(vec![
        req("REQ-001", "draft", ""),
        ("design/templates/spec.md".into(), "# Requirements\n\n<!-- kiln:items type=\"requirement\" -->\n<!-- kiln:items type=\"epic\" -->\n".into()),
    ]);
    assert_eq!(
        run(config, &f),
        ["design/templates/spec.md:4:1 K112", "kiln.toml:1:1 K112"]
    );
}
