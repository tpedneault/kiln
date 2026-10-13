//! Confluence storage format (CONFLUENCE.md, "Storage format"): XHTML
//! with `ac:`/`ri:` elements. Always well-formed (every golden file is
//! parsed in the tests), every text escaped, raw HTML never passed
//! through.

use std::collections::HashMap;

use kiln_export::{Node, Page, RepoLinks, Section, SectionKind};
use kiln_model::{ConfluenceConfig, Diagrams, ItemStyle};
use kiln_render::{Align, Block, Callout, Inline, Props, Target};

/// What a page attaches: content-addressed, so only ever created and
/// deleted, never re-uploaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub name: String,
    pub source: AttachmentSource,
    /// Content hash: the picture name for diagrams (already
    /// content-addressed by source), sha256 of the bytes for files.
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentSource {
    /// A Mermaid diagram, rasterised when published (K3).
    Diagram(String),
    /// A file in the repository.
    File(String),
}

/// A page's body and attachments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Emitted {
    pub body: String,
    pub attachments: Vec<Attachment>,
}

/// What emitting needs beyond the page.
pub struct EmitContext<'a> {
    pub config: &'a ConfluenceConfig,
    /// The space pages are in.
    pub space: &'a str,
    /// Final titles of every page a link may name: (export, key) → title,
    /// with `None` for this export.
    pub titles: &'a HashMap<(Option<String>, String), String>,
    /// Spaces of `link_to` exports, by name.
    pub spaces: &'a HashMap<String, String>,
    pub repo: &'a RepoLinks,
    pub jira_url: Option<&'a str>,
    /// The info banner's text for this page.
    pub banner: String,
    /// Content hashes of repository images, by path.
    pub image_hash: &'a dyn Fn(&str) -> Option<String>,
    /// The table of contents' depth.
    pub toc_levels: u8,
}

struct Emitter<'a> {
    ctx: &'a EmitContext<'a>,
    base: u8,
    attachments: Vec<Attachment>,
}

/// The storage body of `page`.
pub fn emit(page: &Page, ctx: &EmitContext) -> Emitted {
    let mut e = Emitter {
        ctx,
        base: page.base_depth,
        attachments: Vec::new(),
    };
    let mut body = macro_body("info", &[], &format!("<p>{}</p>", esc(&ctx.banner)));
    body.push_str(&e.nodes(&page.nodes));
    let mut attachments = e.attachments;
    attachments.sort_by(|a, b| a.name.cmp(&b.name));
    attachments.dedup_by(|a, b| a.name == b.name);
    Emitted { body, attachments }
}

fn macro_open(name: &str, params: &[(&str, &str)]) -> String {
    let mut out = format!("<ac:structured-macro ac:name=\"{}\">", esc(name));
    for (k, v) in params {
        out.push_str(&format!(
            "<ac:parameter ac:name=\"{}\">{}</ac:parameter>",
            esc(k),
            esc(v)
        ));
    }
    out
}

fn macro_empty(name: &str, params: &[(&str, &str)]) -> String {
    format!("{}</ac:structured-macro>", macro_open(name, params))
}

fn macro_body(name: &str, params: &[(&str, &str)], rich: &str) -> String {
    format!(
        "{}<ac:rich-text-body>{rich}</ac:rich-text-body></ac:structured-macro>",
        macro_open(name, params)
    )
}

fn code_macro(lang: &str, text: &str) -> String {
    let params: Vec<(&str, &str)> = if lang.is_empty() {
        vec![]
    } else {
        vec![("language", lang)]
    };
    format!(
        "{}<ac:plain-text-body>{}</ac:plain-text-body></ac:structured-macro>",
        macro_open("code", &params),
        cdata(text)
    )
}

/// CDATA, with any `]]>` split across two sections.
fn cdata(text: &str) -> String {
    format!("<![CDATA[{}]]>", text.replace("]]>", "]]]]><![CDATA[>"))
}

impl Emitter<'_> {
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
            SectionKind::Item(id) => macro_empty("anchor", &[("", &id.to_string())]),
            _ => String::new(),
        };
        let marker = s
            .marker_inline()
            .map(|m| format!(" {}", self.inlines(&[m])))
            .unwrap_or_default();
        format!(
            "<h{level}>{anchor}{}{marker}</h{level}>{}",
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
                    format!("<p><strong>{text}</strong></p>")
                } else {
                    format!("<h{0}>{text}</h{0}>", level.max(1))
                }
            }
            Block::Paragraph(c) => format!("<p>{}</p>", self.inlines(c)),
            Block::List { start, items } => {
                let tag = if start.is_some() { "ol" } else { "ul" };
                let mut out = format!("<{tag}>");
                for item in items {
                    // Confluence task lists are interactive, which invites
                    // edits: tasks are text.
                    let task = match item.task {
                        Some(true) => "☑ ",
                        Some(false) => "☐ ",
                        None => "",
                    };
                    let body = match item.content.as_slice() {
                        [Block::Paragraph(c)] => self.inlines(c),
                        blocks => self.blocks(blocks),
                    };
                    out.push_str(&format!("<li>{task}{body}</li>"));
                }
                out.push_str(&format!("</{tag}>"));
                out
            }
            Block::Quote(b) => format!("<blockquote>{}</blockquote>", self.blocks(b)),
            Block::Callout { kind, content } => {
                let name = match kind {
                    Callout::Note | Callout::Important => "info",
                    Callout::Tip => "tip",
                    Callout::Warning => "note",
                    Callout::Caution => "warning",
                };
                let inner = self.blocks(content);
                macro_body(name, &[], &inner)
            }
            Block::Code { lang, text } => code_macro(lang, text),
            Block::Diagram { source, .. } => self.diagram(source),
            Block::Table(t) => {
                let mut out = "<table><tbody><tr>".to_owned();
                for (i, h) in t.header.iter().enumerate() {
                    out.push_str(&format!(
                        "<th{}>{}</th>",
                        align(t.align.get(i)),
                        self.inlines(h)
                    ));
                }
                out.push_str("</tr>");
                for row in &t.rows {
                    out.push_str("<tr>");
                    for (i, c) in row.iter().enumerate() {
                        out.push_str(&format!(
                            "<td{}>{}</td>",
                            align(t.align.get(i)),
                            self.inlines(c)
                        ));
                    }
                    out.push_str("</tr>");
                }
                out.push_str("</tbody></table>");
                out
            }
            Block::Rule => "<hr />".to_owned(),
            Block::Directive(_) => String::new(),
            Block::Toc => macro_empty("toc", &[("maxLevel", &self.ctx.toc_levels.to_string())]),
            Block::Props(p) => self.props(p),
            Block::ChildPages => macro_empty("children", &[("all", "true")]),
        }
    }

    fn diagram(&mut self, source: &str) -> String {
        let code = code_macro("mermaid", source);
        if self.ctx.config.diagrams == Diagrams::Source {
            return code;
        }
        if let Some((line, _, message)) = kiln_render::diagram_error(source) {
            let warning = macro_body(
                "warning",
                &[],
                &format!(
                    "<p>This diagram couldn't be drawn: line {}: {}</p>",
                    line + 1,
                    esc(&message)
                ),
            );
            return format!("{warning}{code}");
        }
        let name = kiln_export::picture_name(source);
        self.attachments.push(Attachment {
            name: name.clone(),
            source: AttachmentSource::Diagram(source.to_owned()),
            hash: name.clone(),
        });
        format!(
            "<p><ac:image><ri:attachment ri:filename=\"{}\" /></ac:image></p>{}",
            esc(&name),
            macro_body("expand", &[("title", "Diagram source")], &code)
        )
    }

    fn props(&mut self, props: &Props) -> String {
        let status = self.inline(&props.status);
        match props.style {
            ItemStyle::None => String::new(),
            ItemStyle::Line => {
                let mut parts = vec![status];
                for e in &props.entries {
                    let values: Vec<String> = e.values.iter().map(|v| self.inlines(v)).collect();
                    parts.push(format!("{} {}", esc(&e.lead), values.join(", ")));
                }
                format!("<p>{}</p>", parts.join(" · "))
            }
            ItemStyle::Table => {
                let mut out = format!("<table><tbody><tr><th>Status</th><td>{status}</td></tr>");
                for e in &props.entries {
                    let values: Vec<String> = e.values.iter().map(|v| self.inlines(v)).collect();
                    out.push_str(&format!(
                        "<tr><th>{}</th><td>{}</td></tr>",
                        esc(&e.label),
                        values.join(", ")
                    ));
                }
                out.push_str("</tbody></table>");
                out
            }
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
            Inline::Link { target, content } => self.link(target, content),
            Inline::Image { target, alt } => match target {
                Target::Repo { path, .. } => match (self.ctx.image_hash)(path) {
                    Some(hash) => {
                        let ext = path
                            .rsplit('.')
                            .next()
                            .unwrap_or("png")
                            .to_ascii_lowercase();
                        let name = format!("image-{}.{ext}", &hash[..hash.len().min(12)]);
                        self.attachments.push(Attachment {
                            name: name.clone(),
                            source: AttachmentSource::File(path.clone()),
                            hash,
                        });
                        format!("<ac:image ac:alt=\"{}\"><ri:attachment ri:filename=\"{}\" /></ac:image>", esc(alt), esc(&name))
                    }
                    None => esc(alt),
                },
                Target::Url(u) => format!(
                    "<ac:image ac:alt=\"{}\"><ri:url ri:value=\"{}\" /></ac:image>",
                    esc(alt),
                    esc(u)
                ),
                _ => esc(alt),
            },
            Inline::Ref { id, text } => esc(text.as_deref().unwrap_or(&id.to_string())),
            Inline::Status { text, tone } => {
                macro_empty("status", &[("colour", tone.name()), ("title", text)])
            }
            Inline::Jira(key) => {
                if self.ctx.config.jira_macro {
                    let mut params = vec![("key", key.as_str())];
                    if !self.ctx.config.jira_server_id.is_empty() {
                        params.push(("serverId", self.ctx.config.jira_server_id.as_str()));
                    }
                    macro_empty("jira", &params)
                } else {
                    match self.ctx.jira_url {
                        Some(url) => format!(
                            "<a href=\"{}/browse/{}\">{}</a>",
                            esc(url.trim_end_matches('/')),
                            esc(key),
                            esc(key)
                        ),
                        None => esc(key),
                    }
                }
            }
            Inline::Value(_) => String::new(),
            Inline::Break => "<br />".to_owned(),
            Inline::Anchor(a) => macro_empty("anchor", &[("", a)]),
        }
    }

    fn link(&mut self, target: &Target, content: &[Inline]) -> String {
        let text = kiln_render::plain(content);
        let body = format!(
            "<ac:plain-text-link-body>{}</ac:plain-text-link-body>",
            cdata(&text)
        );
        match target {
            Target::Url(u) => format!("<a href=\"{}\">{}</a>", esc(u), self.inlines(content)),
            Target::Anchor(a) => format!("<ac:link ac:anchor=\"{}\">{body}</ac:link>", esc(a)),
            Target::Page {
                export,
                key,
                anchor,
            } => {
                let Some(title) = self.ctx.titles.get(&(export.clone(), key.clone())) else {
                    return self.inlines(content);
                };
                let space = match export {
                    Some(name) => self
                        .ctx
                        .spaces
                        .get(name)
                        .map(String::as_str)
                        .unwrap_or(self.ctx.space),
                    None => self.ctx.space,
                };
                let anchor = anchor
                    .as_ref()
                    .map(|a| format!(" ac:anchor=\"{}\"", esc(a)))
                    .unwrap_or_default();
                format!(
                    "<ac:link{anchor}><ri:page ri:content-title=\"{}\" ri:space-key=\"{}\" />{body}</ac:link>",
                    esc(title),
                    esc(space)
                )
            }
            Target::Repo { path, line, .. } => match self.ctx.repo.url("", path, *line) {
                Some(url) => format!("<a href=\"{}\">{}</a>", esc(&url), self.inlines(content)),
                None => self.inlines(content),
            },
        }
    }
}

fn align(a: Option<&Align>) -> &'static str {
    match a {
        Some(Align::Center) => " style=\"text-align: center;\"",
        Some(Align::Right) => " style=\"text-align: right;\"",
        _ => "",
    }
}

/// XML-escapes text and attribute values.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // Control characters aren't allowed in XML 1.0 at all.
            c if (c as u32) < 0x20 && !matches!(c, '\n' | '\t' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}
