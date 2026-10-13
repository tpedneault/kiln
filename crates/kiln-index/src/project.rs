//! A loaded project and the graph built from it: forward and inverse
//! links (from items and code locations), the parent tree, mentions; and
//! the queries above it (FORMAT.md, "The model Kiln computes").

use std::collections::{HashMap, HashSet, VecDeque};

use kiln_model::{CodeRef, Config, Finding, Item, ItemId, Rule, Span, TypeDef};
use kiln_parse::{
    is_scannable, normalize, parse_config, parse_item, scan_code, CodePattern, ItemOptions,
    CONFIG_FILE,
};
use rayon::prelude::*;

use crate::source::FileSource;

/// Why a project couldn't be loaded at all.
#[derive(Debug)]
pub enum LoadError {
    /// `kiln.toml` couldn't be read.
    Read(std::io::Error),
    /// `kiln.toml` couldn't be interpreted: the findings say why (K001,
    /// or a `format` this Kiln doesn't read).
    Config(Vec<Finding>),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Read(e) => write!(f, "{CONFIG_FILE}: can't read it: {e}"),
            LoadError::Config(findings) => match findings.first() {
                Some(first) => write!(f, "{first}"),
                None => write!(f, "{CONFIG_FILE} can't be used"),
            },
        }
    }
}

impl std::error::Error for LoadError {}

/// Where a link comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    /// An index into [`Project::items`].
    Item(usize),
    /// An index into [`Project::code_refs`].
    Code(usize),
}

/// A link arriving at an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inbound {
    pub source: Source,
    pub role: String,
}

/// One step on an impact path: `from` reaches `to` by `relation` (a link
/// role, or `parent` for a child).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub from: Source,
    pub relation: String,
    pub to: ItemId,
}

/// Something that reaches the item asked about, and how: the steps from
/// it to that item, nearest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Impact {
    pub source: Source,
    pub path: Vec<Step>,
}

/// Milestone progress: done ÷ total over its non-closed items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
}

/// A loaded project.
#[derive(Debug)]
pub struct Project {
    pub config: Config,
    /// Ordered by type (in `kiln.toml` order), then ID, then path.
    pub items: Vec<Item>,
    /// Ordered by path, then position.
    pub code_refs: Vec<CodeRef>,
    /// Code files that weren't UTF-8 and were scanned lossily; their spans
    /// can't be edited (`renumber` leaves them alone).
    pub lossy_code_files: HashSet<String>,
    /// Findings from loading: configuration, parsing, missing folders.
    pub findings: Vec<Finding>,
    by_id: HashMap<ItemId, Vec<usize>>,
    inbound: HashMap<ItemId, Vec<Inbound>>,
    children: HashMap<ItemId, Vec<usize>>,
    mentioned_by: HashMap<ItemId, Vec<usize>>,
}

impl Project {
    /// Reads `kiln.toml`, every item file and every code file, in
    /// parallel, and builds the graph.
    // implements: REQ-007
    pub fn load(source: &dyn FileSource) -> Result<Project, LoadError> {
        let bytes = source.read(CONFIG_FILE).map_err(LoadError::Read)?;
        let text = normalize(&bytes).ok_or_else(|| {
            LoadError::Config(vec![Finding::new(
                Rule::K001,
                CONFIG_FILE,
                Span::default(),
                "kiln.toml isn't UTF-8",
            )])
        })?;
        let (config, mut findings) = parse_config(&text);
        let Some(config) = config else {
            return Err(LoadError::Config(findings));
        };
        let missing_dir = |dir: &str, key: &str| {
            Finding::new(
                Rule::K002,
                CONFIG_FILE,
                Span::default(),
                format!("{key}: `{dir}` doesn't exist"),
            )
        };

        let item_listing = match source.list(&config.paths, &[], Some("md")) {
            Ok(l) => l,
            Err(e) => {
                findings.push(Finding::new(
                    Rule::K002,
                    CONFIG_FILE,
                    Span::default(),
                    format!("paths: {e}"),
                ));
                Default::default()
            }
        };
        findings.extend(item_listing.missing.iter().map(|d| missing_dir(d, "paths")));

        let prefixes = config.prefixes();
        let options = ItemOptions {
            prefixes: &prefixes,
            link_bare_ids: config.link_bare_ids,
        };
        let parsed: Vec<(Option<Item>, Vec<Finding>, bool)> = item_listing
            .files
            .par_iter()
            .map(|path| match source.read(path) {
                Err(e) => (
                    None,
                    vec![Finding::new(
                        Rule::K100,
                        path,
                        Span::default(),
                        format!("can't read it: {e}"),
                    )],
                    false,
                ),
                Ok(bytes) => match normalize(&bytes) {
                    None => {
                        let is_item = bytes.starts_with(b"+++");
                        let f =
                            Finding::new(Rule::K100, path, Span::default(), "the file isn't UTF-8");
                        (None, if is_item { vec![f] } else { vec![] }, is_item)
                    }
                    Some(text) => {
                        let is_item = kiln_parse::sections(&text).is_some();
                        let (item, f) = parse_item(path, &text, &options);
                        (item, f, is_item)
                    }
                },
            })
            .collect();
        let mut item_files: HashSet<&str> = HashSet::new();
        let mut items = Vec::new();
        for ((item, f, is_item), path) in parsed.into_iter().zip(&item_listing.files) {
            findings.extend(f);
            if is_item {
                item_files.insert(path);
            }
            items.extend(item);
        }

        let mut code_refs = Vec::new();
        let mut lossy_code_files = HashSet::new();
        let patterns: Vec<CodePattern> = config
            .code_patterns()
            .iter()
            .filter_map(|p| kiln_parse::compile_pattern(p).ok())
            .collect();
        if !config.code.paths.is_empty() && !patterns.is_empty() {
            match source.list(&config.code.paths, &config.code.exclude, None) {
                Err(e) => findings.push(Finding::new(
                    Rule::K002,
                    CONFIG_FILE,
                    Span::default(),
                    format!("code: {e}"),
                )),
                Ok(listing) => {
                    findings.extend(listing.missing.iter().map(|d| missing_dir(d, "code.paths")));
                    let scanned: Vec<(Vec<CodeRef>, Option<String>)> = listing
                        .files
                        .par_iter()
                        .filter(|path| !item_files.contains(path.as_str()))
                        .filter_map(|path| {
                            let bytes = source.read(path).ok()?;
                            if !is_scannable(&bytes) {
                                return None;
                            }
                            match std::str::from_utf8(&bytes) {
                                Ok(text) => Some((scan_code(path, text, &patterns), None)),
                                Err(_) => {
                                    let text = String::from_utf8_lossy(&bytes);
                                    let refs = scan_code(path, &text, &patterns);
                                    let lossy = (!refs.is_empty()).then(|| path.clone());
                                    Some((refs, lossy))
                                }
                            }
                        })
                        .collect();
                    for (refs, lossy) in scanned {
                        code_refs.extend(refs);
                        lossy_code_files.extend(lossy);
                    }
                }
            }
        }

        let type_order = |name: &str| {
            config
                .types
                .iter()
                .position(|t| t.name == name)
                .unwrap_or(usize::MAX)
        };
        items.sort_by(|a, b| {
            (type_order(&a.type_name), &a.id, &a.path).cmp(&(
                type_order(&b.type_name),
                &b.id,
                &b.path,
            ))
        });
        code_refs
            .sort_by(|a, b| (&a.path, a.target.span.start).cmp(&(&b.path, b.target.span.start)));

        let mut project = Project {
            config,
            items,
            code_refs,
            lossy_code_files,
            findings,
            by_id: HashMap::new(),
            inbound: HashMap::new(),
            children: HashMap::new(),
            mentioned_by: HashMap::new(),
        };
        project.build_graph();
        Ok(project)
    }

    fn build_graph(&mut self) {
        for (i, item) in self.items.iter().enumerate() {
            self.by_id.entry(item.id.clone()).or_default().push(i);
            if let Some(Ok(parent)) = item.parent.as_ref().map(|p| &p.id) {
                self.children.entry(parent.clone()).or_default().push(i);
            }
            for link in &item.links {
                if let Ok(target) = &link.target.id {
                    self.inbound
                        .entry(target.clone())
                        .or_default()
                        .push(Inbound {
                            source: Source::Item(i),
                            role: link.role.clone(),
                        });
                }
            }
            let mut seen = HashSet::new();
            for mention in &item.mentions {
                if let Ok(target) = &mention.target.id {
                    if *target != item.id && seen.insert(target.clone()) {
                        self.mentioned_by.entry(target.clone()).or_default().push(i);
                    }
                }
            }
        }
        for (i, code) in self.code_refs.iter().enumerate() {
            if let Ok(target) = &code.target.id {
                self.inbound
                    .entry(target.clone())
                    .or_default()
                    .push(Inbound {
                        source: Source::Code(i),
                        role: code.role.clone(),
                    });
            }
        }
    }

    /// The item with `id`; the first one when the ID is duplicated.
    pub fn item(&self, id: &ItemId) -> Option<&Item> {
        self.by_id
            .get(id)
            .and_then(|v| v.first())
            .map(|&i| &self.items[i])
    }

    /// Indices of every item with `id` (more than one is K101).
    pub fn indices(&self, id: &ItemId) -> &[usize] {
        self.by_id.get(id).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn ids(&self) -> impl Iterator<Item = &ItemId> {
        self.by_id.keys()
    }

    pub fn type_of(&self, item: &Item) -> Option<&TypeDef> {
        self.config.type_named(&item.type_name)
    }

    pub fn is_closed(&self, item: &Item) -> bool {
        self.type_of(item)
            .is_some_and(|t| t.is_closed(&item.status))
    }

    pub fn is_done(&self, item: &Item) -> bool {
        self.type_of(item).is_some_and(|t| t.is_done(&item.status))
    }

    /// Whether a link source counts: code locations always do; items
    /// unless closed.
    pub fn source_is_open(&self, source: Source) -> bool {
        match source {
            Source::Code(_) => true,
            Source::Item(i) => !self.is_closed(&self.items[i]),
        }
    }

    /// `REQ-014`, or `crates/x/src/lib.rs:12` for a code location.
    pub fn describe(&self, source: Source) -> String {
        match source {
            Source::Item(i) => self.items[i].id.to_string(),
            Source::Code(i) => self.code_refs[i].location(),
        }
    }

    /// Items whose `parent` is `id`, in item order.
    pub fn children(&self, id: &ItemId) -> Vec<&Item> {
        self.children
            .get(id)
            .map(|v| v.iter().map(|&i| &self.items[i]).collect())
            .unwrap_or_default()
    }

    /// The chain of parents above `id`, nearest first, stopping at a cycle
    /// or a missing parent.
    pub fn ancestors(&self, id: &ItemId) -> Vec<&Item> {
        let mut out = Vec::new();
        let mut seen = HashSet::from([id.clone()]);
        let mut at = self.item(id);
        while let Some(Some(Ok(parent))) = at.map(|i| i.parent.as_ref().map(|p| &p.id)) {
            if !seen.insert(parent.clone()) {
                break;
            }
            match self.item(parent) {
                Some(p) => {
                    out.push(p);
                    at = Some(p);
                }
                None => break,
            }
        }
        out
    }

    /// Links arriving at `id`, from items and code, in source order.
    pub fn inbound(&self, id: &ItemId) -> &[Inbound] {
        self.inbound.get(id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Items whose bodies mention `id`.
    pub fn mentioned_by(&self, id: &ItemId) -> Vec<&Item> {
        self.mentioned_by
            .get(id)
            .map(|v| v.iter().map(|&i| &self.items[i]).collect())
            .unwrap_or_default()
    }

    /// Whether `id` is covered for `role`: a non-closed source links to it
    /// with that role, or it has non-closed children and all of them are
    /// covered (FORMAT.md, "Coverage").
    pub fn covered(&self, id: &ItemId, role: &str) -> bool {
        self.covered_inner(id, role, &mut HashSet::new())
    }

    fn covered_inner(&self, id: &ItemId, role: &str, seen: &mut HashSet<ItemId>) -> bool {
        if !seen.insert(id.clone()) {
            return false;
        }
        let direct = self
            .inbound(id)
            .iter()
            .any(|link| link.role == role && self.source_is_open(link.source));
        if direct {
            return true;
        }
        let children: Vec<&Item> = self
            .children(id)
            .into_iter()
            .filter(|c| !self.is_closed(c))
            .collect();
        !children.is_empty()
            && children
                .iter()
                .all(|c| self.covered_inner(&c.id, role, seen))
    }

    /// For a milestone: done ÷ (all − closed) over items `part-of` it or
    /// any milestone it `depends-on`, transitively. `None` when nothing
    /// counts.
    pub fn progress(&self, id: &ItemId) -> Option<Progress> {
        let mut milestones = HashSet::from([id.clone()]);
        let mut queue = VecDeque::from([id.clone()]);
        while let Some(m) = queue.pop_front() {
            for item in self.indices(&m).iter().map(|&i| &self.items[i]) {
                for link in item.links.iter().filter(|l| l.role == "depends-on") {
                    if let Ok(dep) = &link.target.id {
                        let is_milestone =
                            self.item(dep).is_some_and(|d| d.type_name == "milestone");
                        if is_milestone && milestones.insert(dep.clone()) {
                            queue.push_back(dep.clone());
                        }
                    }
                }
            }
        }
        let mut seen = HashSet::new();
        let mut progress = Progress { done: 0, total: 0 };
        for m in &milestones {
            for link in self.inbound(m).iter().filter(|l| l.role == "part-of") {
                if let Source::Item(i) = link.source {
                    let item = &self.items[i];
                    if seen.insert(i) && !self.is_closed(item) {
                        progress.total += 1;
                        progress.done += usize::from(self.is_done(item));
                    }
                }
            }
        }
        (progress.total > 0).then_some(progress)
    }

    /// Everything that reaches `id` by a link or a parent edge,
    /// transitively, breadth first, each with its path (FORMAT.md,
    /// "Impact").
    pub fn impact(&self, id: &ItemId) -> Vec<Impact> {
        let mut out: Vec<Impact> = Vec::new();
        let mut seen: HashSet<Source> = self.indices(id).iter().map(|&i| Source::Item(i)).collect();
        let mut queue: VecDeque<(ItemId, Vec<Step>)> = VecDeque::from([(id.clone(), Vec::new())]);
        while let Some((target, path)) = queue.pop_front() {
            let mut next: Vec<Step> = Vec::new();
            for link in self.inbound(&target) {
                next.push(Step {
                    from: link.source,
                    relation: link.role.clone(),
                    to: target.clone(),
                });
            }
            if let Some(children) = self.children.get(&target) {
                for &c in children {
                    next.push(Step {
                        from: Source::Item(c),
                        relation: "parent".to_owned(),
                        to: target.clone(),
                    });
                }
            }
            next.sort_by(|a, b| (a.from, &a.relation).cmp(&(b.from, &b.relation)));
            for step in next {
                if !seen.insert(step.from) {
                    continue;
                }
                let mut steps = vec![step.clone()];
                steps.extend(path.iter().cloned());
                if let Source::Item(i) = step.from {
                    queue.push_back((self.items[i].id.clone(), steps.clone()));
                }
                out.push(Impact {
                    source: step.from,
                    path: steps,
                });
            }
        }
        out
    }

    /// The next free number for `prefix`: one above the highest used by
    /// any item, closed ones included (IDs are never reused).
    pub fn next_id(&self, prefix: &str) -> ItemId {
        let max = self
            .items
            .iter()
            .filter(|i| i.id.prefix() == prefix)
            .map(|i| i.id.number())
            .max()
            .unwrap_or(0);
        ItemId::new(prefix, max + 1)
    }
}
