//! Markdown (an item body, or a template) to blocks, through
//! `pulldown-cmark` with GitHub tables, task lists, strikethrough and
//! callouts. References become `Inline::Ref`, Mermaid blocks
//! `Block::Diagram`, relative links `Target::Repo`. Raw HTML becomes text:
//! it is never passed through (DESIGN.md, "Security"). In a template,
//! `<!-- kiln:… -->` comments become directives and values.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use kiln_model::ItemId;
use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, Event, Options, Parser, Tag, TagEnd,
};
use regex::Regex;

use crate::doc::{Align, Block, Callout, Directive, Inline, ListItem, Table, Target};

/// What rendering a body needs to know.
pub struct BodyContext<'a> {
    /// The project-relative folder of the file (`design/req`), for
    /// relative links.
    pub dir: &'a str,
    /// The file line the text starts on (1-based).
    pub first_line: u32,
    /// Configured ID prefixes, for bare references.
    pub prefixes: &'a [String],
    pub link_bare_ids: bool,
    /// A template: `<!-- kiln:… -->` comments are directives.
    pub template: bool,
}

enum Inl {
    Paragraph,
    Heading(u8),
    Emph,
    Strong,
    Strike,
    Link(Target),
    Image(Target),
    Cell,
}

enum Frame {
    Blocks(Vec<Block>),
    Quote(Option<Callout>, Vec<Block>),
    List(Option<u64>, Vec<ListItem>),
    Item(Option<bool>, Vec<Block>, Vec<Inline>),
    Inlines(Inl, Vec<Inline>),
    Table(
        Vec<Align>,
        Vec<Vec<Inline>>,
        Vec<Vec<Vec<Inline>>>,
        Vec<Vec<Inline>>,
    ),
    Code(String, String, bool, u32),
    Html(String, u32),
}

struct Builder<'a> {
    ctx: &'a BodyContext<'a>,
    stack: Vec<Frame>,
    text: String,
    explicit: Regex,
    bare: Option<Regex>,
    directive: Regex,
}

/// Renders Markdown to blocks.
pub fn render_markdown(text: &str, ctx: &BodyContext) -> Vec<Block> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_GFM);
    let bare = (ctx.link_bare_ids && !ctx.prefixes.is_empty()).then(|| bare_regex(ctx.prefixes));
    let mut b = Builder {
        ctx,
        stack: vec![Frame::Blocks(Vec::new())],
        text: String::new(),
        explicit: explicit_regex().clone(),
        bare,
        directive: directive_regex().clone(),
    };
    let line_of = |offset: usize| {
        ctx.first_line + text[..offset.min(text.len())].matches('\n').count() as u32
    };
    for (event, range) in Parser::new_ext(text, opts).into_offset_iter() {
        let is_text = matches!(event, Event::Text(_))
            || (matches!(&event, Event::InlineHtml(h) if !(ctx.template && h.contains("kiln:"))));
        if !is_text {
            b.flush();
        }
        match event {
            Event::Text(t) => match b.stack.last_mut() {
                Some(Frame::Code(_, code, _, _)) => code.push_str(&t),
                Some(Frame::Html(html, _)) => html.push_str(&t),
                _ => b.text.push_str(&t),
            },
            Event::Code(c) => b.inline(Inline::Code(c.to_string())),
            Event::InlineHtml(h) => match ctx.template.then(|| b.value_directive(&h)).flatten() {
                Some(value) => b.inline(value),
                // Shown as text, merged with the text around it.
                None => b.text.push_str(&h),
            },
            Event::Html(h) => match b.stack.last_mut() {
                Some(Frame::Html(html, _)) => html.push_str(&h),
                _ => b.inline(Inline::Text(h.to_string())),
            },
            Event::SoftBreak => b.text.push(' '),
            Event::HardBreak => b.inline(Inline::Break),
            Event::Rule => b.block(Block::Rule),
            Event::TaskListMarker(done) => {
                if let Some(Frame::Item(task, _, _)) = b.stack.last_mut() {
                    *task = Some(done);
                }
            }
            Event::FootnoteReference(f) => b.inline(Inline::Text(format!("[^{f}]"))),
            Event::InlineMath(m) | Event::DisplayMath(m) => b.inline(Inline::Code(m.to_string())),
            Event::Start(tag) => match tag {
                Tag::Paragraph => b.stack.push(Frame::Inlines(Inl::Paragraph, Vec::new())),
                Tag::Heading { level, .. } => b
                    .stack
                    .push(Frame::Inlines(Inl::Heading(level as u8), Vec::new())),
                Tag::BlockQuote(kind) => b.stack.push(Frame::Quote(kind.map(callout), Vec::new())),
                Tag::CodeBlock(kind) => {
                    let lang = match kind {
                        CodeBlockKind::Fenced(info) => {
                            info.split_whitespace().next().unwrap_or("").to_owned()
                        }
                        CodeBlockKind::Indented => String::new(),
                    };
                    let mermaid = lang == "mermaid";
                    b.stack.push(Frame::Code(
                        lang,
                        String::new(),
                        mermaid,
                        line_of(range.start) + 1,
                    ));
                }
                Tag::HtmlBlock => b
                    .stack
                    .push(Frame::Html(String::new(), line_of(range.start))),
                Tag::List(start) => b.stack.push(Frame::List(start, Vec::new())),
                Tag::Item => b.stack.push(Frame::Item(None, Vec::new(), Vec::new())),
                Tag::Table(aligns) => {
                    let align = aligns
                        .iter()
                        .map(|a| match a {
                            Alignment::None => Align::None,
                            Alignment::Left => Align::Left,
                            Alignment::Center => Align::Center,
                            Alignment::Right => Align::Right,
                        })
                        .collect();
                    b.stack
                        .push(Frame::Table(align, Vec::new(), Vec::new(), Vec::new()));
                }
                Tag::TableHead | Tag::TableRow => {}
                Tag::TableCell => b.stack.push(Frame::Inlines(Inl::Cell, Vec::new())),
                Tag::Emphasis => b.stack.push(Frame::Inlines(Inl::Emph, Vec::new())),
                Tag::Strong => b.stack.push(Frame::Inlines(Inl::Strong, Vec::new())),
                Tag::Strikethrough => b.stack.push(Frame::Inlines(Inl::Strike, Vec::new())),
                Tag::Link { dest_url, .. } => {
                    let target = b.target(&dest_url);
                    b.stack.push(Frame::Inlines(Inl::Link(target), Vec::new()));
                }
                Tag::Image { dest_url, .. } => {
                    let target = b.target(&dest_url);
                    b.stack.push(Frame::Inlines(Inl::Image(target), Vec::new()));
                }
                // Not enabled; kept as their content.
                _ => b.stack.push(Frame::Inlines(Inl::Emph, Vec::new())),
            },
            Event::End(end) => b.end(end),
        }
    }
    b.flush();
    while b.stack.len() > 1 {
        b.close_top();
    }
    match b.stack.pop() {
        Some(Frame::Blocks(blocks)) => blocks,
        _ => Vec::new(),
    }
}

// Compiled once: a document renders thousands of bodies, and compiling
// these per body was most of an export's time. (`Regex` clones share the
// compiled program.)
fn explicit_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\[\[([^\]\|\n]+)(?:\|([^\]\n]*))?\]\]").expect("a valid literal regex")
    })
}

fn directive_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\s*<!--\s*kiln:([a-z]+)\b(.*?)-->\s*$").expect("a valid literal regex")
    })
}

/// The bare-reference regex for a set of prefixes, compiled once per set.
fn bare_regex(prefixes: &[String]) -> Regex {
    static CACHE: OnceLock<Mutex<HashMap<Vec<String>, Regex>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let compile = || {
        let alternatives: Vec<String> = prefixes.iter().map(|p| regex::escape(p)).collect();
        Regex::new(&format!(r"\b(?:{})-\d+\b", alternatives.join("|")))
            .expect("prefixes are validated letters and digits")
    };
    match cache.lock() {
        Ok(mut map) => map.entry(prefixes.to_vec()).or_insert_with(compile).clone(),
        Err(_) => compile(),
    }
}

fn callout(kind: BlockQuoteKind) -> Callout {
    match kind {
        BlockQuoteKind::Note => Callout::Note,
        BlockQuoteKind::Tip => Callout::Tip,
        BlockQuoteKind::Important => Callout::Important,
        BlockQuoteKind::Warning => Callout::Warning,
        BlockQuoteKind::Caution => Callout::Caution,
    }
}

impl Builder<'_> {
    fn target(&self, dest: &str) -> Target {
        let dest = dest.trim();
        if let Some(anchor) = dest.strip_prefix('#') {
            return Target::Anchor(anchor.to_owned());
        }
        match relative_path(dest) {
            Some((path, fragment)) => match resolve(self.ctx.dir, &path) {
                Some(path) => Target::Repo {
                    path,
                    line: None,
                    fragment,
                },
                None => Target::Url(dest.to_owned()),
            },
            None => Target::Url(dest.to_owned()),
        }
    }

    fn value_directive(&self, html: &str) -> Option<Inline> {
        let caps = self.directive.captures(html)?;
        (&caps[1] == "value").then(|| Inline::Value(caps[2].trim().to_owned()))
    }

    /// Moves pending text into the current inline container, splitting out
    /// references.
    fn flush(&mut self) {
        if self.text.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.text);
        for inline in split_references(&text, &self.explicit, self.bare.as_ref()) {
            self.inline(inline);
        }
    }

    fn inline(&mut self, inline: Inline) {
        match self.stack.last_mut() {
            Some(Frame::Inlines(_, v)) => v.push(inline),
            Some(Frame::Item(_, _, loose)) => loose.push(inline),
            Some(Frame::Code(_, code, _, _)) => code.push_str(&crate::doc::plain(&[inline])),
            Some(Frame::Html(html, _)) => html.push_str(&crate::doc::plain(&[inline])),
            _ => self.block(Block::Paragraph(vec![inline])),
        }
    }

    fn block(&mut self, block: Block) {
        for frame in self.stack.iter_mut().rev() {
            match frame {
                Frame::Blocks(v) | Frame::Quote(_, v) => {
                    v.push(block);
                    return;
                }
                Frame::Item(_, v, loose) => {
                    if !loose.is_empty() {
                        v.push(Block::Paragraph(std::mem::take(loose)));
                    }
                    v.push(block);
                    return;
                }
                _ => continue,
            }
        }
    }

    fn end(&mut self, end: TagEnd) {
        match end {
            TagEnd::TableHead => {
                if let Some(Frame::Table(_, header, _, row)) = self.stack.last_mut() {
                    *header = std::mem::take(row);
                }
            }
            TagEnd::TableRow => {
                if let Some(Frame::Table(_, _, rows, row)) = self.stack.last_mut() {
                    rows.push(std::mem::take(row));
                }
            }
            _ => self.close_top(),
        }
    }

    fn close_top(&mut self) {
        let Some(frame) = self.stack.pop() else {
            return;
        };
        match frame {
            Frame::Blocks(v) => self.stack.push(Frame::Blocks(v)),
            Frame::Quote(None, v) => self.block(Block::Quote(v)),
            Frame::Quote(Some(kind), v) => self.block(Block::Callout { kind, content: v }),
            Frame::List(start, items) => self.block(Block::List { start, items }),
            Frame::Item(task, mut v, loose) => {
                if !loose.is_empty() {
                    v.push(Block::Paragraph(loose));
                }
                if let Some(Frame::List(_, items)) = self.stack.last_mut() {
                    items.push(ListItem { task, content: v });
                }
            }
            Frame::Inlines(kind, v) => match kind {
                Inl::Paragraph => self.block(Block::Paragraph(v)),
                Inl::Heading(level) => self.block(Block::Heading { level, content: v }),
                Inl::Emph => self.inline(Inline::Emph(v)),
                Inl::Strong => self.inline(Inline::Strong(v)),
                Inl::Strike => self.inline(Inline::Strike(v)),
                Inl::Link(target) => self.inline(Inline::Link { target, content: v }),
                Inl::Image(target) => self.inline(Inline::Image {
                    target,
                    alt: crate::doc::plain(&v),
                }),
                Inl::Cell => {
                    if let Some(Frame::Table(_, _, _, row)) = self.stack.last_mut() {
                        row.push(v);
                    }
                }
            },
            Frame::Table(align, header, rows, _) => self.block(Block::Table(Table {
                align,
                header,
                rows,
            })),
            Frame::Code(lang, text, mermaid, line) => {
                if mermaid {
                    self.block(Block::Diagram { source: text, line });
                } else {
                    self.block(Block::Code { lang, text });
                }
            }
            Frame::Html(html, line) => {
                for (i, raw) in html.lines().enumerate() {
                    if raw.trim().is_empty() {
                        continue;
                    }
                    match self
                        .ctx
                        .template
                        .then(|| parse_directive(&self.directive, raw, line + i as u32))
                        .flatten()
                    {
                        Some(d) => self.block(Block::Directive(d)),
                        None => self.block(Block::Paragraph(vec![Inline::Text(raw.to_owned())])),
                    }
                }
            }
        }
    }
}

/// `<!-- kiln:name a b key="v" -->` on one line.
fn parse_directive(re: &Regex, line: &str, number: u32) -> Option<Directive> {
    let caps = re.captures(line)?;
    let mut positional = Vec::new();
    let mut attrs = Vec::new();
    let mut rest = caps[2].trim();
    while !rest.is_empty() {
        let word_end = rest
            .find(|c: char| c.is_whitespace() || c == '=')
            .unwrap_or(rest.len());
        let word = &rest[..word_end];
        let after = rest[word_end..].trim_start();
        if let Some(value) = after.strip_prefix('=') {
            let value = value.trim_start();
            let (v, remaining) = match value.strip_prefix('"') {
                Some(quoted) => match quoted.find('"') {
                    Some(close) => (&quoted[..close], &quoted[close + 1..]),
                    None => (quoted, ""),
                },
                None => {
                    let end = value.find(char::is_whitespace).unwrap_or(value.len());
                    (&value[..end], &value[end..])
                }
            };
            attrs.push((word.to_owned(), v.to_owned()));
            rest = remaining.trim_start();
        } else {
            positional.push(word.to_owned());
            rest = after;
        }
    }
    Some(Directive {
        name: caps[1].to_owned(),
        positional,
        attrs,
        line: number,
    })
}

/// Text with `[[ID]]`, `[[ID|text]]` and bare IDs turned into references.
/// Text that looks like a reference but isn't a valid ID stays text (it
/// is K102 in `kiln check`).
pub fn split_references(text: &str, explicit: &Regex, bare: Option<&Regex>) -> Vec<Inline> {
    let mut found: Vec<(usize, usize, Inline)> = Vec::new();
    for caps in explicit.captures_iter(text) {
        let (Some(whole), Some(id)) = (caps.get(0), caps.get(1)) else {
            continue;
        };
        if let Ok(id) = id.as_str().trim().parse::<ItemId>() {
            let custom = caps
                .get(2)
                .map(|m| m.as_str().trim().to_owned())
                .filter(|s| !s.is_empty());
            found.push((whole.start(), whole.end(), Inline::Ref { id, text: custom }));
        }
    }
    if let Some(bare) = bare {
        for m in bare.find_iter(text) {
            let inside = found
                .iter()
                .any(|&(s, e, _)| m.start() >= s && m.start() < e);
            let before = text[..m.start()].chars().next_back();
            let after = text[m.end()..].chars().next();
            let boundary = !matches!(before, Some(c) if c == '-' || c == '/' || c == '_' || c == '[' || c.is_alphanumeric())
                && !matches!(after, Some(c) if c == '-' || c == '_' || c.is_alphanumeric());
            if inside || !boundary {
                continue;
            }
            if let Ok(id) = m.as_str().parse::<ItemId>() {
                found.push((m.start(), m.end(), Inline::Ref { id, text: None }));
            }
        }
    }
    found.sort_by_key(|(s, _, _)| *s);
    let mut out = Vec::new();
    let mut at = 0;
    for (start, end, inline) in found {
        if start > at {
            out.push(Inline::Text(text[at..start].to_owned()));
        }
        out.push(inline);
        at = end;
    }
    if at < text.len() {
        out.push(Inline::Text(text[at..].to_owned()));
    }
    out
}

/// The path and fragment of a destination naming a repository file; `None`
/// for URLs, e-mail and anchors.
fn relative_path(dest: &str) -> Option<(String, Option<String>)> {
    let has_scheme = dest.split_once(':').is_some_and(|(s, _)| {
        s.len() > 1
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    });
    if dest.is_empty() || dest.starts_with("//") || has_scheme {
        return None;
    }
    let (path, fragment) = match dest.split_once('#') {
        Some((p, f)) => (p, Some(f.to_owned())),
        None => (dest, None),
    };
    let path = path.split('?').next().unwrap_or(path);
    (!path.is_empty()).then(|| (percent_decode(path), fragment))
}

/// `dest` from `dir` as a project-relative path; `/x` is from the root.
/// `None` when it climbs out of the project.
pub fn resolve(dir: &str, dest: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    let (base, dest) = match dest.strip_prefix('/') {
        Some(rest) => ("", rest),
        None => (dir, dest),
    };
    for part in base.split('/').chain(dest.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    Some(parts.join("/"))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Tone;

    fn ctx(template: bool) -> BodyContext<'static> {
        static PREFIXES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        BodyContext {
            dir: "design/req",
            first_line: 10,
            prefixes: PREFIXES.get_or_init(|| vec!["REQ".to_owned(), "ADR".to_owned()]),
            link_bare_ids: true,
            template,
        }
    }

    fn id(s: &str) -> ItemId {
        s.parse().unwrap()
    }

    #[test]
    fn paragraphs_with_references_emphasis_and_code() {
        let blocks = render_markdown(
            "See [[ADR-004]], REQ-002 and [[REQ-001|the parent]], *not* `REQ-099`.\n",
            &ctx(false),
        );
        assert_eq!(
            blocks,
            vec![Block::Paragraph(vec![
                Inline::text("See "),
                Inline::Ref {
                    id: id("ADR-004"),
                    text: None
                },
                Inline::text(", "),
                Inline::Ref {
                    id: id("REQ-002"),
                    text: None
                },
                Inline::text(" and "),
                Inline::Ref {
                    id: id("REQ-001"),
                    text: Some("the parent".into())
                },
                Inline::text(", "),
                Inline::Emph(vec![Inline::text("not")]),
                Inline::text(" "),
                Inline::Code("REQ-099".into()),
                Inline::text("."),
            ])]
        );
    }

    #[test]
    fn diagrams_callouts_tables_lists_and_links() {
        let text = "> [!WARNING]\n> Careful.\n\n```mermaid\nflowchart TD\n  a --> b\n```\n\n| A | B |\n|:--|--:|\n| 1 | [x](../img/a%20b.png#top) |\n\n- [x] done\n- plain\n\n<b>raw</b>\n";
        let blocks = render_markdown(text, &ctx(false));
        assert_eq!(
            blocks[0],
            Block::Callout {
                kind: Callout::Warning,
                content: vec![Block::Paragraph(vec![Inline::text("Careful.")])]
            }
        );
        assert_eq!(
            blocks[1],
            Block::Diagram {
                source: "flowchart TD\n  a --> b\n".into(),
                line: 14
            }
        );
        let Block::Table(table) = &blocks[2] else {
            panic!("{:?}", blocks[2])
        };
        assert_eq!(table.align, [Align::Left, Align::Right]);
        assert_eq!(
            table.header,
            [vec![Inline::text("A")], vec![Inline::text("B")]]
        );
        assert_eq!(
            table.rows[0][1],
            vec![Inline::Link {
                target: Target::Repo {
                    path: "design/img/a b.png".into(),
                    line: None,
                    fragment: Some("top".into())
                },
                content: vec![Inline::text("x")]
            }]
        );
        let Block::List { items, .. } = &blocks[3] else {
            panic!()
        };
        assert_eq!(items[0].task, Some(true));
        assert_eq!(items[1].task, None);
        assert_eq!(
            blocks[4],
            Block::Paragraph(vec![Inline::text("<b>raw</b>")])
        );
        let _ = Tone::Red;
    }

    #[test]
    fn template_directives_and_values() {
        let text = "# Introduction\n\n<!-- kiln:summary -->\n\nThis is <!-- kiln:value name -->.\n\n<!-- kiln:items type=\"requirement\" tag=performance -->\n<!-- kiln:item REQ-014 -->\n";
        let blocks = render_markdown(text, &ctx(true));
        assert_eq!(
            blocks[0],
            Block::Heading {
                level: 1,
                content: vec![Inline::text("Introduction")]
            }
        );
        assert_eq!(
            blocks[1],
            Block::Directive(Directive {
                name: "summary".into(),
                positional: vec![],
                attrs: vec![],
                line: 12
            })
        );
        assert_eq!(
            blocks[2],
            Block::Paragraph(vec![
                Inline::text("This is "),
                Inline::Value("name".into()),
                Inline::text(".")
            ])
        );
        assert_eq!(
            blocks[3],
            Block::Directive(Directive {
                name: "items".into(),
                positional: vec![],
                attrs: vec![
                    ("type".into(), "requirement".into()),
                    ("tag".into(), "performance".into())
                ],
                line: 16
            })
        );
        let Block::Directive(item) = &blocks[4] else {
            panic!()
        };
        assert_eq!(
            (item.positional.as_slice(), item.line),
            (["REQ-014".to_owned()].as_slice(), 17)
        );
        // Outside templates, a directive is just HTML shown as text.
        assert!(matches!(
            &render_markdown("<!-- kiln:toc -->\n", &ctx(false))[0],
            Block::Paragraph(_)
        ));
    }

    #[test]
    fn resolving_relative_paths() {
        assert_eq!(
            resolve("design/req", "../img/a.png").as_deref(),
            Some("design/img/a.png")
        );
        assert_eq!(
            resolve("design", "/docs/x.md").as_deref(),
            Some("docs/x.md")
        );
        assert_eq!(resolve("design", "../../x"), None);
    }
}
