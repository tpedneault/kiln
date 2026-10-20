//! Assembly (EXPORT.md, "Templates", "Items in a document"): selecting
//! items, placing them where the template's directives say (or one
//! section per type without a template), each item once, as a flat list
//! in tree order or nested by `parent` (`nest=`; nested in `tree`), with
//! its properties and body; reports, the summary and the table of
//! contents; then numbering.

use std::collections::{HashMap, HashSet};

use kiln_index::{Project, Source};
use kiln_model::{ExportTarget, Finding, Item, ItemId, ItemStyle};
use kiln_render::{
    render_markdown, report, status_tone, Block, BodyContext, Directive, Inline, PropEntry, Props,
    ReportOptions, Since, Table, Target,
};

use crate::document::{Document, Node, Section, SectionKind};

/// The revision a document describes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Revision {
    /// The exported commit, when the project is in a git repository.
    pub commit: Option<String>,
    /// That commit's date, `YYYY-MM-DD`.
    pub date: Option<String>,
    /// The output is written into the repository itself, so it can't name
    /// the commit that will contain it (EXPORT.md, "Exports kept in the
    /// repository").
    pub in_tree: bool,
}

/// What assembly reads.
pub struct Assembly<'a> {
    pub project: &'a Project,
    /// `kiln check`'s findings, for the findings report.
    pub findings: &'a [Finding],
    pub revision: &'a Revision,
    /// `--since`: the baseline and every item change from it.
    pub since: Option<&'a Since>,
}

/// An assembled document and the items no part of it placed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assembled {
    pub document: Document,
    /// Open items selected by nothing: an omission is a decision, not an
    /// accident, so `--dry-run` lists them.
    pub left_out: Vec<ItemId>,
}

struct State<'a> {
    a: &'a Assembly<'a>,
    target: &'a ExportTarget,
    placed: HashSet<ItemId>,
    prefixes: Vec<String>,
}

/// Assembles `target`'s document; `template` is the template's text when
/// the target has one.
pub fn assemble(target: &ExportTarget, template: Option<&str>, a: &Assembly) -> Assembled {
    let mut state = State {
        a,
        target,
        placed: HashSet::new(),
        prefixes: a.project.config.prefixes(),
    };
    let mut nodes = match template {
        Some(text) => state.template_layout(text),
        None => state.default_layout(),
    };
    for node in &mut nodes {
        state.fill_values(node);
    }
    // Items a report lists and nothing else placed land on their row.
    // After everything else, so a report never takes an item from a later
    // `kiln:items`.
    for node in &mut nodes {
        claim_report_rows(node, &mut state.placed);
    }
    if target.numbering {
        number(&mut nodes, "", template.is_none());
    }
    let left_out = a
        .project
        .items
        .iter()
        .filter(|i| !state.placed.contains(&i.id) && !state.excluded(i))
        .map(|i| i.id.clone())
        .collect();
    Assembled {
        document: Document {
            title: target.title.clone(),
            nodes,
        },
        left_out,
    }
}

impl<'a> State<'a> {
    fn project(&self) -> &'a Project {
        self.a.project
    }

    /// Left out by the target's `exclude_status` (default: each type's
    /// closed statuses).
    fn excluded(&self, item: &Item) -> bool {
        match &self.target.exclude_status {
            Some(list) => list.contains(&item.status),
            None => self.project().is_closed(item),
        }
    }

    /// Without a template: summary, table of contents, a section per type,
    /// reports as appendices.
    fn default_layout(&mut self) -> Vec<Node> {
        let mut nodes: Vec<Node> = self.summary().into_iter().map(Node::Block).collect();
        nodes.push(Node::Block(Block::Toc));
        let types = self.target.types.clone();
        for type_name in &types {
            let Some(ty) = self.project().config.type_named(type_name) else {
                continue;
            };
            let selected: Vec<usize> = self
                .project()
                .items
                .iter()
                .enumerate()
                .filter(|(_, i)| {
                    i.type_name == *type_name && !self.excluded(i) && !self.placed.contains(&i.id)
                })
                .map(|(n, _)| n)
                .collect();
            if selected.is_empty() {
                continue;
            }
            let nest = self.target.nests_by_default();
            let children = self.place(&selected, 2, true, nest);
            nodes.push(Node::Section(Section {
                number: String::new(),
                title: vec![Inline::text(ty.plural.clone())],
                depth: 1,
                kind: SectionKind::Heading,
                own_page: false,
                marker: None,
                nodes: children,
            }));
        }
        for name in self.target.reports.clone() {
            let blocks = report(
                &name,
                self.project(),
                self.a.findings,
                &ReportOptions {
                    since: self.a.since,
                    ..Default::default()
                },
            )
            .unwrap_or_default();
            nodes.push(Node::Section(Section {
                number: String::new(),
                title: vec![Inline::text(capitalised(&name))],
                depth: 1,
                kind: SectionKind::Report(name),
                own_page: false,
                marker: None,
                nodes: blocks.into_iter().map(Node::Block).collect(),
            }));
        }
        nodes
    }

    fn template_layout(&mut self, text: &str) -> Vec<Node> {
        let ctx = BodyContext {
            dir: self
                .target
                .template
                .as_deref()
                .and_then(|t| t.rsplit_once('/'))
                .map(|(d, _)| d)
                .unwrap_or(""),
            first_line: 1,
            prefixes: &self.prefixes,
            link_bare_ids: self.project().config.link_bare_ids,
            template: true,
        };
        let blocks = render_markdown(text, &ctx);
        let base = blocks
            .iter()
            .filter_map(|b| {
                if let Block::Heading { level, .. } = b {
                    Some(*level)
                } else {
                    None
                }
            })
            .min()
            .unwrap_or(1);
        let mut top: Vec<Node> = Vec::new();
        let mut open: Vec<Section> = Vec::new();
        for block in blocks {
            match block {
                Block::Heading { level, content } => {
                    let depth = level - base + 1;
                    close_to(&mut open, &mut top, depth);
                    open.push(Section {
                        number: String::new(),
                        title: content,
                        depth,
                        kind: SectionKind::Heading,
                        own_page: false,
                        marker: None,
                        nodes: Vec::new(),
                    });
                }
                Block::Directive(d) => {
                    let depth = open.last().map(|s| s.depth).unwrap_or(0) + 1;
                    let nodes = self.directive(&d, depth);
                    match open.last_mut() {
                        Some(section) => section.nodes.extend(nodes),
                        None => top.extend(nodes),
                    }
                }
                block => match open.last_mut() {
                    Some(section) => section.nodes.push(Node::Block(block)),
                    None => top.push(Node::Block(block)),
                },
            }
        }
        close_to(&mut open, &mut top, 1);
        top
    }

    fn directive(&mut self, d: &Directive, depth: u8) -> Vec<Node> {
        match d.name.as_str() {
            "summary" => self.summary().into_iter().map(Node::Block).collect(),
            "toc" => vec![Node::Block(Block::Toc)],
            "report" => {
                let name = d.positional.first().map(String::as_str).unwrap_or("");
                let options = ReportOptions {
                    type_name: d.attr("type").map(str::to_owned),
                    since: self.a.since,
                };
                report(name, self.project(), self.a.findings, &options)
                    .unwrap_or_default()
                    .into_iter()
                    .map(Node::Block)
                    .collect()
            }
            "item" => {
                let found = d
                    .positional
                    .first()
                    .and_then(|s| s.parse::<ItemId>().ok())
                    .and_then(|id| self.project().indices(&id).first().copied());
                match found {
                    Some(i) if !self.placed.contains(&self.project().items[i].id) => {
                        self.place(&[i], depth, true, false)
                    }
                    _ => Vec::new(),
                }
            }
            "items" => {
                let selected = self.select(d);
                if d.attr("style") == Some("row") {
                    for &i in &selected {
                        self.placed.insert(self.project().items[i].id.clone());
                    }
                    if selected.is_empty() {
                        return Vec::new();
                    }
                    vec![Node::Block(self.row_table(&selected))]
                } else {
                    let nest = match d.attr("nest") {
                        Some("flat") => false,
                        Some("parent") => true,
                        _ => self.target.nests_by_default(),
                    };
                    self.place(&selected, depth, d.attr("split") != Some("no"), nest)
                }
            }
            // `kiln check` reports anything else (K112).
            _ => Vec::new(),
        }
    }

    /// Indices of the items a `kiln:items` directive selects, in project
    /// order, not yet placed.
    fn select(&self, d: &Directive) -> Vec<usize> {
        let p = self.project();
        let list = |key: &str| {
            d.attr(key).map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_owned())
                    .collect::<Vec<_>>()
            })
        };
        let types = list("type");
        let statuses = list("status");
        let root: Option<HashSet<ItemId>> = d
            .attr("root")
            .and_then(|r| r.parse::<ItemId>().ok())
            .map(|root| {
                let mut set = HashSet::from([root.clone()]);
                let mut queue = vec![root];
                while let Some(id) = queue.pop() {
                    for child in p.children(&id) {
                        if set.insert(child.id.clone()) {
                            queue.push(child.id.clone());
                        }
                    }
                }
                set
            });
        p.items
            .iter()
            .enumerate()
            .filter(|(_, i)| !self.placed.contains(&i.id))
            .filter(|(_, i)| types.as_ref().is_none_or(|t| t.contains(&i.type_name)))
            .filter(|(_, i)| match &statuses {
                // An explicit status filter decides, even for statuses the
                // target excludes by default (a decision log showing
                // superseded decisions).
                Some(s) => {
                    let (exclude, include): (Vec<&String>, Vec<&String>) =
                        s.iter().partition(|x| x.starts_with('!'));
                    let excluded = exclude.iter().any(|x| x[1..] == i.status);
                    let included = include.is_empty() || include.iter().any(|x| **x == i.status);
                    !excluded && included && (include.contains(&&i.status) || !self.excluded(i))
                }
                None => !self.excluded(i),
            })
            .filter(|(_, i)| d.attr("tag").is_none_or(|t| i.tags.iter().any(|x| x == t)))
            .filter(|(_, i)| root.as_ref().is_none_or(|r| r.contains(&i.id)))
            .filter(|(_, i)| {
                d.attr("jira")
                    .is_none_or(|pattern| i.jira.iter().any(|k| glob(pattern, k)))
            })
            .map(|(n, _)| n)
            .collect()
    }

    /// Item sections for `selected` at `depth`: nested by `parent` within
    /// the selection, or (`nest` false) all at `depth` in the same order,
    /// each parent followed by its children, so related items stay
    /// together while every one is a heading of the same level.
    fn place(&mut self, selected: &[usize], depth: u8, own_page: bool, nest: bool) -> Vec<Node> {
        let p = self.project();
        let in_selection: HashSet<&ItemId> = selected.iter().map(|&i| &p.items[i].id).collect();
        let mut children: HashMap<ItemId, Vec<usize>> = HashMap::new();
        let mut roots = Vec::new();
        for &i in selected {
            let item = &p.items[i];
            match item.parent.as_ref().and_then(|r| r.id.as_ref().ok()) {
                Some(parent) if in_selection.contains(parent) && *parent != item.id => {
                    children.entry(parent.clone()).or_default().push(i)
                }
                _ => roots.push(i),
            }
        }
        let mut out = Vec::new();
        if !nest {
            let mut order = Vec::new();
            let mut seen = HashSet::new();
            for &i in &roots {
                tree_order(p, i, &children, &mut seen, &mut order);
            }
            let none = HashMap::new();
            for i in order {
                if !self.placed.contains(&self.project().items[i].id) {
                    out.push(Node::Section(self.item_section(
                        i,
                        depth,
                        own_page,
                        &none,
                        &mut HashSet::new(),
                    )));
                }
            }
            return out;
        }
        for i in roots {
            out.push(Node::Section(self.item_section(
                i,
                depth,
                own_page,
                &children,
                &mut HashSet::new(),
            )));
        }
        out
    }

    fn item_section(
        &mut self,
        i: usize,
        depth: u8,
        own_page: bool,
        children: &HashMap<ItemId, Vec<usize>>,
        path: &mut HashSet<ItemId>,
    ) -> Section {
        let p = self.a.project;
        let item = &p.items[i];
        self.placed.insert(item.id.clone());
        path.insert(item.id.clone());
        let mut nodes = Vec::new();
        let style = self.target.item_style();
        if style != ItemStyle::None {
            nodes.push(Node::Block(Block::Props(self.props(item, style))));
        }
        let dir = item.path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let ctx = BodyContext {
            dir,
            first_line: item.body_line,
            prefixes: &self.prefixes,
            link_bare_ids: p.config.link_bare_ids,
            template: false,
        };
        for block in render_markdown(&item.body, &ctx) {
            nodes.push(Node::Block(match block {
                Block::Heading { level, content } => Block::Heading {
                    level: depth.saturating_add(level),
                    content,
                },
                other => other,
            }));
        }
        for &c in children.get(&item.id).map(Vec::as_slice).unwrap_or(&[]) {
            if !path.contains(&p.items[c].id) && !self.placed.contains(&p.items[c].id) {
                nodes.push(Node::Section(self.item_section(
                    c,
                    depth.saturating_add(1),
                    own_page,
                    children,
                    path,
                )));
            }
        }
        path.remove(&item.id);
        Section {
            number: String::new(),
            title: vec![Inline::text(format!("{} {}", item.id, item.title))],
            depth,
            kind: SectionKind::Item(item.id.clone()),
            own_page,
            marker: self
                .a
                .since
                .and_then(|s| s.kind_of(&item.id))
                .filter(|k| *k != kiln_index::ChangeKind::Removed),
            nodes,
        }
    }

    fn props(&self, item: &Item, style: ItemStyle) -> Props {
        let p = self.project();
        let mut entries = Vec::new();
        let r = |id: &ItemId| {
            vec![Inline::Ref {
                id: id.clone(),
                text: None,
            }]
        };
        if style == ItemStyle::Table {
            if let Some(ty) = p.type_of(item) {
                entries.push(PropEntry {
                    label: "Type".into(),
                    lead: String::new(),
                    values: vec![vec![Inline::text(ty.title.clone())]],
                });
            }
        }
        if let Some(Ok(parent)) = item.parent.as_ref().map(|r| &r.id) {
            entries.push(PropEntry {
                label: "Parent".into(),
                lead: "under".into(),
                values: vec![r(parent)],
            });
        }
        let mut roles: Vec<&str> = Vec::new();
        for link in &item.links {
            if !roles.contains(&link.role.as_str()) {
                roles.push(&link.role);
            }
        }
        for role in roles {
            let values = item
                .links
                .iter()
                .filter(|l| l.role == role)
                .filter_map(|l| l.target.id.as_ref().ok())
                .map(&r)
                .collect();
            entries.push(PropEntry {
                label: capitalised(role),
                lead: role.to_owned(),
                values,
            });
        }
        for role in &p.config.links {
            let sources: Vec<Source> = p
                .inbound(&item.id)
                .iter()
                .filter(|l| l.role == role.name)
                .map(|l| l.source)
                .collect();
            if sources.is_empty() {
                continue;
            }
            let values = sources
                .iter()
                .map(|s| vec![kiln_render_source(p, *s)])
                .collect();
            entries.push(PropEntry {
                label: capitalised(&role.inverse),
                lead: role.inverse.clone(),
                values,
            });
        }
        if !item.jira.is_empty() {
            let values = item
                .jira
                .iter()
                .map(|k| vec![Inline::Jira(k.clone())])
                .collect();
            entries.push(PropEntry {
                label: "Jira".into(),
                lead: "Jira".into(),
                values,
            });
        }
        if style == ItemStyle::Table {
            if !item.tags.is_empty() {
                let values = item
                    .tags
                    .iter()
                    .map(|t| vec![Inline::text(t.clone())])
                    .collect();
                entries.push(PropEntry {
                    label: "Tags".into(),
                    lead: String::new(),
                    values,
                });
            }
            if let Some(owner) = &item.owner {
                entries.push(PropEntry {
                    label: "Owner".into(),
                    lead: String::new(),
                    values: vec![vec![Inline::text(owner.clone())]],
                });
            }
            if let Some(due) = item.due {
                entries.push(PropEntry {
                    label: "Due".into(),
                    lead: String::new(),
                    values: vec![vec![Inline::text(due.to_string())]],
                });
            }
            entries.push(PropEntry {
                label: "Source".into(),
                lead: String::new(),
                values: vec![vec![Inline::Link {
                    target: Target::Repo {
                        path: item.path.clone(),
                        line: None,
                        fragment: None,
                    },
                    content: vec![Inline::text(item.path.clone())],
                }]],
            });
        }
        Props {
            style,
            status: Inline::Status {
                text: item.status.clone(),
                tone: status_tone(p, item),
            },
            entries,
        }
    }

    /// `style="row"`: one table, bodies left out.
    fn row_table(&self, selected: &[usize]) -> Block {
        let p = self.project();
        let rows = selected
            .iter()
            .map(|&i| {
                let item = &p.items[i];
                let mut links = Vec::new();
                for (n, link) in item.links.iter().enumerate() {
                    if n > 0 {
                        links.push(Inline::text(", "));
                    }
                    links.push(Inline::text(format!("{} ", link.role)));
                    match &link.target.id {
                        Ok(id) => links.push(Inline::Ref {
                            id: id.clone(),
                            text: Some(id.to_string()),
                        }),
                        Err(_) => links.push(Inline::text(link.target.raw.clone())),
                    }
                }
                vec![
                    vec![
                        Inline::Anchor(item.id.to_string()),
                        Inline::text(item.id.to_string()),
                    ],
                    vec![Inline::text(item.title.clone())],
                    vec![Inline::Status {
                        text: item.status.clone(),
                        tone: status_tone(p, item),
                    }],
                    links,
                ]
            })
            .collect();
        Block::Table(Table {
            align: vec![kiln_render::Align::None; 4],
            header: ["ID", "Title", "Status", "Links"]
                .iter()
                .map(|h| vec![Inline::text(*h)])
                .collect(),
            rows,
        })
    }

    /// The commit, counts by type and status, coverage and milestone
    /// progress.
    fn summary(&self) -> Vec<Block> {
        let p = self.project();
        let rev = self.a.revision;
        let origin = if rev.in_tree {
            "Generated by Kiln from the files in this repository.".to_owned()
        } else {
            match (&rev.commit, &rev.date) {
                (Some(c), Some(d)) => format!("Generated by Kiln from commit {} ({d}).", short(c)),
                (Some(c), None) => format!("Generated by Kiln from commit {}.", short(c)),
                _ => "Generated by Kiln from files that aren't in a git repository.".to_owned(),
            }
        };
        let mut blocks = vec![Block::Paragraph(vec![Inline::text(origin)])];
        let mut rows = Vec::new();
        for ty in &p.config.types {
            let items: Vec<&Item> = p.items.iter().filter(|i| i.type_name == ty.name).collect();
            if items.is_empty() {
                continue;
            }
            let by_status: Vec<String> = ty
                .statuses
                .iter()
                .filter_map(|s| {
                    let n = items.iter().filter(|i| &i.status == s).count();
                    (n > 0).then(|| format!("{s} {n}"))
                })
                .collect();
            rows.push(vec![
                vec![Inline::text(ty.plural.clone())],
                vec![Inline::text(items.len().to_string())],
                vec![Inline::text(by_status.join(", "))],
            ]);
        }
        if !rows.is_empty() {
            blocks.push(Block::Table(Table {
                align: vec![
                    kiln_render::Align::None,
                    kiln_render::Align::Right,
                    kiln_render::Align::None,
                ],
                header: ["Type", "Items", "By status"]
                    .iter()
                    .map(|h| vec![Inline::text(*h)])
                    .collect(),
                rows,
            }));
        }
        let mut coverage = Vec::new();
        for ty in &p.config.types {
            let roles: Vec<&kiln_model::LinkRole> = p
                .config
                .coverage_roles
                .iter()
                .filter_map(|r| p.config.role(r))
                .filter(|r| r.allows_to(&ty.name))
                .collect();
            let open: Vec<&Item> = p
                .items
                .iter()
                .filter(|i| i.type_name == ty.name && !p.is_closed(i))
                .collect();
            if roles.is_empty() || open.is_empty() {
                continue;
            }
            let parts: Vec<String> = roles
                .iter()
                .map(|r| {
                    format!(
                        "{} {} of {}",
                        r.inverse,
                        open.iter().filter(|i| p.covered(&i.id, &r.name)).count(),
                        open.len()
                    )
                })
                .collect();
            coverage.push(format!("{}: {}.", ty.plural, parts.join(", ")));
        }
        if !coverage.is_empty() {
            blocks.push(Block::Paragraph(vec![Inline::text(coverage.join(" "))]));
        }
        let progress: Vec<Vec<Inline>> = p
            .items
            .iter()
            .filter(|i| i.type_name == "milestone" && !p.is_closed(i))
            .filter_map(|m| {
                p.progress(&m.id).map(|pr| {
                    vec![
                        Inline::Ref {
                            id: m.id.clone(),
                            text: None,
                        },
                        Inline::text(format!(": {} of {} done", pr.done, pr.total)),
                    ]
                })
            })
            .collect();
        if !progress.is_empty() {
            blocks.push(Block::List {
                start: None,
                items: progress
                    .into_iter()
                    .map(|c| kiln_render::ListItem {
                        task: None,
                        content: vec![Block::Paragraph(c)],
                    })
                    .collect(),
            });
        }
        blocks
    }

    fn fill_values(&self, node: &mut Node) {
        match node {
            Node::Block(b) => fill_block(b, &|name| self.value(name)),
            Node::Section(s) => {
                fill_inlines(&mut s.title, &|name| self.value(name));
                for n in &mut s.nodes {
                    self.fill_values(n);
                }
            }
        }
    }

    fn value(&self, name: &str) -> String {
        let rev = self.a.revision;
        match name {
            "name" => self.project().config.name.clone(),
            "commit" if rev.in_tree => "this revision".to_owned(),
            "commit" => rev
                .commit
                .as_deref()
                .map(short)
                .unwrap_or("(no commit)")
                .to_owned(),
            "date" if rev.in_tree => String::new(),
            "date" => rev.date.clone().unwrap_or_default(),
            "since" => self.a.since.map(Since::describe).unwrap_or_default(),
            _ => String::new(),
        }
    }
}

fn kiln_render_source(p: &Project, s: Source) -> Inline {
    match s {
        Source::Item(i) => Inline::Ref {
            id: p.items[i].id.clone(),
            text: None,
        },
        Source::Code(i) => {
            let code = &p.code_refs[i];
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

fn fill_block(block: &mut Block, value: &dyn Fn(&str) -> String) {
    match block {
        Block::Heading { content, .. } | Block::Paragraph(content) => fill_inlines(content, value),
        Block::Quote(blocks)
        | Block::Callout {
            content: blocks, ..
        } => blocks.iter_mut().for_each(|b| fill_block(b, value)),
        Block::List { items, .. } => items
            .iter_mut()
            .for_each(|i| i.content.iter_mut().for_each(|b| fill_block(b, value))),
        Block::Table(t) => {
            t.header.iter_mut().for_each(|c| fill_inlines(c, value));
            t.rows
                .iter_mut()
                .flatten()
                .for_each(|c| fill_inlines(c, value));
        }
        _ => {}
    }
}

fn fill_inlines(inlines: &mut [Inline], value: &dyn Fn(&str) -> String) {
    for inline in inlines {
        match inline {
            Inline::Value(name) => *inline = Inline::Text(value(name)),
            Inline::Emph(c)
            | Inline::Strong(c)
            | Inline::Strike(c)
            | Inline::Link { content: c, .. } => fill_inlines(c, value),
            _ => {}
        }
    }
}

/// Gives report rows whose item is placed nowhere else an anchor, and
/// counts them as placed.
fn claim_report_rows(node: &mut Node, placed: &mut HashSet<ItemId>) {
    match node {
        Node::Section(s) => s
            .nodes
            .iter_mut()
            .for_each(|n| claim_report_rows(n, placed)),
        Node::Block(Block::Table(t)) => {
            for row in &mut t.rows {
                let Some(cell) = row.first_mut() else {
                    continue;
                };
                if let [Inline::Ref {
                    id,
                    text: Some(text),
                }] = cell.as_slice()
                {
                    if *text == id.to_string() && !placed.contains(id) {
                        placed.insert(id.clone());
                        *cell = vec![Inline::Anchor(id.to_string()), Inline::text(text.clone())];
                    }
                }
            }
        }
        Node::Block(_) => {}
    }
}

/// Appends `i` and its descendants among `children`, depth first, each
/// once (a parent cycle can't loop).
fn tree_order(
    p: &Project,
    i: usize,
    children: &HashMap<ItemId, Vec<usize>>,
    seen: &mut HashSet<usize>,
    out: &mut Vec<usize>,
) {
    if !seen.insert(i) {
        return;
    }
    out.push(i);
    for &c in children
        .get(&p.items[i].id)
        .map(Vec::as_slice)
        .unwrap_or(&[])
    {
        tree_order(p, c, children, seen, out);
    }
}

/// Closes open sections down to (and including) `depth`, attaching each
/// to its parent.
fn close_to(open: &mut Vec<Section>, top: &mut Vec<Node>, depth: u8) {
    while open.last().is_some_and(|s| s.depth >= depth) {
        let Some(done) = open.pop() else { break };
        match open.last_mut() {
            Some(parent) => parent.nodes.push(Node::Section(done)),
            None => top.push(Node::Section(done)),
        }
    }
}

/// Numbers sections `2.1.3`; without a template, top-level reports are
/// appendices `A`, `B`.
fn number(nodes: &mut [Node], prefix: &str, appendices: bool) {
    let mut n = 0;
    let mut letter = 0u8;
    for node in nodes {
        let Node::Section(s) = node else { continue };
        s.number = if appendices && matches!(s.kind, SectionKind::Report(_)) {
            letter += 1;
            ((b'A' + letter - 1) as char).to_string()
        } else {
            n += 1;
            if prefix.is_empty() {
                n.to_string()
            } else {
                format!("{prefix}.{n}")
            }
        };
        let child_prefix = s.number.clone();
        number(&mut s.nodes, &child_prefix, false);
    }
}

/// `FLINT-*`: `*` matches any run of characters.
fn glob(pattern: &str, text: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == text,
        Some((head, tail)) => text.starts_with(head) && glob_rest(tail, &text[head.len()..]),
    }
}

fn glob_rest(pattern: &str, text: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }
    (0..=text.len())
        .filter(|&i| text.is_char_boundary(i))
        .any(|i| glob(pattern, &text[i..]))
}

/// The first 12 characters of a commit id.
pub fn short(commit: &str) -> &str {
    &commit[..commit.len().min(12)]
}

pub fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::glob;

    #[test]
    fn jira_globs() {
        assert!(glob("FLINT-*", "FLINT-88"));
        assert!(glob("*-88", "FLINT-88"));
        assert!(!glob("FLINT-*", "FENIX-1"));
        assert!(glob("FLINT-88", "FLINT-88"));
        assert!(glob("A*B*", "AB"));
    }
}
