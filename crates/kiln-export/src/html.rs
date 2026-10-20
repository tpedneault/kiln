//! The HTML back end (EXPORT.md, "`html`"): one self-contained file per
//! page, styles inline, images as `data:` URIs, no scripts and no network
//! fetches. A print stylesheet (page breaks before top-level sections,
//! link URLs shown) makes the browser's "Print to PDF" a decent PDF, which
//! covers PDF without a PDF engine.

use std::collections::HashMap;

use kiln_index::FileSource;
use kiln_model::ItemStyle;
use kiln_render::{Align, Block, Inline, Props, Target};

use crate::document::{Node, Section, SectionKind};
use crate::markdown::{gitlab_slug, FileContext};
use crate::paths::{page_file, relative};

const STYLE: &str = r#"
:root { --fg: #1f2328; --muted: #59636e; --line: #d1d9e0; --soft: #f6f8fa; --link: #0969da; }
body { color: var(--fg); background: #fff; font: 15px/1.55 -apple-system, "Segoe UI", "Noto Sans", Helvetica, Arial, sans-serif; max-width: 60rem; margin: 2rem auto; padding: 0 1.5rem; }
h1, h2, h3, h4, h5, h6 { line-height: 1.25; margin: 1.6em 0 0.6em; }
h1.title { font-size: 2.1em; margin-top: 0; border-bottom: 1px solid var(--line); padding-bottom: 0.3em; }
a { color: var(--link); text-decoration: none; } a:hover { text-decoration: underline; }
code, pre { font-family: ui-monospace, "Cascadia Mono", Consolas, monospace; font-size: 0.9em; background: var(--soft); border-radius: 4px; }
code { padding: 0.1em 0.3em; } pre { padding: 0.8em 1em; overflow-x: auto; } pre code { background: none; padding: 0; }
table { border-collapse: collapse; margin: 0.8em 0; } th, td { border: 1px solid var(--line); padding: 0.35em 0.7em; text-align: left; vertical-align: top; } th { background: var(--soft); }
blockquote { margin: 0.8em 0; padding: 0 1em; color: var(--muted); border-left: 4px solid var(--line); }
.callout { margin: 0.8em 0; padding: 0.5em 1em; border-left: 4px solid; border-radius: 4px; background: var(--soft); }
.callout > .kind { font-weight: 600; margin: 0.3em 0; }
.callout.note, .callout.important { border-color: #0969da; } .callout.tip { border-color: #1a7f37; } .callout.warning { border-color: #9a6700; } .callout.caution { border-color: #cf222e; }
.props { color: var(--muted); margin: 0.2em 0 0.8em; }
.status { display: inline-block; font-size: 0.75em; font-weight: 600; text-transform: uppercase; letter-spacing: 0.03em; padding: 0.1em 0.5em; border-radius: 3px; color: #fff; }
.status.grey { background: #6e7781; } .status.blue { background: #0969da; } .status.green { background: #1a7f37; } .status.yellow { background: #9a6700; } .status.red { background: #cf222e; }
.diagram img { max-width: 100%; }
details summary { color: var(--muted); cursor: pointer; }
nav.toc ul { list-style: none; padding-left: 1.2em; } nav.toc > ul { padding-left: 0; }
@media print {
  body { max-width: none; margin: 0; padding: 0; font-size: 11pt; }
  section.top { break-before: page; }
  a { color: inherit; }
  a[href^="http"]::after { content: " (" attr(href) ")"; font-size: 0.85em; color: var(--muted); word-break: break-all; }
  details { display: none; }
  pre, table, .diagram { break-inside: avoid; }
}
"#;

/// Every file of an HTML target: (project-relative path, bytes).
// implements: REQ-002
pub fn render(ctx: &FileContext, files: &dyn FileSource) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = ctx
        .split
        .pages
        .iter()
        .map(|page| {
            let file = page_file(ctx.target, ctx.split, &page.key, "html");
            let mut w = Writer { ctx, files, file: file.clone(), page_key: page.key.clone(), base: page.base_depth, ids: HashMap::new() };
            let body = w.nodes(&page.nodes);
            let html = format!(
                "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"generator\" content=\"Kiln\">\n<title>{}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<h1 class=\"title\">{}</h1>\n{body}</body>\n</html>\n",
                esc(&page.title),
                esc(&page.title)
            );
            (file, html.into_bytes())
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

struct Writer<'a> {
    ctx: &'a FileContext<'a>,
    files: &'a dyn FileSource,
    file: String,
    page_key: String,
    base: u8,
    ids: HashMap<String, usize>,
}

impl Writer<'_> {
    fn level(&self, depth: u8) -> usize {
        (depth as i32 - self.base as i32 + 2).clamp(1, 7) as usize
    }

    fn section_id(&mut self, s: &Section) -> String {
        match &s.kind {
            SectionKind::Item(id) => id.to_string(),
            _ => self.unique(gitlab_slug(&s.heading_text())),
        }
    }

    fn unique(&mut self, base: String) -> String {
        let n = self.ids.entry(base.clone()).or_insert(0);
        let id = if *n == 0 {
            base.clone()
        } else {
            format!("{base}-{n}")
        };
        *n += 1;
        id
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
        let id = self.section_id(s);
        let level = self.level(s.depth).min(6);
        let class = if s.depth == self.base {
            " class=\"top\""
        } else {
            ""
        };
        let marker = s
            .marker_inline()
            .map(|m| format!(" {}", self.inlines(&[m])))
            .unwrap_or_default();
        format!(
            "<section{class}>\n<h{level} id=\"{}\">{}{marker}</h{level}>\n{}</section>\n",
            esc(&id),
            esc(&s.heading_text()),
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
                    format!("<p><strong>{text}</strong></p>\n")
                } else {
                    format!("<h{level}>{text}</h{level}>\n")
                }
            }
            Block::Paragraph(c) => format!("<p>{}</p>\n", self.inlines(c)),
            Block::List { start, items } => {
                let (open, close) = match start {
                    Some(1) => ("<ol>".to_owned(), "</ol>"),
                    Some(s) => (format!("<ol start=\"{s}\">"), "</ol>"),
                    None => ("<ul>".to_owned(), "</ul>"),
                };
                let mut out = format!("{open}\n");
                for item in items {
                    let task = match item.task {
                        Some(true) => "☑ ",
                        Some(false) => "☐ ",
                        None => "",
                    };
                    // A tight item is one paragraph: no <p> inside <li>.
                    let body = match item.content.as_slice() {
                        [Block::Paragraph(c)] => self.inlines(c),
                        blocks => self.blocks(blocks),
                    };
                    out.push_str(&format!("<li>{task}{body}</li>\n"));
                }
                out.push_str(close);
                out.push('\n');
                out
            }
            Block::Quote(b) => format!("<blockquote>\n{}</blockquote>\n", self.blocks(b)),
            Block::Callout { kind, content } => format!(
                "<div class=\"callout {}\">\n<p class=\"kind\">{}</p>\n{}</div>\n",
                kind.name().to_lowercase(),
                callout_title(kind.name()),
                self.blocks(content)
            ),
            Block::Code { lang, text } => code_block(lang, text),
            Block::Diagram { source, .. } => {
                // HTML always shows pictures (EXPORT.md, "Diagrams").
                match self.ctx.pictures.diagram(source) {
                    Ok(p) => format!(
                        "<figure class=\"diagram\"><img alt=\"Diagram\" src=\"data:image/png;base64,{}\"></figure>\n<details><summary>Diagram source</summary>\n{}</details>\n",
                        base64(&p.png),
                        code_block("mermaid", source)
                    ),
                    Err(e) => format!(
                        "<div class=\"callout warning\">\n<p class=\"kind\">Warning</p>\n<p>This diagram couldn't be drawn: {}</p>\n</div>\n{}",
                        esc(&e),
                        code_block("mermaid", source)
                    ),
                }
            }
            Block::Table(t) => {
                let mut out = "<table>\n<thead><tr>".to_owned();
                for (i, h) in t.header.iter().enumerate() {
                    out.push_str(&format!(
                        "<th{}>{}</th>",
                        align(t.align.get(i)),
                        self.inlines(h)
                    ));
                }
                out.push_str("</tr></thead>\n<tbody>\n");
                for row in &t.rows {
                    out.push_str("<tr>");
                    for (i, c) in row.iter().enumerate() {
                        out.push_str(&format!(
                            "<td{}>{}</td>",
                            align(t.align.get(i)),
                            self.inlines(c)
                        ));
                    }
                    out.push_str("</tr>\n");
                }
                out.push_str("</tbody>\n</table>\n");
                out
            }
            Block::Rule => "<hr>\n".to_owned(),
            Block::Directive(_) => String::new(),
            Block::Toc => self.toc(),
            Block::Props(p) => self.props(p),
            Block::ChildPages => {
                let items: String = self
                    .ctx
                    .split
                    .children(&self.page_key)
                    .iter()
                    .map(|p| {
                        let href = relative(
                            &self.file,
                            &page_file(self.ctx.target, self.ctx.split, &p.key, "html"),
                        );
                        format!(
                            "<li><a href=\"{}\">{}</a></li>\n",
                            esc(&crate::paths::encode(&href)),
                            esc(&p.title)
                        )
                    })
                    .collect();
                format!("<ul class=\"children\">\n{items}</ul>\n")
            }
        }
    }

    fn props(&mut self, props: &Props) -> String {
        let status = self.inline(&props.status);
        match props.style {
            ItemStyle::None => String::new(),
            ItemStyle::List => {
                let mut lines = vec![status];
                for e in &props.entries {
                    let values: Vec<String> = e.values.iter().map(|v| self.inlines(v)).collect();
                    lines.push(format!(
                        "<strong>{}:</strong> {}",
                        esc(&e.label),
                        values.join(", ")
                    ));
                }
                format!("<p class=\"props\">{}</p>\n", lines.join("<br>\n"))
            }
            ItemStyle::Line => {
                let mut parts = vec![status];
                for e in &props.entries {
                    let values: Vec<String> = e.values.iter().map(|v| self.inlines(v)).collect();
                    parts.push(format!("{} {}", esc(&e.lead), values.join(", ")));
                }
                format!("<p class=\"props\">{}</p>\n", parts.join(" · "))
            }
            ItemStyle::Table => {
                let mut out =
                    format!("<table class=\"props\">\n<tr><th>Status</th><td>{status}</td></tr>\n");
                for e in &props.entries {
                    let values: Vec<String> = e.values.iter().map(|v| self.inlines(v)).collect();
                    out.push_str(&format!(
                        "<tr><th>{}</th><td>{}</td></tr>\n",
                        esc(&e.label),
                        values.join(", ")
                    ));
                }
                out.push_str("</table>\n");
                out
            }
        }
    }

    fn toc(&mut self) -> String {
        // Ids are handed out in document order, so compute them the same
        // way on a scratch writer.
        let mut scratch = Writer {
            ctx: self.ctx,
            files: self.files,
            file: self.file.clone(),
            page_key: self.page_key.clone(),
            base: self.base,
            ids: HashMap::new(),
        };
        let Some(page) = self.ctx.split.page(&self.page_key) else {
            return String::new();
        };
        // Every section gets its id in order, listed or not, so repeated
        // headings are numbered as the page numbers them.
        fn walk(w: &mut Writer, nodes: &[Node], depth: usize) -> String {
            let mut out = String::new();
            for n in nodes {
                if let Node::Section(s) = n {
                    let id = w.section_id(s);
                    let inner = walk(w, &s.nodes, depth + 1);
                    if depth > 2 {
                        continue;
                    }
                    let inner = if inner.is_empty() {
                        inner
                    } else {
                        format!("<ul>\n{inner}</ul>\n")
                    };
                    out.push_str(&format!(
                        "<li><a href=\"#{}\">{}</a>{inner}</li>\n",
                        esc(&id),
                        esc(&s.heading_text())
                    ));
                }
            }
            out
        }
        let items = walk(&mut scratch, &page.nodes, 0);
        if items.is_empty() {
            String::new()
        } else {
            format!("<nav class=\"toc\">\n<ul>\n{items}</ul>\n</nav>\n")
        }
    }

    fn inlines(&mut self, inlines: &[Inline]) -> String {
        inlines.iter().map(|i| self.inline(i)).collect()
    }

    fn inline(&mut self, inline: &Inline) -> String {
        match inline {
            Inline::Text(s) => esc(s),
            Inline::Code(s) => format!("<code>{}</code>", esc(s)),
            Inline::Emph(c) => format!("<em>{}</em>", self.inlines(c)),
            Inline::Strong(c) => format!("<strong>{}</strong>", self.inlines(c)),
            Inline::Strike(c) => format!("<del>{}</del>", self.inlines(c)),
            Inline::Link { target, content } => {
                let text = self.inlines(content);
                match self.href(target) {
                    Some(href) => format!("<a href=\"{}\">{text}</a>", esc(&href)),
                    None => text,
                }
            }
            Inline::Image { target, alt } => match self.image(target) {
                Some(src) => format!("<img alt=\"{}\" src=\"{src}\">", esc(alt)),
                None => esc(alt),
            },
            Inline::Ref { id, text } => esc(text.as_deref().unwrap_or(&id.to_string())),
            Inline::Status { text, tone } => format!(
                "<span class=\"status {}\">{}</span>",
                tone.name().to_lowercase(),
                esc(text)
            ),
            Inline::Jira(key) => match self.ctx.jira_url {
                Some(url) => format!(
                    "<a href=\"{}/browse/{}\">{}</a>",
                    esc(url.trim_end_matches('/')),
                    esc(key),
                    esc(key)
                ),
                None => esc(key),
            },
            Inline::Value(_) => String::new(),
            Inline::Break => "<br>".to_owned(),
            Inline::Anchor(a) => format!("<span id=\"{}\"></span>", esc(a)),
        }
    }

    fn href(&self, target: &Target) -> Option<String> {
        match target {
            Target::Url(u) => Some(u.clone()),
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
            Target::Anchor(a) => Some(format!("#{a}")),
            Target::Page {
                export,
                key,
                anchor,
            } => {
                let file = self.ctx.file_of(export.as_deref(), key, "html")?;
                let mut url = if file == self.file {
                    String::new()
                } else {
                    crate::paths::encode(&relative(&self.file, &file))
                };
                if let Some(a) = anchor {
                    url.push_str(&format!("#{a}"));
                }
                Some(url)
            }
        }
    }

    /// Images are embedded, never fetched: a repository image as a
    /// `data:` URI, a remote one left out (shown as its alt text).
    fn image(&self, target: &Target) -> Option<String> {
        let Target::Repo { path, .. } = target else {
            return None;
        };
        let bytes = self.files.read(path).ok()?;
        let mime = match path
            .rsplit('.')
            .next()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("png") => "image/png",
            Some("jpg" | "jpeg") => "image/jpeg",
            Some("gif") => "image/gif",
            Some("svg") => "image/svg+xml",
            Some("webp") => "image/webp",
            _ => return None,
        };
        Some(format!("data:{mime};base64,{}", base64(&bytes)))
    }
}

fn callout_title(kind: &str) -> String {
    let mut c = kind.to_lowercase();
    if let Some(first) = c.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    c
}

fn align(a: Option<&Align>) -> &'static str {
    match a {
        Some(Align::Left) => " style=\"text-align:left\"",
        Some(Align::Center) => " style=\"text-align:center\"",
        Some(Align::Right) => " style=\"text-align:right\"",
        _ => "",
    }
}

fn code_block(lang: &str, text: &str) -> String {
    let class = if lang.is_empty() {
        String::new()
    } else {
        format!(" class=\"language-{}\"", esc(lang))
    };
    format!("<pre><code{class}>{}</code></pre>\n", esc(text))
}

/// HTML-escapes text and attribute values.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Standard base64 with padding.
pub fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping_and_base64() {
        assert_eq!(
            esc("<a href=\"x\">&'"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
