//! Reports and template validation over a small in-memory project.

use kiln_index::{MemorySource, Project};

use crate::{plain, report, template_problems, Block, Inline, ReportOptions, Tone};

fn item(id: &str, ty: &str, status: &str, rest: &str) -> String {
    format!("+++\nid = \"{id}\"\ntype = \"{ty}\"\ntitle = \"{id} title\"\nstatus = \"{status}\"\n{rest}+++\n")
}

fn project() -> Project {
    let source = MemorySource::new()
        .with(
            "kiln.toml",
            format!(
                "{}\n[code]\npaths = [\"src\"]\n",
                kiln_model::default_config("T")
            ),
        )
        .with(
            "design/REQ-001.md",
            item("REQ-001", "requirement", "approved", ""),
        )
        .with(
            "design/REQ-002.md",
            item("REQ-002", "requirement", "verified", ""),
        )
        .with(
            "design/REQ-003.md",
            item("REQ-003", "requirement", "obsolete", ""),
        )
        .with(
            "design/TEST-001.md",
            item(
                "TEST-001",
                "test",
                "passing",
                "[links]\nverifies = [\"REQ-002\"]\n",
            ),
        )
        .with(
            "design/MS-001.md",
            item("MS-001", "milestone", "active", "due = 2026-12-01\n"),
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
                "[links]\npart-of = [\"MS-001\"]\nmitigates = [\"RISK-001\"]\n",
            ),
        )
        .with("design/RISK-001.md", item("RISK-001", "risk", "open", ""))
        .with("design/RISK-002.md", item("RISK-002", "risk", "open", ""))
        .with("src/lib.rs", format!("// {}: REQ-002\n", "implements"));
    Project::load(&source).unwrap()
}

fn rows(blocks: &[Block]) -> Vec<String> {
    let table = blocks
        .iter()
        .find_map(|b| {
            if let Block::Table(t) = b {
                Some(t)
            } else {
                None
            }
        })
        .unwrap();
    table
        .rows
        .iter()
        .map(|r| r.iter().map(|c| plain(c)).collect::<Vec<_>>().join(" | "))
        .collect()
}

#[test]
fn traceability_marks_gaps_and_skips_closed_items() {
    let p = project();
    let blocks = report("traceability", &p, &[], &ReportOptions::default()).unwrap();
    assert_eq!(
        plain(match &blocks[0] {
            Block::Paragraph(i) => i,
            _ => panic!(),
        }),
        "2 requirements: implemented by 1 of 2, verified by 1 of 2."
    );
    assert_eq!(
        rows(&blocks),
        [
            "REQ-001 | REQ-001 title | approved | gap | gap",
            "REQ-002 | REQ-002 title | verified | src/lib.rs:1 | TEST-001"
        ]
    );
    let Block::Table(t) = &blocks[1] else {
        panic!()
    };
    assert_eq!(
        t.rows[0][3],
        vec![Inline::Status {
            text: "gap".into(),
            tone: Tone::Red
        }]
    );
    assert_eq!(
        t.rows[1][2],
        vec![Inline::Status {
            text: "verified".into(),
            tone: Tone::Green
        }]
    );
}

#[test]
fn roadmap_risks_and_findings() {
    let p = project();
    let roadmap = report("roadmap", &p, &[], &ReportOptions::default()).unwrap();
    assert_eq!(
        rows(&roadmap),
        [
            "MS-001 | MS-001 title | 2026-12-01 | active | 1 of 1 done | TASK-001",
            "MS-002 | MS-002 title | — | planned | — | "
        ]
    );
    let risks = report("risks", &p, &[], &ReportOptions::default()).unwrap();
    assert_eq!(
        rows(&risks),
        [
            "RISK-001 | RISK-001 title | open | TASK-001",
            "RISK-002 | RISK-002 title | open | unmitigated"
        ]
    );
    let none = report("findings", &p, &[], &ReportOptions::default()).unwrap();
    assert_eq!(
        none,
        vec![Block::Paragraph(vec![Inline::text("No findings.")])]
    );
    let no_baseline = report("changes", &p, &[], &ReportOptions::default()).unwrap();
    assert!(
        matches!(&no_baseline[0], Block::Paragraph(p) if crate::plain(p).contains("--since REV"))
    );
    assert!(report("nonsense", &p, &[], &ReportOptions::default()).is_none());
}

#[test]
fn template_problems_name_their_line() {
    let p = project();
    let text = "# A\n\n<!-- kiln:items type=\"requirement,epic\" status=\"!obsolete,gone\" root=\"REQ-009\" colour=\"x\" -->\n\n<!-- kiln:item REQ-001 -->\n<!-- kiln:item REQ-099 -->\n<!-- kiln:report changes -->\n<!-- kiln:report traceability type=\"test\" -->\n<!-- kiln:frobnicate -->\n\nA <!-- kiln:value colour --> and <!-- kiln:value name -->.\n";
    let problems: Vec<String> = template_problems(text, &p)
        .iter()
        .map(|q| format!("{}: {}", q.line, q.message))
        .collect();
    assert_eq!(
        problems,
        [
            "3: kiln:items has no attribute `colour` (type, status, tag, root, jira, style, split)",
            "3: `epic` isn't a type",
            "3: `gone` isn't a status of any type",
            "3: root `REQ-009` isn't an item",
            "6: kiln:item `REQ-099` isn't an item",
            "9: unknown directive kiln:frobnicate (items, item, report, summary, toc, value)",
            "11: kiln:value `colour` isn't name, commit, date or since",
        ]
    );
}
