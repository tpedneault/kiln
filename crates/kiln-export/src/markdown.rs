//! The Markdown back end (EXPORT.md, "`markdown`"): CommonMark with
//! GitHub tables, readable on GitLab and GitHub.
//!
//! Item anchors: GitLab and GitHub form heading anchors from the heading
//! text, each its own way, so Kiln computes them with the flavour's rules
//! and links with them; `commonmark` has no such rule and gets an explicit
//! `<a id>` before each heading. Links across pages need the other
//! page's anchors, so pages are rendered twice: once to learn every
//! heading, once to write.

use std::collections::HashMap;

use kiln_model::{Diagrams, Flavour, ItemStyle};
use kiln_render::{plain, Align, Block, Inline, Props, Target};

use crate::document::{Node, Section, SectionKind};
use crate::paths::{page_file, relative, RepoLinks};
use crate::pictures::Pictures;
use crate::split::{Page, Split};
use kiln_model::ExportTarget;

/// What rendering a file export needs.
pub struct FileContext<'a> {
    pub target: &'a ExportTarget,
    pub split: &'a Split,
    /// `link_to` file exports, by name.
    pub others: Vec<(String, &'a ExportTarget, &'a Split)>,
    pub repo: &'a RepoLinks,
    /// `[jira] url`.
    pub jira_url: Option<&'a str>,
    pub pictures: &'a dyn Pictures,
}

impl FileContext<'_> {
    /// The file of a page of this export (`export` `None`) or of a
    /// `link_to` export.
    pub fn file_of(&self, export: Option<&str>, key: &str, ext: &str) -> Option<String> {
        match export {
            None => Some(page_file(self.target, self.split, key, ext)),
            Some(name) => {
                let (_, target, split) = self.others.iter().find(|(n, _, _)| n == name)?;
                Some(page_file(target, split, key, target.format.ext()))
            }
        }
    }
}

/// GitHub's heading anchor: lowercase, punctuation removed, spaces to `-`.
pub fn github_slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|c| {
            if c == ' ' {
                Some('-')
            } else if c.is_alphanumeric() || c == '-' || c == '_' {
                Some(c)
            } else {
                None
            }
        })
        .collect()
}

/// GitLab's: as GitHub's, then runs of `-` collapsed.
pub fn gitlab_slug(text: &str) -> String {
    let mut out = String::new();
    for c in github_slug(text).chars() {
        if c == '-' && out.ends_with('-') {
            continue;
        }
        out.push(c);
    }
    out
}

/// Anchors handed out on one page, with GitLab's and GitHub's `-1`,
/// `-2` suffixes for repeats.
#[derive(Default)]
struct Slugger {
    used: HashMap<String, usize>,
}

impl Slugger {
    fn slug(&mut self, flavour: Flavour, text: &str) -> String {
        let base = match flavour {
            Flavour::Github => github_slug(text),
            _ => gitlab_slug(text),
        };
        let n = self.used.entry(base.clone()).or_insert(0);
        let slug = if *n == 0 {
            base.clone()
        } else {
            format!("{base}-{n}")
        };
        *n += 1;
        slug
    }
}

/// Per page: item ID or section heading → anchor.
type Anchors = HashMap<String, HashMap<String, String>>;

struct Writer<'a> {
    ctx: &'a FileContext<'a>,
    page: &'a Page,
    file: String,
    anchors: &'a Anchors,
    /// `link_to` exports' anchors, by export name.
    others: &'a HashMap<String, Anchors>,
    slugger: Slugger,
    /// Learned this pass: anchor key → slug.
    learned: HashMap<String, String>,
    /// The anchor of the heading being written, for row anchors.
    current: String,
    attachments: Vec<(String, Vec<u8>)>,
}

/// Every page's anchors in one Markdown export, learned by rendering it
/// once.
fn learn_anchors(ctx: &FileContext) -> Anchors {
    let mut anchors: Anchors = HashMap::new();
    let none = HashMap::new();
    for page in &ctx.split.pages {
        let mut w = Writer::new(ctx, page, &anchors, &none);
        let _ = w.page();
        anchors.insert(page.key.clone(), w.learned);
    }
    anchors
}

/// Every file of a Markdown target: (project-relative path, bytes).
// implements: REQ-002
pub fn render(ctx: &FileContext) -> Vec<(String, Vec<u8>)> {
    let anchors = learn_anchors(ctx);
    // A `link_to` Markdown export's anchors follow its own flavour.
    let mut others: HashMap<String, Anchors> = HashMap::new();
    for (name, target, split) in &ctx.others {
        if target.format == kiln_model::ExportFormat::Markdown {
            let other = FileContext {
                target,
                split,
                others: Vec::new(),
                repo: ctx.repo,
                jira_url: ctx.jira_url,
                pictures: ctx.pictures,
            };
            others.insert(name.clone(), learn_anchors(&other));
        }
    }
    let mut out = Vec::new();
    for page in &ctx.split.pages {
        let mut w = Writer::new(ctx, page, &anchors, &others);
        let text = w.page();
        let dir = w
            .file
            .rsplit_once('/')
            .map(|(d, _)| format!("{d}/"))
            .unwrap_or_default();
        out.push((w.file.clone(), text.into_bytes()));
        for (name, bytes) in w.attachments {
            out.push((format!("{dir}{name}"), bytes));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

impl<'a> Writer<'a> {
    fn new(
        ctx: &'a FileContext<'a>,
        page: &'a Page,
        anchors: &'a Anchors,
        others: &'a HashMap<String, Anchors>,
    ) -> Self {
        Writer {
            ctx,
            page,
            file: page_file(ctx.target, ctx.split, &page.key, "md"),
            anchors,
            others,
            slugger: Slugger::default(),
            learned: HashMap::new(),
            current: String::new(),
            attachments: Vec::new(),
        }
    }

    fn flavour(&self) -> Flavour {
        self.ctx.target.markdown.flavour
    }

    fn page(&mut self) -> String {
        let mut out = String::new();
        let title = self.page.title.clone();
        self.heading_anchor(None, &title);
        out.push_str(&format!("# {}\n", escape(&title)));
        let nodes = &self.page.nodes;
        let body = self.nodes(nodes);
        if !body.is_empty() {
            out.push('\n');
            out.push_str(&body);
        }
        out
    }

    /// Hands out the anchor for a heading; returns the explicit `<a id>`
    /// line `commonmark` needs.
    fn heading_anchor(&mut self, key: Option<&str>, text: &str) -> String {
        if self.flavour() == Flavour::Commonmark {
            let id = key.map(str::to_owned).unwrap_or_else(|| gitlab_slug(text));
            if let Some(k) = key {
                self.learned.insert(k.to_owned(), id.clone());
            }
            return format!("<a id=\"{id}\"></a>\n");
        }
        let slug = self.slugger.slug(self.flavour(), text);
        if let Some(k) = key {
            self.learned.insert(k.to_owned(), slug);
        }
        String::new()
    }

    fn level(&self, depth: u8) -> usize {
        (depth as i32 - self.page.base_depth as i32 + 2).clamp(1, 7) as usize
    }

    fn nodes(&mut self, nodes: &[Node]) -> String {
        let mut parts: Vec<String> = Vec::new();
        for node in nodes {
            let text = match node {
                Node::Block(b) => self.block(b),
                Node::Section(s) => self.section(s),
            };
            if !text.is_empty() {
                parts.push(text);
            }
        }
        parts.join("\n")
    }

    fn section(&mut self, s: &Section) -> String {
        let text = s.heading_text();
        let key = match &s.kind {
            SectionKind::Item(id) => id.to_string(),
            _ => format!("section:{text}"),
        };
        let explicit = self.heading_anchor(Some(&key), &text);
        let outer = std::mem::replace(
            &mut self.current,
            self.learned.get(&key).cloned().unwrap_or_default(),
        );
        let level = self.level(s.depth).min(6);
        let mut out = format!("{explicit}{} {}\n", "#".repeat(level), escape(&text));
        // Under the heading, not in it: GitLab and GitHub derive anchors
        // from heading text, and a baseline mustn't move them.
        // Italic, so it doesn't read as the bold status lozenge below it.
        if let Some(kiln_render::Inline::Status { text, .. }) = s.marker_inline() {
            out.push_str(&format!("\n*{text}*\n"));
        }
        let inner = self.nodes(&s.nodes);
        if !inner.is_empty() {
            out.push('\n');
            out.push_str(&inner);
        }
        self.current = outer;
        out
    }

    fn blocks(&mut self, blocks: &[Block]) -> String {
        let parts: Vec<String> = blocks
            .iter()
            .map(|b| self.block(b))
            .filter(|s| !s.is_empty())
            .collect();
        parts.join("\n")
    }

    fn block(&mut self, block: &Block) -> String {
        match block {
            Block::Heading { level, content } => {
                let level = self.level(*level);
                let text = self.inlines(content);
                if level > 6 {
                    format!("**{text}**\n")
                } else {
                    let explicit = self.heading_anchor(None, &plain(content));
                    format!("{explicit}{} {text}\n", "#".repeat(level))
                }
            }
            Block::Paragraph(content) => {
                let text = self.inlines(content);
                if text.trim().is_empty() {
                    String::new()
                } else {
                    format!("{}\n", guard_line_start(&text))
                }
            }
            Block::List { start, items } => {
                let mut out = String::new();
                for (n, item) in items.iter().enumerate() {
                    let marker = match start {
                        Some(s) => format!("{}. ", s + n as u64),
                        None => "- ".to_owned(),
                    };
                    let task = match item.task {
                        Some(true) => "[x] ",
                        Some(false) => "[ ] ",
                        None => "",
                    };
                    let body = self.blocks(&item.content);
                    out.push_str(&indent_after_first(
                        &format!("{marker}{task}{body}"),
                        marker.len(),
                    ));
                }
                out
            }
            Block::Quote(blocks) => quote(&self.blocks(blocks), None),
            Block::Callout { kind, content } => quote(&self.blocks(content), Some(kind.name())),
            Block::Code { lang, text } => fence(lang, text),
            Block::Diagram { source, .. } => self.diagram(source),
            Block::Table(t) => self.table(&t.align, &t.header, &t.rows),
            Block::Rule => "---\n".to_owned(),
            Block::Directive(_) => String::new(),
            Block::Toc => self.toc(),
            Block::Props(props) => self.props(props),
            Block::ChildPages => {
                let children: Vec<(String, String)> = self
                    .ctx
                    .split
                    .children(&self.page.key)
                    .iter()
                    .map(|p| {
                        (
                            p.title.clone(),
                            relative(
                                &self.file,
                                &page_file(self.ctx.target, self.ctx.split, &p.key, "md"),
                            ),
                        )
                    })
                    .collect();
                children
                    .iter()
                    .map(|(title, path)| {
                        format!("- [{}]({})\n", escape(title), crate::paths::encode(path))
                    })
                    .collect()
            }
        }
    }

    fn diagram(&mut self, source: &str) -> String {
        let code = fence("mermaid", source);
        if self.ctx.target.markdown.diagrams == Diagrams::Source {
            return code;
        }
        match self.ctx.pictures.diagram(source) {
            Ok(picture) => {
                let out = format!(
                    "![Diagram]({})\n\n<details><summary>Diagram source</summary>\n\n{code}\n</details>\n",
                    picture.name
                );
                self.attachments.push((picture.name, picture.png));
                out
            }
            Err(e) => format!(
                "> [!WARNING]\n> This diagram couldn't be drawn: {}\n\n{code}",
                escape(&e)
            ),
        }
    }

    fn table(
        &mut self,
        align: &[Align],
        header: &[Vec<Inline>],
        rows: &[Vec<Vec<Inline>>],
    ) -> String {
        let cols = header
            .len()
            .max(rows.iter().map(Vec::len).max().unwrap_or(0));
        if cols == 0 {
            return String::new();
        }
        let mut cell = |c: Option<&Vec<Inline>>| {
            c.map(|c| self.inlines(c).replace('\n', " "))
                .unwrap_or_default()
        };
        let mut out = String::new();
        let head: Vec<String> = (0..cols).map(|i| cell(header.get(i))).collect();
        out.push_str(&format!("| {} |\n", head.join(" | ")));
        let seps: Vec<&str> = (0..cols)
            .map(|i| match align.get(i).copied().unwrap_or(Align::None) {
                Align::None => "---",
                Align::Left => ":---",
                Align::Center => ":---:",
                Align::Right => "---:",
            })
            .collect();
        out.push_str(&format!("| {} |\n", seps.join(" | ")));
        for row in rows {
            let cells: Vec<String> = (0..cols).map(|i| cell(row.get(i))).collect();
            out.push_str(&format!("| {} |\n", cells.join(" | ")));
        }
        out
    }

    fn props(&mut self, props: &Props) -> String {
        let status = self.inline(&props.status);
        match props.style {
            ItemStyle::None => String::new(),
            // A backslash is CommonMark's hard line break, which GitLab
            // and GitHub keep; trailing spaces would be lost to editors.
            ItemStyle::List => {
                let mut lines = vec![status];
                for entry in &props.entries {
                    let values: Vec<String> =
                        entry.values.iter().map(|v| self.inlines(v)).collect();
                    lines.push(format!(
                        "**{}:** {}",
                        escape(&entry.label),
                        values.join(", ")
                    ));
                }
                format!("{}\n", lines.join("\\\n"))
            }
            ItemStyle::Line => {
                let mut parts = vec![status];
                for entry in &props.entries {
                    let values: Vec<String> =
                        entry.values.iter().map(|v| self.inlines(v)).collect();
                    parts.push(format!("{} {}", entry.lead, values.join(", ")));
                }
                format!("{}\n", parts.join(" · "))
            }
            ItemStyle::Table => {
                let mut out = "| Property | Value |\n| --- | --- |\n".to_owned();
                out.push_str(&format!("| Status | {status} |\n"));
                for entry in &props.entries {
                    let values: Vec<String> =
                        entry.values.iter().map(|v| self.inlines(v)).collect();
                    out.push_str(&format!(
                        "| {} | {} |\n",
                        escape(&entry.label),
                        values.join(", ")
                    ));
                }
                out
            }
        }
    }

    /// A list of this page's sections, three levels deep, linked.
    fn toc(&mut self) -> String {
        fn walk(nodes: &[Node], depth: usize, out: &mut Vec<(usize, String, String)>) {
            for n in nodes {
                if let Node::Section(s) = n {
                    let key = match &s.kind {
                        SectionKind::Item(id) => id.to_string(),
                        _ => format!("section:{}", s.heading_text()),
                    };
                    out.push((depth, s.heading_text(), key));
                    if depth < 2 {
                        walk(&s.nodes, depth + 1, out);
                    }
                }
            }
        }
        let mut entries = Vec::new();
        walk(&self.page.nodes, 0, &mut entries);
        let anchors = self.anchors.get(&self.page.key);
        entries
            .iter()
            .map(|(depth, text, key)| {
                let anchor = anchors
                    .and_then(|a| a.get(key))
                    .cloned()
                    .unwrap_or_default();
                format!("{}- [{}](#{anchor})\n", "  ".repeat(*depth), escape(text))
            })
            .collect()
    }

    fn inlines(&mut self, inlines: &[Inline]) -> String {
        inlines.iter().map(|i| self.inline(i)).collect()
    }

    fn inline(&mut self, inline: &Inline) -> String {
        match inline {
            Inline::Text(s) => escape(s),
            Inline::Code(s) => code_span(s),
            Inline::Emph(c) => format!("*{}*", self.inlines(c)),
            Inline::Strong(c) => format!("**{}**", self.inlines(c)),
            Inline::Strike(c) => format!("~~{}~~", self.inlines(c)),
            Inline::Link { target, content } => {
                let text = self.inlines(content);
                match self.url(target) {
                    Some(url) => format!("[{text}]({url})"),
                    None => text,
                }
            }
            Inline::Image { target, alt } => match self.image_url(target) {
                Some(url) => format!("![{}]({url})", escape(alt)),
                None => escape(alt),
            },
            Inline::Ref { id, text } => escape(text.as_deref().unwrap_or(&id.to_string())),
            Inline::Status { text, .. } => format!("**{}**", escape(text)),
            Inline::Jira(key) => match self.ctx.jira_url {
                Some(url) => format!(
                    "[{}]({}/browse/{key})",
                    escape(key),
                    url.trim_end_matches('/')
                ),
                None => escape(key),
            },
            Inline::Value(_) => String::new(),
            Inline::Break => "  \n".to_owned(),
            Inline::Anchor(a) => {
                if self.flavour() == Flavour::Commonmark {
                    self.learned.insert(a.clone(), a.clone());
                    format!("<a id=\"{a}\"></a>")
                } else {
                    // GitLab and GitHub keep no anchors of Kiln's making;
                    // the reference lands on the section holding the row.
                    let here = self.current.clone();
                    self.learned.insert(a.clone(), here);
                    String::new()
                }
            }
        }
    }

    fn image_url(&self, target: &Target) -> Option<String> {
        match target {
            Target::Repo { path, .. } => self.ctx.repo.image_url(&self.file, path),
            other => self.url(other),
        }
    }

    fn url(&self, target: &Target) -> Option<String> {
        match target {
            Target::Url(u) => Some(u.replace(' ', "%20").replace(')', "%29")),
            Target::Repo {
                path,
                line,
                fragment,
            } => {
                let mut url = self.ctx.repo.url(&self.file, path, *line)?;
                if let (Some(f), None) = (fragment, line) {
                    url.push_str(&format!("#{f}"));
                }
                Some(url)
            }
            Target::Anchor(a) => {
                let slug = self
                    .anchors
                    .get(&self.page.key)
                    .and_then(|m| m.get(a))
                    .cloned()
                    .unwrap_or_else(|| a.clone());
                Some(format!("#{slug}"))
            }
            Target::Page {
                export,
                key,
                anchor,
            } => {
                let file = self.ctx.file_of(export.as_deref(), key, "md")?;
                let mut url = if file == self.file {
                    String::new()
                } else {
                    crate::paths::encode(&relative(&self.file, &file))
                };
                if let Some(a) = anchor {
                    let anchors = match export {
                        None => Some(self.anchors),
                        Some(name) => self.others.get(name),
                    };
                    let slug = anchors
                        .and_then(|m| m.get(key))
                        .and_then(|m| m.get(a))
                        .cloned()
                        .unwrap_or_else(|| a.clone());
                    url.push_str(&format!("#{slug}"));
                }
                Some(url)
            }
        }
    }
}

/// Backslash-escapes what Markdown would otherwise interpret.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(
            c,
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '|' | '~' | '&' | '#'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A paragraph that would start a list, quote or heading gets its first
/// character escaped.
fn guard_line_start(text: &str) -> String {
    let starts_list = text.starts_with(['-', '+', '=']) || {
        let digits = text.chars().take_while(char::is_ascii_digit).count();
        digits > 0 && text[digits..].starts_with(['.', ')'])
    };
    if starts_list {
        let digits = text.chars().take_while(char::is_ascii_digit).count();
        if digits > 0 {
            format!("{}\\{}", &text[..digits], &text[digits..])
        } else {
            format!("\\{text}")
        }
    } else {
        text.to_owned()
    }
}

fn code_span(s: &str) -> String {
    let longest = longest_run(s, '`');
    let ticks = "`".repeat(longest + 1);
    let pad = if s.starts_with('`') || s.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{ticks}{pad}{s}{pad}{ticks}")
}

fn fence(lang: &str, text: &str) -> String {
    let ticks = "`".repeat(longest_run(text, '`').max(2) + 1);
    let mut body = text.to_owned();
    if !body.ends_with('\n') {
        body.push('\n');
    }
    format!("{ticks}{lang}\n{body}{ticks}\n")
}

fn longest_run(s: &str, c: char) -> usize {
    let mut best = 0;
    let mut run = 0;
    for ch in s.chars() {
        if ch == c {
            run += 1;
            best = best.max(run);
        } else {
            run = 0;
        }
    }
    best
}

fn quote(text: &str, callout: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(kind) = callout {
        out.push_str(&format!("> [!{kind}]\n"));
    }
    for line in text.trim_end().lines() {
        if line.is_empty() {
            out.push_str(">\n");
        } else {
            out.push_str(&format!("> {line}\n"));
        }
    }
    out
}

/// Indents every line after the first by `width`, for list item bodies.
fn indent_after_first(text: &str, width: usize) -> String {
    let pad = " ".repeat(width);
    let mut out = String::new();
    for (i, line) in text.trim_end().lines().enumerate() {
        if i > 0 && !line.is_empty() {
            out.push_str(&pad);
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_follow_each_flavour() {
        assert_eq!(
            github_slug("2.1.3 REQ-014 Flint appears within 50 ms"),
            "213-req-014-flint-appears-within-50-ms"
        );
        assert_eq!(github_slug("A — b"), "a--b");
        assert_eq!(gitlab_slug("A — b"), "a-b");
        assert_eq!(github_slug("Ünïcode & (stuff)"), "ünïcode--stuff");
        let mut s = Slugger::default();
        assert_eq!(
            (
                s.slug(Flavour::Github, "Intro"),
                s.slug(Flavour::Github, "Intro")
            ),
            ("intro".into(), "intro-1".into())
        );
    }

    #[test]
    fn escaping() {
        assert_eq!(escape("a*b_[c]<d>|e#&"), "a\\*b\\_\\[c\\]\\<d\\>\\|e\\#\\&");
        assert_eq!(guard_line_start("1. not a list"), "1\\. not a list");
        assert_eq!(guard_line_start("- not a list"), "\\- not a list");
        assert_eq!(code_span("a`b"), "``a`b``");
        assert_eq!(fence("rust", "```\n"), "````rust\n```\n````\n");
    }
}
