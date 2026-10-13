//! The report builders (EXPORT.md, "Reports"): traceability, findings,
//! roadmap and risks, as the same document tree bodies render to. The
//! `changes` report needs `--since` and arrives in K6.
//!
//! Some reports name FORMAT.md's default types on purpose (`milestone`,
//! `risk`, `part-of`, `mitigates`): that is what they report on, and a
//! project without them gets an empty report that says so.

use kiln_index::{Project, Source};
use kiln_model::{Finding, Item};

use crate::doc::{Align, Block, Inline, Table, Target, Tone};

/// What a report directive or target may say.
#[derive(Debug, Clone, Default)]
pub struct ReportOptions {
    /// `traceability`: the type to trace (default `requirement`).
    pub type_name: Option<String>,
}

/// The lozenge colour for an item's status: initial grey, done green,
/// closed grey, others blue.
pub fn status_tone(project: &Project, item: &Item) -> Tone {
    match project.type_of(item) {
        Some(t) if t.is_done(&item.status) => Tone::Green,
        Some(t) if t.is_closed(&item.status) || t.initial == item.status => Tone::Grey,
        _ => Tone::Blue,
    }
}

/// The blocks of report `name`; `None` for a name this version doesn't
/// build.
pub fn report(
    name: &str,
    project: &Project,
    findings: &[Finding],
    options: &ReportOptions,
) -> Option<Vec<Block>> {
    match name {
        "traceability" => Some(traceability(project, options)),
        "findings" => Some(findings_report(findings)),
        "roadmap" => Some(roadmap(project)),
        "risks" => Some(risks(project)),
        _ => None,
    }
}

fn id_cell(item: &Item) -> Vec<Inline> {
    vec![Inline::Ref {
        id: item.id.clone(),
        text: Some(item.id.to_string()),
    }]
}

fn status_cell(project: &Project, item: &Item) -> Vec<Inline> {
    vec![Inline::Status {
        text: item.status.clone(),
        tone: status_tone(project, item),
    }]
}

fn source_cell(project: &Project, sources: &[Source]) -> Vec<Inline> {
    let mut out = Vec::new();
    for (i, source) in sources.iter().enumerate() {
        if i > 0 {
            out.push(Inline::text(", "));
        }
        out.push(source_inline(project, *source));
    }
    out
}

/// A link source: an item reference, or a link to a code location.
pub fn source_inline(project: &Project, source: Source) -> Inline {
    match source {
        Source::Item(i) => {
            let item = &project.items[i];
            Inline::Ref {
                id: item.id.clone(),
                text: Some(item.id.to_string()),
            }
        }
        Source::Code(i) => {
            let code = &project.code_refs[i];
            Inline::Link {
                target: Target::Repo {
                    path: code.path.clone(),
                    line: Some(code.target.span.line),
                    fragment: None,
                },
                content: vec![Inline::text(code.location())],
            }
        }
    }
}

fn table(header: &[&str], rows: Vec<Vec<Vec<Inline>>>) -> Block {
    Block::Table(Table {
        align: vec![Align::None; header.len()],
        header: header.iter().map(|h| vec![Inline::text(*h)]).collect(),
        rows,
    })
}

fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

fn traceability(project: &Project, options: &ReportOptions) -> Vec<Block> {
    let config = &project.config;
    let type_name = options
        .type_name
        .clone()
        .unwrap_or_else(|| "requirement".to_owned());
    let Some(ty) = config.type_named(&type_name) else {
        return vec![Block::Paragraph(vec![Inline::text(format!(
            "There is no type `{type_name}` to trace."
        ))])];
    };
    let roles: Vec<&kiln_model::LinkRole> = config
        .coverage_roles
        .iter()
        .filter_map(|r| config.role(r))
        .filter(|r| r.allows_to(&ty.name))
        .collect();
    let items: Vec<&Item> = project
        .items
        .iter()
        .filter(|i| i.type_name == ty.name && !ty.is_closed(&i.status))
        .collect();
    if items.is_empty() {
        return vec![Block::Paragraph(vec![Inline::text(format!(
            "No open {} to trace.",
            ty.plural.to_lowercase()
        ))])];
    }
    let mut header: Vec<String> = vec![ty.title.clone(), "Title".into(), "Status".into()];
    header.extend(roles.iter().map(|r| capitalised(&r.inverse)));
    let rows = items
        .iter()
        .map(|item| {
            let mut row = vec![
                id_cell(item),
                vec![Inline::text(item.title.clone())],
                status_cell(project, item),
            ];
            for role in &roles {
                let sources: Vec<Source> = project
                    .inbound(&item.id)
                    .iter()
                    .filter(|l| l.role == role.name && project.source_is_open(l.source))
                    .map(|l| l.source)
                    .collect();
                row.push(if !sources.is_empty() {
                    source_cell(project, &sources)
                } else if project.covered(&item.id, &role.name) {
                    vec![Inline::text("through its children")]
                } else {
                    vec![Inline::Status {
                        text: "gap".into(),
                        tone: Tone::Red,
                    }]
                });
            }
            row
        })
        .collect();
    let header: Vec<&str> = header.iter().map(String::as_str).collect();
    let covered: Vec<String> = roles
        .iter()
        .map(|r| {
            let n = items
                .iter()
                .filter(|i| project.covered(&i.id, &r.name))
                .count();
            format!("{} {n} of {}", r.inverse, items.len())
        })
        .collect();
    vec![
        Block::Paragraph(vec![Inline::text(format!(
            "{} {}: {}.",
            items.len(),
            ty.plural.to_lowercase(),
            covered.join(", ")
        ))]),
        table(&header, rows),
    ]
}

fn findings_report(findings: &[Finding]) -> Vec<Block> {
    if findings.is_empty() {
        return vec![Block::Paragraph(vec![Inline::text("No findings.")])];
    }
    let mut sorted: Vec<&Finding> = findings.iter().collect();
    sorted.sort_by(|a, b| {
        (a.rule, &a.path, a.line, a.column, &a.message)
            .cmp(&(b.rule, &b.path, b.line, b.column, &b.message))
    });
    let rows = sorted
        .iter()
        .map(|f| {
            vec![
                vec![Inline::text(f.rule.code())],
                vec![Inline::Status {
                    text: f.severity.as_str().into(),
                    tone: if f.severity == kiln_model::Severity::Error {
                        Tone::Red
                    } else {
                        Tone::Yellow
                    },
                }],
                vec![Inline::Link {
                    target: Target::Repo {
                        path: f.path.clone(),
                        line: Some(f.line),
                        fragment: None,
                    },
                    content: vec![Inline::text(format!("{}:{}", f.path, f.line))],
                }],
                vec![Inline::text(f.message.clone())],
            ]
        })
        .collect();
    vec![table(&["Rule", "Severity", "Where", "Finding"], rows)]
}

fn roadmap(project: &Project) -> Vec<Block> {
    let mut milestones: Vec<&Item> = project
        .items
        .iter()
        .filter(|i| i.type_name == "milestone")
        .collect();
    if milestones.is_empty() {
        return vec![Block::Paragraph(vec![Inline::text("No milestones.")])];
    }
    milestones
        .sort_by(|a, b| (a.due.is_none(), a.due, &a.id).cmp(&(b.due.is_none(), b.due, &b.id)));
    let rows = milestones
        .iter()
        .map(|m| {
            let includes: Vec<Source> = project
                .inbound(&m.id)
                .iter()
                .filter(|l| l.role == "part-of")
                .map(|l| l.source)
                .collect();
            let progress = match project.progress(&m.id) {
                Some(p) => format!("{} of {} done", p.done, p.total),
                None => "—".to_owned(),
            };
            vec![
                id_cell(m),
                vec![Inline::text(m.title.clone())],
                vec![Inline::text(
                    m.due.map(|d| d.to_string()).unwrap_or_else(|| "—".into()),
                )],
                status_cell(project, m),
                vec![Inline::text(progress)],
                source_cell(project, &includes),
            ]
        })
        .collect();
    vec![table(
        &[
            "Milestone",
            "Title",
            "Due",
            "Status",
            "Progress",
            "Includes",
        ],
        rows,
    )]
}

fn risks(project: &Project) -> Vec<Block> {
    let risks: Vec<&Item> = project
        .items
        .iter()
        .filter(|i| i.type_name == "risk" && !project.is_closed(i))
        .collect();
    if risks.is_empty() {
        return vec![Block::Paragraph(vec![Inline::text("No open risks.")])];
    }
    let rows = risks
        .iter()
        .map(|r| {
            let by: Vec<Source> = project
                .inbound(&r.id)
                .iter()
                .filter(|l| l.role == "mitigates" && project.source_is_open(l.source))
                .map(|l| l.source)
                .collect();
            let by = if by.is_empty() {
                vec![Inline::Status {
                    text: "unmitigated".into(),
                    tone: Tone::Red,
                }]
            } else {
                source_cell(project, &by)
            };
            vec![
                id_cell(r),
                vec![Inline::text(r.title.clone())],
                status_cell(project, r),
                by,
            ]
        })
        .collect();
    vec![table(&["Risk", "Title", "Status", "Mitigated by"], rows)]
}
