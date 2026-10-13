//! The assembled document: one tree of numbered sections holding blocks,
//! before it is split into pages (EXPORT.md, "The pipeline", step 2).

use kiln_model::ItemId;
use kiln_render::{Block, Inline};

/// One whole document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub title: String,
    /// Sections and blocks in order; blocks before the first section are
    /// the opening material (summary, table of contents, prose).
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Block(Block),
    Section(Section),
}

/// What a section is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionKind {
    /// A template heading, or a type's section without a template.
    Heading,
    /// An item placed with `style="full"`.
    Item(ItemId),
    /// A report appended as an appendix.
    Report(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// `2.1.3`, `A`, `A.1`; empty without numbering.
    pub number: String,
    /// The heading text without its number.
    pub title: Vec<Inline>,
    /// 1 for top-level sections.
    pub depth: u8,
    pub kind: SectionKind,
    /// `tree` may give this section a page of its own (items, unless the
    /// directive said `split="no"`).
    pub own_page: bool,
    /// Blocks and child sections in order. Headings inside blocks carry
    /// absolute depths (EXPORT.md: an item body's headings move under the
    /// item's level).
    pub nodes: Vec<Node>,
}

impl Section {
    /// `2.1.3 REQ-014 Title`, the heading as shown.
    pub fn heading_text(&self) -> String {
        let title = kiln_render::plain(&self.title);
        if self.number.is_empty() {
            title
        } else {
            format!("{} {title}", self.number)
        }
    }

    /// The item's ID, as its anchor.
    pub fn anchor(&self) -> Option<String> {
        match &self.kind {
            SectionKind::Item(id) => Some(id.to_string()),
            _ => None,
        }
    }

    /// Child sections, in order.
    pub fn sections(&self) -> impl Iterator<Item = &Section> {
        self.nodes.iter().filter_map(|n| match n {
            Node::Section(s) => Some(s),
            Node::Block(_) => None,
        })
    }
}

impl Document {
    /// Every section, depth first.
    pub fn all_sections(&self) -> Vec<&Section> {
        fn walk<'a>(nodes: &'a [Node], out: &mut Vec<&'a Section>) {
            for n in nodes {
                if let Node::Section(s) = n {
                    out.push(s);
                    walk(&s.nodes, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.nodes, &mut out);
        out
    }
}
