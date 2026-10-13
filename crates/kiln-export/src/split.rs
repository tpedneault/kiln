//! Layouts (EXPORT.md, "Layouts"): the one document kept whole
//! (`document`), split at each top-level section (`sections`), or also at
//! each item (`tree`). Page keys are stable across exports
//! (CONFLUENCE.md, "Pages and their identity").

use std::collections::{HashMap, HashSet};

use kiln_model::{ItemId, Layout};
use kiln_render::Block;

use crate::document::{Document, Node, Section, SectionKind};

/// One page of an export: a Confluence page or a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// `doc`, `section:<slug>`, `report:<name>`, `item:<ID>`.
    pub key: String,
    pub title: String,
    /// The parent page's key; `None` for the root.
    pub parent: Option<String>,
    /// The section depth shown as the page's top heading level.
    pub base_depth: u8,
    pub nodes: Vec<Node>,
}

impl Page {
    /// Every item on this page: under its own heading, or as a row with an
    /// anchor.
    pub fn items(&self) -> Vec<ItemId> {
        fn walk(nodes: &[Node], out: &mut Vec<ItemId>) {
            for n in nodes {
                match n {
                    Node::Section(s) => {
                        if let SectionKind::Item(id) = &s.kind {
                            out.push(id.clone());
                        }
                        walk(&s.nodes, out);
                    }
                    Node::Block(Block::Table(t)) => {
                        for cell in t.rows.iter().filter_map(|r| r.first()) {
                            for inline in cell {
                                if let kiln_render::Inline::Anchor(a) = inline {
                                    out.extend(a.parse::<ItemId>().ok());
                                }
                            }
                        }
                    }
                    Node::Block(_) => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.nodes, &mut out);
        out
    }
}

/// Where an item ended up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub page: String,
    /// The item has the page to itself (`tree`), so links need no anchor.
    pub own_page: bool,
}

/// The pages of one export and where each item is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    pub pages: Vec<Page>,
    pub locations: HashMap<ItemId, Location>,
}

impl Split {
    pub fn page(&self, key: &str) -> Option<&Page> {
        self.pages.iter().find(|p| p.key == key)
    }

    pub fn children(&self, key: &str) -> Vec<&Page> {
        self.pages
            .iter()
            .filter(|p| p.parent.as_deref() == Some(key))
            .collect()
    }
}

/// Splits `doc` by `layout`.
pub fn split(doc: Document, layout: Layout) -> Split {
    let mut pages = Vec::new();
    if layout == Layout::Document {
        pages.push(Page {
            key: "doc".into(),
            title: doc.title,
            parent: None,
            base_depth: 1,
            nodes: doc.nodes,
        });
    } else {
        let mut root_nodes = Vec::new();
        let mut sections = Vec::new();
        for node in doc.nodes {
            match node {
                Node::Section(s) => sections.push(s),
                block => root_nodes.push(block),
            }
        }
        if !sections.is_empty() {
            root_nodes.push(Node::Block(Block::ChildPages));
        }
        pages.push(Page {
            key: "doc".into(),
            title: doc.title,
            parent: None,
            base_depth: 1,
            nodes: root_nodes,
        });
        let mut used = HashSet::new();
        for section in sections {
            let key = unique(section_key(&section), &mut used);
            let title = page_title(&section);
            let base_depth = section.depth + 1;
            let mut nodes = section.nodes;
            if layout == Layout::Tree {
                extract(&mut nodes, &key, &mut pages, &mut used);
            }
            pages.push(Page {
                key,
                title,
                parent: Some("doc".into()),
                base_depth,
                nodes,
            });
        }
        // Parents first, as created; children were pushed before their
        // parent page by `extract`, so order by depth in the tree.
        order_parents_first(&mut pages);
    }
    let mut locations = HashMap::new();
    for page in &pages {
        for id in page.items() {
            locations.entry(id).or_insert(Location {
                page: page.key.clone(),
                own_page: false,
            });
        }
        if let Some(id) = page
            .key
            .strip_prefix("item:")
            .and_then(|s| s.parse::<ItemId>().ok())
        {
            locations.insert(
                id,
                Location {
                    page: page.key.clone(),
                    own_page: true,
                },
            );
        }
    }
    Split { pages, locations }
}

fn section_key(s: &Section) -> String {
    match &s.kind {
        SectionKind::Item(id) => format!("item:{id}"),
        SectionKind::Report(name) => format!("report:{name}"),
        SectionKind::Heading => format!(
            "section:{}",
            kiln_model::slug(&kiln_render::plain(&s.title), 60)
        ),
    }
}

fn page_title(s: &Section) -> String {
    match &s.kind {
        SectionKind::Item(_) => kiln_render::plain(&s.title),
        _ => s.heading_text(),
    }
}

fn unique(key: String, used: &mut HashSet<String>) -> String {
    if used.insert(key.clone()) {
        return key;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{key}-{n}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

/// `tree`: moves item sections that may have pages of their own out of
/// `nodes` into pages under `parent`, leaving a list of child pages.
fn extract(nodes: &mut Vec<Node>, parent: &str, pages: &mut Vec<Page>, used: &mut HashSet<String>) {
    let mut kept = Vec::new();
    let mut moved = false;
    for node in std::mem::take(nodes) {
        match node {
            Node::Section(s) if s.own_page && matches!(s.kind, SectionKind::Item(_)) => {
                let key = unique(section_key(&s), used);
                let title = page_title(&s);
                let base_depth = s.depth + 1;
                let mut inner = s.nodes;
                extract(&mut inner, &key, pages, used);
                pages.push(Page {
                    key,
                    title,
                    parent: Some(parent.to_owned()),
                    base_depth,
                    nodes: inner,
                });
                moved = true;
            }
            Node::Section(mut s) => {
                extract(&mut s.nodes, parent, pages, used);
                kept.push(Node::Section(s));
            }
            block => kept.push(block),
        }
    }
    if moved {
        kept.push(Node::Block(Block::ChildPages));
    }
    *nodes = kept;
}

/// Reorders so each page comes after its parent, keeping document order
/// among siblings.
fn order_parents_first(pages: &mut Vec<Page>) {
    let all = std::mem::take(pages);
    let mut by_parent: HashMap<Option<String>, Vec<Page>> = HashMap::new();
    let mut order: Vec<Option<String>> = Vec::new();
    for page in all {
        if !order.contains(&page.parent) {
            order.push(page.parent.clone());
        }
        by_parent.entry(page.parent.clone()).or_default().push(page);
    }
    fn emit(
        parent: Option<String>,
        by_parent: &mut HashMap<Option<String>, Vec<Page>>,
        out: &mut Vec<Page>,
    ) {
        let mut children = by_parent.remove(&parent).unwrap_or_default();
        // Pages under a parent were pushed in document order, except that
        // `extract` pushes a child before its section; keep insertion
        // order, which is document order among siblings.
        for child in children.drain(..) {
            let key = child.key.clone();
            out.push(child);
            emit(Some(key), by_parent, out);
        }
    }
    emit(None, &mut by_parent, pages);
}
