//! The report builders (EXPORT.md, "Reports"): traceability, the
//! requirement levels, findings, roadmap, risks and (with `kiln export
//! --since`) changes, as the same document tree bodies render to.
//!
//! Some reports name FORMAT.md's default types on purpose (`milestone`,
//! `risk`, `part-of`, `mitigates`): that is what they report on, and a
//! project without them gets an empty report that says so.

use kiln_index::{ChangeKind, ItemChange, Project, Source};
use kiln_model::{Finding, Item};

use crate::doc::{Align, Block, Inline, Table, Target, Tone};

/// What a report directive or target may say, and the baseline when the
/// export has one.
#[derive(Debug, Clone, Default)]
pub struct ReportOptions<'a> {
    /// `traceability`: the type to trace (default: the highest
    /// requirement level, `requirement` without `[requirements]`).
    pub type_name: Option<String>,
    /// `--since`: what the `changes` report lists.
    pub since: Option<&'a Since>,
}

/// The baseline of `kiln export --since REV`: the revision as given, its
/// commit, and every item change from it to what is exported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Since {
    pub label: String,
    pub commit: String,
    pub changes: Vec<ItemChange>,
}

impl Since {
    /// `v1.0 (1a2b3c4)`.
    pub fn describe(&self) -> String {
        let short = &self.commit[..self.commit.len().min(7)];
        if self.label.starts_with(short) {
            short.to_owned()
        } else {
            format!("{} ({short})", self.label)
        }
    }

    /// Whether `id` was added or changed since the baseline.
    pub fn kind_of(&self, id: &kiln_model::ItemId) -> Option<ChangeKind> {
        self.changes.iter().find(|c| &c.id == id).map(|c| c.kind)
    }
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
        "levels" => Some(levels(project)),
        "findings" => Some(findings_report(findings)),
        "roadmap" => Some(roadmap(project)),
        "risks" => Some(risks(project)),
        "changes" => Some(changes(options.since)),
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
        .or_else(|| config.levels().first().map(|t| (*t).to_owned()))
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

/// The requirement levels as a matrix: a row per chain from the top level
/// down, a column per level, then what covers the row's last
/// requirement. A requirement spanning several rows is shown on the
/// first. Derived requirements follow, then those tracing to nothing.
// implements: REQ-018
fn levels(project: &Project) -> Vec<Block> {
    let config = &project.config;
    let Some(trace_role) = config.trace() else {
        return vec![Block::Paragraph(vec![Inline::text(
            "This project has one requirement level; [requirements] levels lists more.",
        )])];
    };
    let trace = project.level_trace();
    let lowest = trace.levels.len() - 1;
    let roles: Vec<&kiln_model::LinkRole> = config
        .coverage_roles
        .iter()
        .filter(|r| **r != trace_role.name)
        .filter_map(|r| config.role(r))
        .filter(|r| trace.levels.iter().any(|l| r.allows_to(l)))
        .collect();
    let mut header: Vec<String> = trace
        .levels
        .iter()
        .map(|l| {
            config
                .type_named(l)
                .map_or(l.to_string(), |t| t.title.clone())
        })
        .collect();
    header.extend(roles.iter().map(|r| capitalised(&r.inverse)));
    let marker = |text: &str, tone: Tone| {
        vec![Inline::Status {
            text: text.into(),
            tone,
        }]
    };

    let mut rows = Vec::new();
    let mut chains = 0;
    let mut gaps = 0;
    let sections = [
        (&trace.roots, None),
        (&trace.derived, Some(("derived", Tone::Grey))),
        (&trace.untraced, Some(("gap", Tone::Red))),
    ];
    for (nodes, above) in sections {
        for node in nodes.iter() {
            let mut previous: Vec<Option<&Item>> = vec![None; trace.levels.len()];
            for path in node.paths() {
                let mut row = vec![Vec::new(); header.len()];
                let first = config.level_of(&path[0].type_name).unwrap_or(0);
                if let Some((text, tone)) = above {
                    for cell in row.iter_mut().take(first) {
                        *cell = marker(text, tone);
                    }
                    if tone == Tone::Red {
                        gaps += 1;
                    }
                }
                let mut shown: Vec<Option<&Item>> = vec![None; trace.levels.len()];
                for item in &path {
                    if let Some(level) = config.level_of(&item.type_name) {
                        shown[level] = Some(item);
                    }
                }
                // Blank only a prefix of the previous row: a requirement
                // under two parents is shown under each.
                let same = |a: Option<&Item>, b: Option<&Item>| match (a, b) {
                    (Some(a), Some(b)) => a.id == b.id,
                    (None, None) => true,
                    _ => false,
                };
                let mut repeating = true;
                for (level, item) in shown.iter().enumerate() {
                    repeating = repeating && same(*item, previous[level]);
                    if let (Some(item), false) = (item, repeating) {
                        row[level] = id_cell(item);
                    }
                }
                let leaf = path[path.len() - 1];
                let level = config.level_of(&leaf.type_name).unwrap_or(0);
                if level < lowest && !project.covered(&leaf.id, &trace_role.name) {
                    row[level + 1] = marker("gap", Tone::Red);
                    gaps += 1;
                }
                for (n, role) in roles.iter().enumerate() {
                    let cell = &mut row[trace.levels.len() + n];
                    if !role.allows_to(&leaf.type_name) {
                        *cell = vec![Inline::text("—")];
                        continue;
                    }
                    let sources: Vec<Source> = project
                        .inbound(&leaf.id)
                        .iter()
                        .filter(|l| l.role == role.name && project.source_is_open(l.source))
                        .map(|l| l.source)
                        .collect();
                    *cell = if !sources.is_empty() {
                        source_cell(project, &sources)
                    } else if project.covered(&leaf.id, &role.name) {
                        vec![Inline::text("through its children")]
                    } else {
                        gaps += 1;
                        marker("gap", Tone::Red)
                    };
                }
                previous = shown;
                chains += usize::from(above.is_none());
                rows.push(row);
            }
        }
    }
    if rows.is_empty() {
        return vec![Block::Paragraph(vec![Inline::text(
            "No open requirements to trace.",
        )])];
    }
    let header: Vec<&str> = header.iter().map(String::as_str).collect();
    vec![
        Block::Paragraph(vec![Inline::text(format!(
            "{chains} chains from {} {}; {} derived, {} not traced to the level above; {gaps} gaps.",
            trace.roots.len(),
            config
                .type_named(trace.levels[0])
                .map_or(trace.levels[0].to_string(), |t| t.plural.to_lowercase()),
            trace.derived.len(),
            trace.untraced.len(),
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

/// Items added, changed and removed since the baseline, one row each; a
/// paragraph saying how to get them when there is no baseline.
fn changes(since: Option<&Since>) -> Vec<Block> {
    let Some(since) = since else {
        return vec![Block::Paragraph(vec![
            Inline::text("No baseline: export with "),
            Inline::Code("kiln export --since REV".into()),
            Inline::text(" to list what changed."),
        ])];
    };
    let count = |k| since.changes.iter().filter(|c| c.kind == k).count();
    let intro = Block::Paragraph(vec![Inline::text(format!(
        "Since {}: {} added, {} changed, {} removed.",
        since.describe(),
        count(ChangeKind::Added),
        count(ChangeKind::Changed),
        count(ChangeKind::Removed)
    ))]);
    if since.changes.is_empty() {
        return vec![intro];
    }
    let rows = since
        .changes
        .iter()
        .map(|c| {
            let item = match c.kind {
                // Gone from the document: nothing to link to.
                ChangeKind::Removed => vec![Inline::text(format!("{} {}", c.id, c.title))],
                _ => vec![
                    Inline::Ref {
                        id: c.id.clone(),
                        text: Some(c.id.to_string()),
                    },
                    Inline::text(format!(" {}", c.title)),
                ],
            };
            let (text, tone) = match c.kind {
                ChangeKind::Added => ("New", Tone::Green),
                ChangeKind::Changed => ("Changed", Tone::Yellow),
                ChangeKind::Removed => ("Removed", Tone::Grey),
            };
            vec![
                item,
                vec![Inline::Status {
                    text: text.into(),
                    tone,
                }],
                vec![Inline::text(c.summary())],
            ]
        })
        .collect();
    vec![
        intro,
        Block::Table(Table {
            align: vec![Align::None, Align::None, Align::None],
            header: vec![
                vec![Inline::text("Item")],
                vec![Inline::text("Change")],
                vec![Inline::text("What")],
            ],
            rows,
        }),
    ]
}
