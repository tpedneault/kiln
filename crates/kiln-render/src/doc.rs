//! The intermediate document tree every back end renders: blocks and
//! inlines, with item references, callouts, Mermaid diagrams, status
//! lozenges and item properties as nodes of their own, so each back end
//! renders what it understands instead of re-parsing Markdown.

use kiln_model::{ItemId, ItemStyle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// A heading inside a body or report (not a document section; those
    /// are `kiln-export`'s). `level` is 1–6.
    Heading {
        level: u8,
        content: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    List {
        start: Option<u64>,
        items: Vec<ListItem>,
    },
    Quote(Vec<Block>),
    Callout {
        kind: Callout,
        content: Vec<Block>,
    },
    /// Fenced or indented code; `lang` may be empty.
    Code {
        lang: String,
        text: String,
    },
    /// A ` ```mermaid ` block: its source and the file line it starts on.
    Diagram {
        source: String,
        line: u32,
    },
    Table(Table),
    Rule,
    /// A `<!-- kiln:… -->` block directive (templates only).
    Directive(Directive),
    /// Where the table of contents goes.
    Toc,
    /// An item's properties, under its heading.
    Props(Props),
    /// The list of child pages, on a `sections` or `tree` root page.
    ChildPages,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    /// `Some(done)` for a GitHub task list item.
    pub task: Option<bool>,
    pub content: Vec<Block>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callout {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl Callout {
    pub fn name(self) -> &'static str {
        match self {
            Callout::Note => "NOTE",
            Callout::Tip => "TIP",
            Callout::Important => "IMPORTANT",
            Callout::Warning => "WARNING",
            Callout::Caution => "CAUTION",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub align: Vec<Align>,
    pub header: Vec<Vec<Inline>>,
    pub rows: Vec<Vec<Vec<Inline>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Code(String),
    Emph(Vec<Inline>),
    Strong(Vec<Inline>),
    Strike(Vec<Inline>),
    Link {
        target: Target,
        content: Vec<Inline>,
    },
    Image {
        target: Target,
        alt: String,
    },
    /// An item reference not yet resolved to a link: `[[ID]]`,
    /// `[[ID|text]]` or a bare ID.
    Ref {
        id: ItemId,
        text: Option<String>,
    },
    /// A status lozenge.
    Status {
        text: String,
        tone: Tone,
    },
    /// A Jira issue key.
    Jira(String),
    /// A template's `<!-- kiln:value name -->`, until assembly fills it.
    Value(String),
    /// A hard line break.
    Break,
    /// An anchor named by an item's ID where the item has no heading (a
    /// row of a `style="row"` table), so references can still land there.
    Anchor(String),
}

impl Inline {
    pub fn text(s: impl Into<String>) -> Inline {
        Inline::Text(s.into())
    }
}

/// Where a link goes. References resolve to `Anchor` (same page), `Page`
/// (another page of this export or a `link_to` export) or `Repo` (a file
/// in the repository, which each back end turns into a URL or path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Url(String),
    /// A file in the repository, relative to the project root.
    Repo {
        path: String,
        line: Option<u32>,
        fragment: Option<String>,
    },
    /// An anchor on the same page (an item's ID).
    Anchor(String),
    /// A page of an export: `export` is `None` for this one.
    Page {
        export: Option<String>,
        key: String,
        anchor: Option<String>,
    },
}

/// A lozenge's colour group (CONFLUENCE.md, "Storage format": initial
/// grey, done green, closed grey, others blue; gaps red).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Grey,
    Blue,
    Green,
    Yellow,
    Red,
}

impl Tone {
    pub fn name(self) -> &'static str {
        match self {
            Tone::Grey => "Grey",
            Tone::Blue => "Blue",
            Tone::Green => "Green",
            Tone::Yellow => "Yellow",
            Tone::Red => "Red",
        }
    }
}

/// An item's properties (EXPORT.md, "Items in a document").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Props {
    pub style: ItemStyle,
    pub status: Inline,
    pub entries: Vec<PropEntry>,
}

/// One property: `label` heads its row in the table style; `lead` starts
/// it in the line style (`under`, `verified by`, `Jira`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropEntry {
    pub label: String,
    pub lead: String,
    pub values: Vec<Vec<Inline>>,
}

/// A `<!-- kiln:name positional… key="value"… -->` directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directive {
    pub name: String,
    pub positional: Vec<String>,
    pub attrs: Vec<(String, String)>,
    /// The template line it is on.
    pub line: u32,
}

impl Directive {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// The plain text of inlines, for titles, slugs and alt text.
pub fn plain(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(s) | Inline::Code(s) | Inline::Value(s) | Inline::Jira(s) => {
                out.push_str(s)
            }
            Inline::Emph(c)
            | Inline::Strong(c)
            | Inline::Strike(c)
            | Inline::Link { content: c, .. } => out.push_str(&plain(c)),
            Inline::Image { alt, .. } => out.push_str(alt),
            Inline::Ref { id, text } => out.push_str(text.as_deref().unwrap_or(&id.to_string())),
            Inline::Status { text, .. } => out.push_str(text),
            Inline::Break => out.push(' '),
            Inline::Anchor(_) => {}
        }
    }
    out
}
