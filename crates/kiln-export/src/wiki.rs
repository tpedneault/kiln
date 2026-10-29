//! The Confluence wiki markup back end (EXPORT.md, "`confluence-wiki`"):
//! one text file per page, for pasting into Confluence by hand (Insert →
//! Markup → Confluence wiki) where Kiln can't publish: no token, no API
//! access, or a page someone else owns.
//!
//! It is the `confluence` format's look without its planner: the same
//! macros (`anchor`, `status`, `toc`, `info`/`tip`/`note`/`warning`,
//! `code`, `expand`, `children`) and the same heading levels (the page
//! title is Confluence's, so sections start at `h1.`). What it gives up:
//! drift detection, idempotence and attachments, since a file can't upload
//! anything. Diagrams and repository images are written beside the file
//! for the reader to attach to the page; the markup already names them.
//!
//! Headings carry no section numbers (`2.1`), whatever `numbering`
//! says: a page pasted by hand gets edited around, and fixed numbers in
//! its headings would go stale; the item ID in the heading and its anchor
//! are what links and readers use.
//!
//! Wiki markup has no reliable escaping of everything, so text escapes
//! the characters that start markup with `\`, and emphasis uses the
//! braced forms (`{*}bold{*}`), which work mid-word too. Jira keys are
//! links, not the macro: a file can't know whether the Confluence it is
//! pasted into has Jira linked, and a broken macro is worse than a link.

use kiln_index::FileSource;
use kiln_model::ItemStyle;
use kiln_render::{plain, Block, Callout, Inline, Props, Target};
use sha2::{Digest, Sha256};

use crate::document::{Node, Section, SectionKind};
use crate::markdown::FileContext;
use crate::paths::page_file;

/// Every file of a Confluence wiki target: (project-relative path, bytes).
// implements: REQ-002
pub fn render(ctx: &FileContext, files: &dyn FileSource) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for page in &ctx.split.pages {
        let file = page_file(ctx.target, ctx.split, &page.key, "txt");
        let mut w = Writer {
            ctx,
            files,
            file: file.clone(),
            base: page.base_depth,
            attachments: Vec::new(),
        };
        let mut text = w.nodes(&page.nodes);
        // One newline at the end, however the last block ended.
        while text.ends_with("\n\n") {
            text.pop();
        }
        let dir = file
            .rsplit_once('/')
            .map(|(d, _)| format!("{d}/"))
            .unwrap_or_default();
        out.push((file, text.into_bytes()));
        for (name, bytes) in w.attachments {
            out.push((format!("{dir}{name}"), bytes));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

struct Writer<'a> {
    ctx: &'a FileContext<'a>,
    files: &'a dyn FileSource,
    file: String,
    base: u8,
    /// Pictures and images to attach by hand, written beside the file.
    attachments: Vec<(String, Vec<u8>)>,
}

impl Writer<'_> {
    fn level(&self, depth: u8) -> i32 {
        depth as i32 - self.base as i32 + 1
    }

    fn nodes(&mut self, nodes: &[Node]) -> String {
        nodes
            .iter()
            .map(|n| match n {
                Node::Block(b) => self.block(b),
                Node::Section(s) => self.section(s),
            })
            .collect()
    }

    fn section(&mut self, s: &Section) -> String {
        let level = self.level(s.depth).clamp(1, 6);
        let anchor = match &s.kind {
            SectionKind::Item(id) => format!("{{anchor:{id}}}"),
            _ => String::new(),
        };
        let marker = s
            .marker_inline()
            .map(|m| format!(" {}", self.inlines(&[m])))
            .unwrap_or_default();
        format!(
            "h{level}. {anchor}{}{marker}\n\n{}",
            esc(&plain(&s.title)),
            self.nodes(&s.nodes)
        )
    }

    fn blocks(&mut self, blocks: &[Block]) -> String {
        blocks.iter().map(|b| self.block(b)).collect()
    }

    fn block(&mut self, block: &Block) -> String {
        match block {
            Block::Heading { level, content } => {
                let level = self.level(*level);
                let text = self.inlines(content);
                if level > 6 {
                    format!("{{*}}{text}{{*}}\n\n")
                } else {
                    format!("h{}. {text}\n\n", level.max(1))
                }
            }
            Block::Paragraph(c) => format!("{}\n\n", self.inlines(c)),
            Block::List { start, items } => {
                let mut out = String::new();
                self.list(&mut out, start.is_some(), items, "");
                out.push('\n');
                out
            }
            Block::Quote(b) => format!("{{quote}}\n{}{{quote}}\n\n", self.inner(b)),
            Block::Callout { kind, content } => {
                // As the `confluence` format maps them.
                let name = match kind {
                    Callout::Note | Callout::Important => "info",
                    Callout::Tip => "tip",
                    Callout::Warning => "note",
                    Callout::Caution => "warning",
                };
                format!("{{{name}}}\n{}{{{name}}}\n\n", self.inner(content))
            }
            Block::Code { lang, text } => code(lang, text),
            Block::Diagram { source, .. } => self.diagram(source),
            Block::Table(t) => {
                let mut out = String::new();
                if !t.header.is_empty() {
                    out.push_str("||");
                    for h in &t.header {
                        out.push_str(&format!("{}||", cell(&self.inlines(h))));
                    }
                    out.push('\n');
                }
                for row in &t.rows {
                    out.push('|');
                    for c in row {
                        out.push_str(&format!("{}|", cell(&self.inlines(c))));
                    }
                    out.push('\n');
                }
                out.push('\n');
                out
            }
            Block::Rule => "----\n\n".to_owned(),
            Block::Directive(_) => String::new(),
            Block::Toc => format!(
                "{{toc:maxLevel={}}}\n\n",
                self.ctx.target.confluence.toc_levels.unwrap_or_else(|| {
                    self.ctx
                        .split
                        .pages
                        .iter()
                        .map(|p| crate::document::toc_levels(&p.nodes))
                        .max()
                        .unwrap_or(3)
                })
            ),
            Block::Props(p) => self.props(p),
            Block::ChildPages => "{children:all=true}\n\n".to_owned(),
        }
    }

    /// Blocks inside a macro body, without the trailing blank line.
    fn inner(&mut self, blocks: &[Block]) -> String {
        let mut text = self.blocks(blocks);
        while text.ends_with("\n\n") {
            text.pop();
        }
        text
    }

    /// A list, nested by repeating the marker (`**`, `#*`). An item's
    /// further paragraphs follow on the same item after a line break,
    /// since wiki markup lists hold one line each.
    fn list(
        &mut self,
        out: &mut String,
        ordered: bool,
        items: &[kiln_render::ListItem],
        prefix: &str,
    ) {
        let marker = format!("{prefix}{}", if ordered { '#' } else { '*' });
        for item in items {
            let task = match item.task {
                Some(true) => "☑ ",
                Some(false) => "☐ ",
                None => "",
            };
            let mut line = Vec::new();
            let mut nested = Vec::new();
            for b in &item.content {
                match b {
                    Block::List { start, items } => nested.push((start.is_some(), items)),
                    Block::Paragraph(c) => line.push(self.inlines(c)),
                    other => line.push(self.inner(std::slice::from_ref(other)).replace('\n', " ")),
                }
            }
            out.push_str(&format!("{marker} {task}{}\n", line.join(" \\\\ ")));
            for (ordered, items) in nested {
                self.list(out, ordered, items, &marker);
            }
        }
    }

    fn diagram(&mut self, source: &str) -> String {
        let code = code("mermaid", source);
        match self.ctx.pictures.diagram(source) {
            Ok(picture) => {
                let out = format!(
                    "!{}!\n\n{{expand:title=Diagram source}}\n{}\n{{expand}}\n\n",
                    picture.name,
                    code.trim_end()
                );
                self.attachments.push((picture.name, picture.png));
                out
            }
            Err(e) => format!(
                "{{warning}}\nThis diagram couldn't be drawn: {}\n{{warning}}\n\n{code}",
                esc(&e)
            ),
        }
    }

    fn props(&mut self, props: &Props) -> String {
        let status = self.inline(&props.status);
        match props.style {
            ItemStyle::None => String::new(),
            // `\\` is wiki markup's line break.
            ItemStyle::List => {
                let mut lines = vec![status];
                for e in &props.entries {
                    let values: Vec<String> = e.values.iter().map(|v| self.inlines(v)).collect();
                    lines.push(format!("*{}:* {}", esc(&e.label), values.join(", ")));
                }
                format!("{}\n\n", lines.join(" \\\\\n"))
            }
            ItemStyle::Line => {
                let mut parts = vec![status];
                for e in &props.entries {
                    let values: Vec<String> = e.values.iter().map(|v| self.inlines(v)).collect();
                    parts.push(format!("{} {}", esc(&e.lead), values.join(", ")));
                }
                format!("{}\n\n", parts.join(" · "))
            }
            ItemStyle::Table => {
                let mut out = format!("||Status|{}|\n", cell(&status));
                for e in &props.entries {
                    let values: Vec<String> = e.values.iter().map(|v| self.inlines(v)).collect();
                    out.push_str(&format!(
                        "||{}|{}|\n",
                        cell(&esc(&e.label)),
                        cell(&values.join(", "))
                    ));
                }
                out.push('\n');
                out
            }
        }
    }

    fn inlines(&mut self, inlines: &[Inline]) -> String {
        inlines.iter().map(|i| self.inline(i)).collect()
    }

    fn inline(&mut self, inline: &Inline) -> String {
        match inline {
            Inline::Text(s) => esc(&s.replace('\n', " ")),
            Inline::Code(s) if s.trim().is_empty() => String::new(),
            Inline::Code(s) => format!("{{{{{}}}}}", esc(s)),
            Inline::Emph(c) => format!("{{_}}{}{{_}}", self.inlines(c)),
            Inline::Strong(c) => format!("{{*}}{}{{*}}", self.inlines(c)),
            Inline::Strike(c) => format!("{{-}}{}{{-}}", self.inlines(c)),
            Inline::Link { target, content } => {
                let text = esc(&plain(content));
                match self.href(target) {
                    Some(href) => format!("[{text}|{href}]"),
                    None => self.inlines(content),
                }
            }
            Inline::Image { target, alt } => match self.image(target) {
                Some(name) => format!("!{name}|alt={}!", attr(alt)),
                None => esc(alt),
            },
            Inline::Ref { id, text } => esc(text.as_deref().unwrap_or(&id.to_string())),
            Inline::Status { text, tone } => {
                format!("{{status:colour={}|title={}}}", tone.name(), attr(text))
            }
            Inline::Jira(key) => match self.ctx.jira_url {
                Some(url) => format!(
                    "[{}|{}/browse/{}]",
                    esc(key),
                    target(url.trim_end_matches('/')),
                    target(key)
                ),
                None => esc(key),
            },
            Inline::Value(_) => String::new(),
            Inline::Break => " \\\\ ".to_owned(),
            Inline::Anchor(a) => format!("{{anchor:{}}}", attr(a)),
        }
    }

    /// A link's target: a URL, `#anchor` on this page, or `Page
    /// title#anchor` for another page, which the reader creates under
    /// that title when importing a `sections` or `tree` layout.
    fn href(&self, t: &Target) -> Option<String> {
        match t {
            Target::Url(u) => Some(target(u)),
            Target::Repo { path, line, .. } => {
                Some(target(&self.ctx.repo.url(&self.file, path, *line)?))
            }
            Target::Anchor(a) => Some(format!("#{}", target(a))),
            Target::Page {
                export,
                key,
                anchor,
            } => {
                let split = match export {
                    None => self.ctx.split,
                    Some(name) => {
                        self.ctx
                            .others
                            .iter()
                            .find(|(n, t, _)| n == name && t.format == self.ctx.target.format)?
                            .2
                    }
                };
                let here = page_file(self.ctx.target, self.ctx.split, key, "txt") == self.file
                    && export.is_none();
                let mut out = if here {
                    String::new()
                } else {
                    target(&split.page(key)?.title)
                };
                if let Some(a) = anchor {
                    out.push_str(&format!("#{}", target(a)));
                }
                Some(out)
            }
        }
    }

    /// A repository image, written beside the file under a name from its
    /// content (as the `confluence` format attaches it), for the reader
    /// to attach. A remote image is shown from its URL.
    fn image(&mut self, t: &Target) -> Option<String> {
        match t {
            Target::Url(u) => Some(target(u)),
            Target::Repo { path, .. } => {
                let bytes = self.files.read(path).ok()?;
                let ext = path.rsplit('.').next()?.to_ascii_lowercase();
                if !matches!(
                    ext.as_str(),
                    "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp"
                ) {
                    return None;
                }
                let digest = Sha256::digest(&bytes);
                let hex: String = digest.iter().take(6).map(|b| format!("{b:02x}")).collect();
                let name = format!("image-{hex}.{ext}");
                self.attachments.push((name.clone(), bytes));
                Some(name)
            }
            _ => None,
        }
    }
}

/// A `{code}` macro. Its body is verbatim, so only a literal `{code}`
/// inside would end it early: that one is broken up.
fn code(lang: &str, text: &str) -> String {
    let lang = if lang.is_empty()
        || !lang
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+#-_".contains(c))
    {
        String::new()
    } else {
        format!(":language={lang}")
    };
    let body = text.replace("{code}", "{ code}");
    let body = body.trim_end_matches('\n');
    format!("{{code{lang}}}\n{body}\n{{code}}\n\n")
}

/// Escapes text: `\` before every character that can start markup, so
/// item titles and bodies read as written.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(
            c,
            '\\' | '*'
                | '_'
                | '-'
                | '+'
                | '^'
                | '~'
                | '?'
                | '{'
                | '}'
                | '['
                | ']'
                | '|'
                | '!'
                | '#'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A macro parameter value: `|`, `}` and `=` end or split it and can't be
/// escaped there, so they become look-alikes.
fn attr(s: &str) -> String {
    s.replace('|', "¦")
        .replace('}', ")")
        .replace('{', "(")
        .replace('=', "＝")
}

/// A link or image target: `|` and `]` end it, so they are
/// percent-encoded (a page title with them can't be linked by title).
fn target(s: &str) -> String {
    s.replace('|', "%7C")
        .replace(']', "%5D")
        .replace('[', "%5B")
}

/// A table cell: a line break would end the row, and an empty cell
/// needs a space to keep its column.
fn cell(s: &str) -> String {
    let s = s.replace('\n', " ");
    if s.trim().is_empty() {
        " ".to_owned()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_escapes_what_starts_markup() {
        assert_eq!(
            esc("a-b *c* [d|e] {f} #g!"),
            "a\\-b \\*c\\* \\[d\\|e\\] \\{f\\} \\#g\\!"
        );
        assert_eq!(esc("plain text, 42."), "plain text, 42.");
    }

    #[test]
    fn code_blocks_keep_their_text_and_language() {
        assert_eq!(
            code("rust", "let x = 1;\n"),
            "{code:language=rust}\nlet x = 1;\n{code}\n\n"
        );
        assert_eq!(code("", "{code} *x*"), "{code}\n{ code} *x*\n{code}\n\n");
        assert_eq!(code("a b", "x"), "{code}\nx\n{code}\n\n");
    }

    #[test]
    fn targets_and_parameters_stay_inside_their_markup() {
        assert_eq!(target("https://x/a|b]"), "https://x/a%7Cb%5D");
        assert_eq!(attr("a|b=c}"), "a¦b＝c)");
        assert_eq!(cell(""), " ");
    }
}
