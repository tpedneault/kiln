//! Export targets (`[[export]]`) and the Confluence connection
//! (`[confluence]`), as values (docs/EXPORT.md, "Targets";
//! docs/CONFLUENCE.md, "Configuration").

/// What an export is written as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Confluence,
    Markdown,
    Html,
    /// Word (K8).
    Docx,
    /// Confluence wiki markup in a file, to paste into a page by hand.
    ConfluenceWiki,
}

impl ExportFormat {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "confluence" => Some(ExportFormat::Confluence),
            "markdown" => Some(ExportFormat::Markdown),
            "html" => Some(ExportFormat::Html),
            "docx" => Some(ExportFormat::Docx),
            "confluence-wiki" => Some(ExportFormat::ConfluenceWiki),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ExportFormat::Confluence => "confluence",
            ExportFormat::Markdown => "markdown",
            ExportFormat::Html => "html",
            ExportFormat::Docx => "docx",
            ExportFormat::ConfluenceWiki => "confluence-wiki",
        }
    }

    /// A file format's extension; empty for Confluence.
    pub fn ext(self) -> &'static str {
        match self {
            ExportFormat::Confluence => "",
            ExportFormat::Markdown => "md",
            ExportFormat::Html => "html",
            ExportFormat::Docx => "docx",
            ExportFormat::ConfluenceWiki => "txt",
        }
    }

    /// Whether the target writes files (and so needs `path`).
    pub fn is_file(self) -> bool {
        !matches!(self, ExportFormat::Confluence)
    }

    /// Whether what it writes is read in Confluence, not beside the
    /// repository: its links to the repository are web links naming a
    /// commit, wherever the file is written.
    pub fn read_in_confluence(self) -> bool {
        matches!(
            self,
            ExportFormat::Confluence | ExportFormat::ConfluenceWiki
        )
    }
}

/// How the one document is split into pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// One page (or file) holding the whole document.
    Document,
    /// A root page and one page per top-level section.
    Sections,
    /// As `Sections`, and a page per item.
    Tree,
}

impl Layout {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "document" => Some(Layout::Document),
            "sections" => Some(Layout::Sections),
            "tree" => Some(Layout::Tree),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Layout::Document => "document",
            Layout::Sections => "sections",
            Layout::Tree => "tree",
        }
    }
}

/// How an item's properties are shown under its heading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemStyle {
    /// One line: status, parent, links, Jira.
    Line,
    /// The full properties table.
    Table,
    /// Heading and body only.
    None,
}

impl ItemStyle {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "line" => Some(ItemStyle::Line),
            "table" => Some(ItemStyle::Table),
            "none" => Some(ItemStyle::None),
            _ => None,
        }
    }
}

/// How Markdown heading anchors are formed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavour {
    Gitlab,
    Github,
    Commonmark,
}

impl Flavour {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "gitlab" => Some(Flavour::Gitlab),
            "github" => Some(Flavour::Github),
            "commonmark" => Some(Flavour::Commonmark),
            _ => None,
        }
    }
}

/// Whether Mermaid blocks become pictures or stay source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Diagrams {
    Image,
    Source,
}

impl Diagrams {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "image" => Some(Diagrams::Image),
            "source" => Some(Diagrams::Source),
            _ => None,
        }
    }
}

/// The reports a document can hold (EXPORT.md, "Reports"). `changes`
/// lists what changed when the export has `--since`, and says how to get
/// that when it hasn't.
pub const REPORTS: &[&str] = &["traceability", "findings", "roadmap", "risks", "changes"];

/// One `[[export]]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportTarget {
    pub name: String,
    pub format: ExportFormat,
    pub layout: Layout,
    /// Defaults to the project's name.
    pub title: String,
    /// A Markdown file in the project that arranges the document.
    pub template: Option<String>,
    /// Without a template: which types get a section, in order.
    pub types: Vec<String>,
    /// Statuses left out; `None` means each type's `closed` statuses.
    pub exclude_status: Option<Vec<String>>,
    /// Without a template: reports appended as appendices.
    pub reports: Vec<String>,
    pub numbering: bool,
    /// `None` means the layout's default.
    pub item_style: Option<ItemStyle>,
    /// Other targets to link into for items this one doesn't hold.
    pub link_to: Vec<String>,
    /// File formats: the output file (`document`) or folder.
    pub path: Option<String>,
    pub markdown: MarkdownOptions,
    pub html: HtmlOptions,
    pub docx: DocxOptions,
    pub confluence: ConfluenceTarget,
}

impl ExportTarget {
    pub fn item_style(&self) -> ItemStyle {
        self.item_style.unwrap_or(match self.layout {
            Layout::Tree => ItemStyle::Table,
            _ => ItemStyle::Line,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownOptions {
    pub flavour: Flavour,
    pub diagrams: Diagrams,
}

impl Default for MarkdownOptions {
    fn default() -> Self {
        MarkdownOptions {
            flavour: Flavour::Gitlab,
            diagrams: Diagrams::Source,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HtmlOptions {
    /// Only the built-in light theme so far.
    pub theme: Option<String>,
}

/// `[export.docx]`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocxOptions {
    /// A project-relative `.docx` whose styles (and page setup) the
    /// export takes, as Word's own "reference document" idea.
    pub reference: Option<String>,
}

/// `[export.confluence]`: one target's Confluence settings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConfluenceTarget {
    /// Create the (root) page under this page…
    pub parent: Option<u64>,
    /// …or write into this existing page (document layout only).
    pub page: Option<u64>,
    /// Another space than `[confluence] space`.
    pub space: Option<String>,
    pub labels: Vec<String>,
    /// The table of contents' depth (default 3).
    pub toc_levels: Option<u8>,
    /// Restrict edits on an adopted page too.
    pub restrict_adopted: bool,
}

/// Which Confluence API Kiln talks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfluenceFlavour {
    /// `*.atlassian.net` is Cloud, anything else Data Center.
    Auto,
    Cloud,
    DataCenter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnDrift {
    Stop,
    Skip,
    Overwrite,
}

/// `[confluence]`: the connection shared by every Confluence target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfluenceConfig {
    pub url: String,
    pub flavour: ConfluenceFlavour,
    pub space: String,
    /// Cloud: the account e-mail the token belongs to.
    pub user: String,
    /// `auto`, `bearer` or `basic`.
    pub auth: String,
    pub labels: Vec<String>,
    pub jira_macro: bool,
    pub jira_server_id: String,
    pub restrict_edits: bool,
    pub on_drift: OnDrift,
    pub concurrency: u32,
    pub diagrams: Diagrams,
    pub max_page_kib: u64,
}

impl ConfluenceConfig {
    /// The flavour, with `auto` decided by the URL.
    pub fn resolved_flavour(&self) -> ConfluenceFlavour {
        match self.flavour {
            ConfluenceFlavour::Auto => {
                let host = self
                    .url
                    .split("://")
                    .nth(1)
                    .unwrap_or(&self.url)
                    .split(['/', ':'])
                    .next()
                    .unwrap_or("");
                if host.ends_with(".atlassian.net") {
                    ConfluenceFlavour::Cloud
                } else {
                    ConfluenceFlavour::DataCenter
                }
            }
            f => f,
        }
    }

    /// The URL as configured, without a trailing slash: the base of every
    /// call, and the credential store account's name.
    pub fn base_url(&self) -> &str {
        self.url.trim_end_matches('/')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn confluence(url: &str) -> ConfluenceConfig {
        ConfluenceConfig {
            url: url.into(),
            flavour: ConfluenceFlavour::Auto,
            space: "S".into(),
            user: String::new(),
            auth: "auto".into(),
            labels: vec![],
            jira_macro: true,
            jira_server_id: String::new(),
            restrict_edits: false,
            on_drift: OnDrift::Stop,
            concurrency: 4,
            diagrams: Diagrams::Image,
            max_page_kib: 1024,
        }
    }

    #[test]
    fn auto_flavour_follows_the_host() {
        assert_eq!(
            confluence("https://x.atlassian.net/wiki/").resolved_flavour(),
            ConfluenceFlavour::Cloud
        );
        assert_eq!(
            confluence("https://confluence.example.com").resolved_flavour(),
            ConfluenceFlavour::DataCenter
        );
        assert_eq!(
            confluence("https://x.atlassian.net.evil.com").resolved_flavour(),
            ConfluenceFlavour::DataCenter
        );
        assert_eq!(
            confluence("https://c.example.com/conf/").base_url(),
            "https://c.example.com/conf"
        );
    }
}
