//! The pages as rows (GUI.md, "Pages"): pure functions of the project,
//! what the window has gathered about it (the baseline's changes, the
//! findings, the Jira mirror, the exports' plans, the comments) and the
//! page's options, so each is tested as text without a window.

use std::collections::{HashMap, HashSet};

use kiln_index::{ChangeKind, ItemChange, Project, Source};
use kiln_model::{Finding, Item, ItemId, Rule, Severity};

use crate::host::{CommentGroup, ExportReport};
use crate::screen::{pill, span, Line, Role};

/// The pages, in `Tab` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    Tree,
    Trace,
    Impact,
    Board,
    Roadmap,
    Decisions,
    Changes,
    Item,
    Check,
    Exports,
    Comments,
}

impl Page {
    pub const ALL: [Page; 11] = [
        Page::Tree,
        Page::Trace,
        Page::Impact,
        Page::Board,
        Page::Roadmap,
        Page::Decisions,
        Page::Changes,
        Page::Item,
        Page::Check,
        Page::Exports,
        Page::Comments,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Page::Tree => "Tree",
            Page::Trace => "Trace",
            Page::Impact => "Impact",
            Page::Board => "Board",
            Page::Roadmap => "Roadmap",
            Page::Decisions => "Decisions",
            Page::Changes => "Changes",
            Page::Item => "Item",
            Page::Check => "Check",
            Page::Exports => "Exports",
            Page::Comments => "Comments",
        }
    }

    pub fn index(self) -> usize {
        Page::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }
}

/// Where `Enter` goes: a file relative to the project, at a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Open {
    pub path: String,
    pub line: u32,
}

/// One row of a page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row {
    pub line: Line,
    /// A heading groups the rows under it; the cursor skips it.
    pub heading: bool,
    /// Nesting, for keeping a match's ancestors when filtering.
    pub depth: u16,
    /// The item the row is about, for `i` and for keeping the cursor
    /// across reloads.
    pub item: Option<ItemId>,
    pub open: Option<Open>,
    /// This copy's file, when the item's ID is in several (Check, for
    /// renumbering the right one).
    pub file: Option<String>,
    /// The export target the row is about (Exports).
    pub target: Option<String>,
    /// What `Enter` opens in the browser (Comments).
    pub url: Option<String>,
}

/// Something read on a thread: not asked for yet, on its way, or here.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Remote<T> {
    #[default]
    NotAsked,
    Loading,
    Ready(T),
    Failed(String),
}

/// What the pages show besides the project and their options.
#[derive(Clone, Copy, Default)]
pub struct Data<'a> {
    pub changes: Option<&'a Changes>,
    pub jira: Option<&'a kiln_jira::Mirror>,
    /// `kiln check`'s findings, sorted.
    pub findings: &'a [Finding],
    /// A newer `kiln check` is running.
    pub checking: bool,
    pub exports: Option<&'a Remote<Vec<ExportReport>>>,
    pub comments: Option<&'a Remote<Vec<CommentGroup>>>,
}

/// What a page shows besides the project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// Closed items too (Tree, Board).
    pub closed: bool,
    /// Trace: only items with a gap.
    pub gaps_only: bool,
    /// Trace: the traced type; the first traceable one when unset.
    pub trace_type: Option<String>,
    /// Impact: the item picked with `i`.
    pub impact: Option<ItemId>,
    /// Item: the item picked with `v`.
    pub item: Option<ItemId>,
}

/// The baseline for the Changes page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changes {
    /// `v1.0 (1a2b3c4)`.
    pub label: String,
    pub changes: Vec<ItemChange>,
}

fn heading(text: String) -> Row {
    Row {
        line: vec![span(text, Role::Heading)],
        heading: true,
        ..Row::default()
    }
}

fn note(text: &str) -> Row {
    Row {
        line: vec![span(text, Role::Muted)],
        ..Row::default()
    }
}

/// A status's colour, from what its type says it means, so it reads the
/// same in every project: done is green, `blocked` an error, closed
/// muted, the initial one (a draft, a to-do) the headings' colour, and
/// anything between (approved, doing, active) the theme's purple.
fn status_role(p: &Project, item: &Item) -> Role {
    let status = &item.status;
    match p.type_of(item) {
        Some(t) if t.is_done(status) => Role::Done,
        Some(_) if status == "blocked" => Role::Error,
        Some(t) if t.is_closed(status) => Role::Muted,
        Some(t) if t.initial == *status => Role::Heading,
        Some(_) => Role::Active,
        None => Role::Normal,
    }
}

/// `REQ-014  Title  approved`, indented by `depth`.
fn item_row(p: &Project, item: &Item, depth: u16, extra: Line) -> Row {
    let mut line = vec![
        span("  ".repeat(depth as usize), Role::Normal),
        span(format!("{:<9}", item.id.to_string()), Role::Accent),
        span(format!(" {}", item.title), Role::Normal),
        span(" ", Role::Normal),
        pill(item.status.clone(), status_role(p, item)),
    ];
    line.extend(extra);
    Row {
        line,
        heading: false,
        depth: depth + 1,
        item: Some(item.id.clone()),
        open: Some(Open {
            path: item.path.clone(),
            line: item.title_span.line.max(1),
        }),
        ..Row::default()
    }
}

/// The rows of `page`.
pub fn rows(page: Page, p: &Project, data: &Data, o: &Options) -> Vec<Row> {
    let mut rows = page_rows(page, p, data, o);
    if let Some(mirror) = data.jira {
        with_tickets(&mut rows, p, mirror);
    }
    rows
}

/// Each item row gains its tickets' status from the Jira mirror: green
/// when resolved, muted otherwise.
fn with_tickets(rows: &mut [Row], p: &Project, mirror: &kiln_jira::Mirror) {
    let items: HashMap<&ItemId, &Item> = p.items.iter().map(|i| (&i.id, i)).collect();
    for row in rows.iter_mut() {
        let Some(item) = row.item.as_ref().and_then(|id| items.get(id)) else {
            continue;
        };
        for key in &item.jira {
            match mirror.ticket(key) {
                Some(t) => row.line.push(span(
                    format!("  {key} {}", t.line()),
                    if t.resolved { Role::Good } else { Role::Muted },
                )),
                None => row.line.push(span(format!("  {key} ?"), Role::Muted)),
            }
        }
    }
}

fn page_rows(page: Page, p: &Project, data: &Data, o: &Options) -> Vec<Row> {
    match page {
        Page::Tree => tree(p, o),
        Page::Trace => trace(p, o),
        Page::Impact => impact(p, o),
        Page::Board => board(p, o),
        Page::Roadmap => roadmap(p),
        Page::Decisions => decisions(p),
        Page::Changes => changes_page(p, data.changes),
        Page::Item => item_page(p, o, data.jira),
        Page::Check => check(p, data.findings, data.checking),
        Page::Exports => exports(data.exports),
        Page::Comments => comments(p, data.comments),
    }
}

fn first_items(p: &Project) -> Vec<&Item> {
    let mut seen = HashSet::new();
    p.items.iter().filter(|i| seen.insert(&i.id)).collect()
}

fn tree(p: &Project, o: &Options) -> Vec<Row> {
    let items = first_items(p);
    let has_parent = |i: &Item| {
        i.parent
            .as_ref()
            .and_then(|r| r.id.as_ref().ok())
            .is_some_and(|parent| p.item(parent).is_some() && *parent != i.id)
    };
    let mut out = Vec::new();
    let mut placed = HashSet::new();
    for ty in &p.config.types {
        let roots: Vec<&Item> = items
            .iter()
            .copied()
            .filter(|i| i.type_name == ty.name && !has_parent(i))
            .collect();
        if roots.is_empty() {
            continue;
        }
        let at = out.len();
        for root in roots {
            walk(p, root, 0, o, &mut placed, &mut out);
        }
        if out.len() > at {
            out.insert(at, heading(ty.plural.clone()));
        }
    }
    // Items only reachable through a parent cycle.
    let rest: Vec<&Item> = items
        .into_iter()
        .filter(|i| !placed.contains(&i.id))
        .collect();
    if !rest.is_empty() {
        out.push(heading("In a parent cycle".into()));
        for i in rest {
            if o.closed || !p.is_closed(i) {
                out.push(item_row(p, i, 0, Vec::new()));
            }
        }
    }
    out
}

fn walk<'p>(
    p: &'p Project,
    item: &'p Item,
    depth: u16,
    o: &Options,
    placed: &mut HashSet<ItemId>,
    out: &mut Vec<Row>,
) {
    if !placed.insert(item.id.clone()) {
        return;
    }
    let shown = o.closed || !p.is_closed(item);
    if shown {
        out.push(item_row(p, item, depth, Vec::new()));
    }
    let next = if shown { depth + 1 } else { depth };
    for child in p.children(&item.id) {
        walk(p, child, next, o, placed, out);
    }
}

/// The types some coverage role reaches, in `kiln.toml` order.
pub fn traceable(p: &Project) -> Vec<String> {
    p.config
        .types
        .iter()
        .filter(|t| {
            p.config
                .coverage_roles
                .iter()
                .filter_map(|r| p.config.role(r))
                .any(|r| r.allows_to(&t.name))
        })
        .map(|t| t.name.clone())
        .collect()
}

fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let mut t: String = s.chars().take(max - 1).collect();
        t.push('…');
        t
    }
}

fn trace(p: &Project, o: &Options) -> Vec<Row> {
    let types = traceable(p);
    let Some(ty) = o
        .trace_type
        .clone()
        .filter(|t| types.contains(t))
        .or_else(|| types.first().cloned())
    else {
        return vec![note(
            "No type is reached by a coverage role ([coverage] roles in kiln.toml).",
        )];
    };
    let roles: Vec<String> = p
        .config
        .coverage_roles
        .iter()
        .filter(|r| p.config.role(r).is_some_and(|role| role.allows_to(&ty)))
        .cloned()
        .collect();
    const CELL: usize = 34;
    let mut body = Vec::new();
    for item in first_items(p)
        .into_iter()
        .filter(|i| i.type_name == ty && !p.is_closed(i))
    {
        let mut cells: Vec<(String, Role)> = Vec::new();
        let mut gap = false;
        for role in &roles {
            let sources: Vec<String> = p
                .inbound(&item.id)
                .iter()
                .filter(|l| &l.role == role && p.source_is_open(l.source))
                .map(|l| p.describe(l.source))
                .collect();
            if !sources.is_empty() {
                cells.push((cut(&sources.join(", "), CELL), Role::Good));
            } else if p.covered(&item.id, role) {
                cells.push(("through its children".into(), Role::Good));
            } else {
                cells.push(("gap".into(), Role::Error));
                gap = true;
            }
        }
        if o.gaps_only && !gap {
            continue;
        }
        let mut extra: Line = Vec::new();
        for (text, role) in cells {
            extra.push(span(format!("  {text:<CELL$}"), role));
        }
        let mut row = item_row(p, item, 0, Vec::new());
        // The title column is cut so the coverage columns line up.
        row.line[2] = span(format!(" {:<40}", cut(&item.title, 40)), Role::Normal);
        row.line.truncate(3);
        row.line.extend(extra);
        body.push(row);
    }
    // As wide as the ID and title columns of the rows below.
    let mut head = format!("{:<50}", capital(&ty));
    for role in &roles {
        head.push_str(&format!("  {role:<CELL$}"));
    }
    let mut out = vec![heading(head.trim_end().to_owned())];
    if body.is_empty() {
        out.push(note(if o.gaps_only {
            "No gaps."
        } else {
            "Nothing to trace."
        }));
    }
    out.extend(body);
    out
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

fn impact(p: &Project, o: &Options) -> Vec<Row> {
    let Some(id) = &o.impact else {
        return vec![note("Put the cursor on an item on any page and press i.")];
    };
    let Some(target) = p.item(id) else {
        return vec![note(&format!("{id} isn't in the project any more."))];
    };
    let impact = p.impact(id);
    let mut out = vec![heading(format!(
        "What depends on {} {} ({})",
        target.id,
        target.title,
        impact.len()
    ))];
    if impact.is_empty() {
        out.push(note("Nothing reaches it."));
    }
    for i in impact {
        let path: Vec<String> = i
            .path
            .iter()
            .map(|s| {
                let rel = if s.relation == "parent" {
                    "child of"
                } else {
                    &s.relation
                };
                format!("{rel} {}", s.to)
            })
            .collect();
        let how = span(format!("  ← {}", path.join(", ")), Role::Muted);
        match i.source {
            Source::Item(n) => out.push(item_row(p, &p.items[n], 0, vec![how])),
            Source::Code(n) => {
                let code = &p.code_refs[n];
                out.push(Row {
                    line: vec![span(code.location(), Role::Accent), how],
                    heading: false,
                    depth: 1,
                    item: None,
                    open: Some(Open {
                        path: code.path.clone(),
                        line: code.target.span.line,
                    }),
                    ..Row::default()
                });
            }
        }
    }
    out
}

fn board(p: &Project, o: &Options) -> Vec<Row> {
    let types: Vec<&kiln_model::TypeDef> = match p.config.type_named("task") {
        Some(t) => vec![t],
        None => p
            .config
            .types
            .iter()
            .filter(|t| !t.done.is_empty() && t.name != "milestone")
            .collect(),
    };
    if types.is_empty() {
        return vec![note(
            "No work items: no `task` type, and no type with `done` statuses.",
        )];
    }
    let mut out = Vec::new();
    let items = first_items(p);
    for ty in types {
        for status in &ty.statuses {
            if ty.is_closed(status) && !o.closed {
                continue;
            }
            let here: Vec<&&Item> = items
                .iter()
                .filter(|i| i.type_name == ty.name && &i.status == status)
                .collect();
            let name = if p.config.types.len() > 1 && p.config.type_named("task").is_none() {
                format!("{} · {status} ({})", ty.plural, here.len())
            } else {
                format!("{status} ({})", here.len())
            };
            out.push(heading(name));
            for i in here {
                let mut extra: Line = i
                    .parent
                    .as_ref()
                    .map(|r| span(format!("  under {}", r.raw), Role::Muted))
                    .into_iter()
                    .collect();
                extra.extend(plan_badge(p, i));
                out.push(item_row(p, i, 0, extra));
            }
        }
    }
    out
}

/// Where open work stands in the plan, as `kiln next` and `kiln status`
/// say it (ADR-018): `ready`, or what it waits on; and who has claimed it.
/// Nothing for other items, done or closed work, or a `blocked` status,
/// which the row's own pill already says.
// implements: REQ-022
fn plan_badge(p: &Project, item: &Item) -> Line {
    let mut out = Vec::new();
    if !p.is_work(item) || !p.is_open(item) {
        return out;
    }
    let waits: Vec<String> = p
        .open_dependencies(item)
        .iter()
        .map(|d| d.id.to_string())
        .collect();
    if !waits.is_empty() {
        out.push(span(format!("  waits on {}", waits.join(", ")), Role::Warn));
    } else if p.is_ready(item) {
        out.push(span("  ready".to_owned(), Role::Good));
    }
    if let Some(owner) = &item.owner {
        out.push(span(format!("  · {owner}"), Role::Muted));
    }
    out
}

fn bar(done: usize, total: usize) -> String {
    let full = (done * 10 + total / 2)
        .checked_div(total)
        .unwrap_or(0)
        .min(10);
    format!("{}{}", "█".repeat(full), "░".repeat(10 - full))
}

fn roadmap(p: &Project) -> Vec<Row> {
    let Some(ty) = p.config.type_named("milestone") else {
        return vec![note("No `milestone` type in kiln.toml.")];
    };
    let mut milestones: Vec<&Item> = first_items(p)
        .into_iter()
        .filter(|i| i.type_name == ty.name)
        .collect();
    milestones
        .sort_by(|a, b| (a.due.is_none(), a.due, &a.id).cmp(&(b.due.is_none(), b.due, &b.id)));
    if milestones.is_empty() {
        return vec![note("No milestones yet.")];
    }
    let mut out = vec![heading("Milestones".into())];
    for m in milestones {
        let due = m
            .due
            .map(|d| format!("  due {d}"))
            .unwrap_or_else(|| "  undated".into());
        let progress = match p.progress(&m.id) {
            Some(pr) => format!("  {}/{} {}", pr.done, pr.total, bar(pr.done, pr.total)),
            None => String::new(),
        };
        out.push(item_row(
            p,
            m,
            0,
            vec![span(due, Role::Muted), span(progress, Role::Good)],
        ));
        let mut parts: Vec<&Item> = p
            .inbound(&m.id)
            .iter()
            .filter(|l| l.role == "part-of")
            .filter_map(|l| match l.source {
                Source::Item(n) => Some(&p.items[n]),
                Source::Code(_) => None,
            })
            .collect();
        parts.sort_by(|a, b| a.id.cmp(&b.id));
        parts.dedup_by(|a, b| a.id == b.id);
        for part in parts {
            out.push(item_row(p, part, 1, plan_badge(p, part)));
        }
    }
    out
}

fn decisions(p: &Project) -> Vec<Row> {
    let Some(ty) = p.config.type_named("decision") else {
        return vec![note("No `decision` type in kiln.toml.")];
    };
    let mut items: Vec<&Item> = first_items(p)
        .into_iter()
        .filter(|i| i.type_name == ty.name)
        .collect();
    items.sort_by(|a, b| a.id.cmp(&b.id));
    if items.is_empty() {
        return vec![note("No decisions yet.")];
    }
    let mut out = vec![heading("Decision log".into())];
    for d in items {
        let mut extra: Line = Vec::new();
        let supersedes: Vec<String> = d
            .links
            .iter()
            .filter(|l| l.role == "supersedes")
            .map(|l| l.target.raw.clone())
            .collect();
        if !supersedes.is_empty() {
            extra.push(span(
                format!("  supersedes {}", supersedes.join(", ")),
                Role::Muted,
            ));
        }
        let by: Vec<String> = p
            .inbound(&d.id)
            .iter()
            .filter(|l| l.role == "supersedes")
            .map(|l| p.describe(l.source))
            .collect();
        if !by.is_empty() {
            extra.push(span(
                format!("  superseded by {}", by.join(", ")),
                Role::Warn,
            ));
        }
        out.push(item_row(p, d, 0, extra));
    }
    out
}

fn changes_page(p: &Project, changes: Option<&Changes>) -> Vec<Row> {
    let Some(c) = changes else {
        return vec![note(
            "No baseline: press b to pick one, start with `kiln gui --since REV`, or tag a release.",
        )];
    };
    let count = |k| c.changes.iter().filter(|x| x.kind == k).count();
    let mut out = vec![heading(format!(
        "Since {}: {} added, {} changed, {} removed",
        c.label,
        count(ChangeKind::Added),
        count(ChangeKind::Changed),
        count(ChangeKind::Removed)
    ))];
    for (kind, name, role) in [
        (ChangeKind::Added, "Added", Role::Good),
        (ChangeKind::Changed, "Changed", Role::Warn),
        (ChangeKind::Removed, "Removed", Role::Muted),
    ] {
        let these: Vec<&ItemChange> = c.changes.iter().filter(|x| x.kind == kind).collect();
        if these.is_empty() {
            continue;
        }
        out.push(heading(format!("{name} ({})", these.len())));
        for x in these {
            let what = match kind {
                ChangeKind::Changed => format!("  {}", x.summary()),
                _ => format!("  ({})", x.type_name),
            };
            let open = (kind != ChangeKind::Removed)
                .then(|| p.item(&x.id))
                .flatten()
                .map(|i| Open {
                    path: i.path.clone(),
                    line: i.title_span.line.max(1),
                });
            out.push(Row {
                line: vec![
                    span(format!("{:<9}", x.id.to_string()), role),
                    span(format!(" {}", x.title), Role::Normal),
                    span(what, Role::Muted),
                ],
                heading: false,
                depth: 1,
                item: (kind != ChangeKind::Removed).then(|| x.id.clone()),
                open,
                ..Row::default()
            });
        }
    }
    out
}

/// Where a value `kiln show` prints leads: an item (`REQ-001 Title`) or
/// a code location (`src/lib.rs:12`).
fn resolve(p: &Project, value: &str) -> (Option<ItemId>, Option<Open>) {
    let first = value.split_whitespace().next().unwrap_or_default();
    if let Some(item) = first.parse::<ItemId>().ok().and_then(|id| p.item(&id)) {
        return (
            Some(item.id.clone()),
            Some(Open {
                path: item.path.clone(),
                line: item.title_span.line.max(1),
            }),
        );
    }
    if let Some((path, line)) = first.trim_end_matches(',').rsplit_once(':') {
        if let Ok(line) = line.parse::<u32>() {
            return (
                None,
                Some(Open {
                    path: path.to_owned(),
                    line,
                }),
            );
        }
    }
    (None, None)
}

/// `kiln show` as rows: the item, then one row per related item or
/// location under its label, each one a row the cursor can open or view.
fn item_page(p: &Project, o: &Options, jira: Option<&kiln_jira::Mirror>) -> Vec<Row> {
    let Some(id) = &o.item else {
        return vec![note("Put the cursor on an item on any page and press v.")];
    };
    let Some(item) = p.item(id) else {
        return vec![note(&format!("{id} isn't in the project any more."))];
    };
    let mut out = vec![heading(format!("{} {}", item.id, item.title))];
    out.push(item_row(
        p,
        item,
        0,
        vec![span(
            format!("  {} · {}:{}", item.type_name, item.path, item.id_span.line),
            Role::Muted,
        )],
    ));
    let facts = p.facts(item);
    let width = facts
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0)
        .max(8);
    for (key, values) in facts {
        for (n, value) in values.iter().enumerate() {
            let label = if n == 0 { key.as_str() } else { "" };
            let (target, open) = if key == "jira" {
                (None, None)
            } else {
                resolve(p, value)
            };
            let role = if target.is_some() || open.is_some() {
                Role::Accent
            } else {
                Role::Normal
            };
            let mut line = vec![
                span(format!("  {label:width$}  "), Role::Muted),
                span(value.clone(), role),
            ];
            if key == "jira" {
                if let Some(t) = jira.and_then(|m| m.ticket(value)) {
                    line.push(span(
                        format!("  {}", t.line()),
                        if t.resolved { Role::Good } else { Role::Muted },
                    ));
                }
            }
            out.push(Row {
                line,
                depth: 1,
                item: target,
                open,
                ..Row::default()
            });
        }
    }
    out
}

/// `kiln check`, grouped by file.
fn check(p: &Project, findings: &[Finding], checking: bool) -> Vec<Row> {
    let shown: Vec<&Finding> = findings
        .iter()
        .filter(|f| f.severity != Severity::Off)
        .collect();
    let errors = shown
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warnings = shown.len() - errors;
    let mut out = vec![heading(format!(
        "kiln check: {errors} error{}, {warnings} warning{}{}",
        if errors == 1 { "" } else { "s" },
        if warnings == 1 { "" } else { "s" },
        if checking { " (checking again…)" } else { "" }
    ))];
    if shown.is_empty() {
        out.push(note(if checking {
            "Running kiln check…"
        } else {
            "No findings."
        }));
        return out;
    }
    let mut file: Option<&str> = None;
    for f in shown {
        if file != Some(f.path.as_str()) {
            file = Some(&f.path);
            out.push(heading(f.path.clone()));
        }
        let item = p.items.iter().find(|i| i.path == f.path);
        out.push(Row {
            line: vec![
                span(format!("  {:>4}:{:<3} ", f.line, f.column), Role::Muted),
                span(format!("{} ", f.rule.code()), Role::Accent),
                span(
                    format!("{:<8}", f.severity.as_str()),
                    if f.severity == Severity::Error {
                        Role::Error
                    } else {
                        Role::Warn
                    },
                ),
                span(format!(" {}", f.message), Role::Normal),
            ],
            depth: 1,
            item: item.map(|i| i.id.clone()),
            open: Some(Open {
                path: f.path.clone(),
                line: f.line.max(1),
            }),
            file: (f.rule == Rule::K101).then(|| f.path.clone()),
            ..Row::default()
        });
    }
    out
}

/// A page whose data is read on a thread, until it is here.
fn waiting<T>(remote: Option<&Remote<T>>, what: &str) -> Option<Vec<Row>> {
    match remote {
        None | Some(Remote::NotAsked) | Some(Remote::Loading) => {
            Some(vec![note(&format!("{what}…"))])
        }
        Some(Remote::Failed(e)) => Some(vec![Row {
            line: vec![span(e.clone(), Role::Error)],
            ..Row::default()
        }]),
        Some(Remote::Ready(_)) => None,
    }
}

/// `kiln export --dry-run`, target by target, as the command line prints
/// it: the target's line as the heading, the rest under it.
fn exports(remote: Option<&Remote<Vec<ExportReport>>>) -> Vec<Row> {
    if let Some(rows) = waiting(remote, "Planning the exports") {
        return rows;
    }
    let Some(Remote::Ready(reports)) = remote else {
        return Vec::new();
    };
    if reports.is_empty() {
        return vec![note("kiln.toml declares no [[export]] targets.")];
    }
    let mut out = Vec::new();
    for r in reports {
        let mut lines = r.lines.iter();
        out.push(heading(
            lines.next().cloned().unwrap_or_else(|| r.name.clone()),
        ));
        let target = Some(r.name.clone());
        let mut body: Vec<(String, Role)> = lines
            .map(|line| {
                let t = line.trim_start();
                let role = if t.starts_with("refused") {
                    Role::Error
                } else if [
                    "warning",
                    "would",
                    "drift",
                    "leaves out",
                    "create",
                    "update",
                ]
                .iter()
                .any(|w| t.starts_with(w))
                {
                    Role::Warn
                } else if t.starts_with("unchanged") {
                    Role::Muted
                } else {
                    Role::Normal
                };
                (line.clone(), role)
            })
            .collect();
        if let Some(e) = &r.error {
            body.push((format!("  {e}"), Role::Error));
        }
        if body.is_empty() {
            body.push(("  (nothing to show)".into(), Role::Muted));
        }
        for (text, role) in body {
            out.push(Row {
                line: vec![span(text, role)],
                depth: 1,
                target: target.clone(),
                ..Row::default()
            });
        }
    }
    out
}

/// `kiln confluence comments`, under each target.
fn comments(p: &Project, remote: Option<&Remote<Vec<CommentGroup>>>) -> Vec<Row> {
    if let Some(rows) = waiting(remote, "Reading the comments") {
        return rows;
    }
    let Some(Remote::Ready(groups)) = remote else {
        return Vec::new();
    };
    if groups.is_empty() {
        return vec![note("No Confluence export target.")];
    }
    let mut out = Vec::new();
    for g in groups {
        if !g.published {
            out.push(heading(format!(
                "{} → space {}: not published yet",
                g.target, g.space
            )));
            continue;
        }
        let n = g.comments.len();
        out.push(heading(format!(
            "{} → space {}: {n} unresolved comment{}",
            g.target,
            g.space,
            if n == 1 { "" } else { "s" }
        )));
        for c in &g.comments {
            let about = c.item.clone().unwrap_or_else(|| "(page)".to_owned());
            let (item, open) = resolve(p, &about);
            let mut line = vec![
                span(format!("{about:<9}"), Role::Accent),
                span(format!(" {}, {}: ", c.author, c.date), Role::Muted),
                span(c.text.clone(), Role::Normal),
            ];
            if c.item.is_none() {
                line.push(span(format!("  on \"{}\"", c.page_title), Role::Muted));
            }
            out.push(Row {
                line,
                depth: 1,
                item,
                open,
                url: Some(c.url.clone()),
                ..Row::default()
            });
        }
    }
    out
}

/// The rows a filter keeps: those containing `text` (any case), the rows
/// above them they are nested under, and the headings over them.
pub fn filter(rows: Vec<Row>, text: &str) -> Vec<Row> {
    let needle = text.trim().to_lowercase();
    if needle.is_empty() {
        return rows;
    }
    let mut keep = vec![false; rows.len()];
    let mut stack: Vec<(u16, usize)> = Vec::new();
    let mut heading: Option<usize> = None;
    for (i, row) in rows.iter().enumerate() {
        if row.heading {
            heading = Some(i);
            stack.clear();
            continue;
        }
        while stack.last().is_some_and(|(d, _)| *d >= row.depth) {
            stack.pop();
        }
        if crate::screen::plain(&row.line)
            .to_lowercase()
            .contains(&needle)
        {
            keep[i] = true;
            for (_, a) in &stack {
                keep[*a] = true;
            }
            if let Some(h) = heading {
                keep[h] = true;
            }
        }
        stack.push((row.depth, i));
    }
    rows.into_iter()
        .zip(keep)
        .filter_map(|(r, k)| k.then_some(r))
        .collect()
}
