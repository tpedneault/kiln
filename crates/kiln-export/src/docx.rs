//! The Word back end (EXPORT.md, "`docx`"): one `.docx` per page, written
//! directly as WordprocessingML in a zip (ADR-011). Headings use Word's
//! own Heading 1–6 styles, so its navigation pane and table of contents
//! work; tables, images and diagrams are embedded; section numbers are
//! text, as in every other format. An optional reference `.docx` supplies
//! the styles, theme, numbering and page setup, the way Word users already
//! share a house style.
//!
//! The bytes are deterministic: parts in a fixed order, a fixed zip time,
//! ids counted per document. That is why this isn't built on `docx-rs`,
//! whose ids come from process-wide counters, so the same document
//! rendered twice in one run differs and `export --check` can't hold.
//!
//! Deliberately not here: Word's own list numbering (bullets and numbers
//! are text, so a reference document's numbering definitions can't clash
//! with ours), comments, tracked changes, headers and footers.

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

use kiln_index::FileSource;
use kiln_model::ItemStyle;
use kiln_render::{Align, Block, Inline, Props, Target, Tone};

use crate::document::{Node, Section, SectionKind};
use crate::markdown::{gitlab_slug, FileContext};
use crate::paths::{encode, page_file, relative};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const PIC: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";

/// A4 with 2.54 cm margins, when no reference document says otherwise.
const SECT_PR: &str = r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="708" w:footer="708" w:gutter="0"/></w:sectPr>"#;

/// Pictures wider than this (EMU, 6.25 in) are scaled down to fit the
/// page.
const MAX_WIDTH_EMU: u64 = 5_715_000;
/// EMU per pixel at 96 dpi.
const EMU_PER_PX: u64 = 9525;
/// A reference document's parts are read up to this size, so a hostile
/// one can't exhaust memory.
const MAX_PART: u64 = 32 << 20;
/// One level of list or quote indentation, in twentieths of a point.
const INDENT: u32 = 360;

/// Kiln's styles, each `(styleId, definition)`. With a reference
/// document, those its `styles.xml` lacks are added, so headings keep
/// their outline levels even when Word left them latent.
const STYLES: &[(&str, &str)] = &[
    (
        "Normal",
        r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:after="120" w:line="264" w:lineRule="auto"/></w:pPr></w:style>"#,
    ),
    (
        "Title",
        r#"<w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:pBdr><w:bottom w:val="single" w:sz="6" w:space="4" w:color="D1D9E0"/></w:pBdr><w:spacing w:after="240"/></w:pPr><w:rPr><w:sz w:val="48"/></w:rPr></w:style>"#,
    ),
    (
        "Heading1",
        r#"<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="360" w:after="120"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="36"/></w:rPr></w:style>"#,
    ),
    (
        "Heading2",
        r#"<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="300" w:after="120"/><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:sz w:val="30"/></w:rPr></w:style>"#,
    ),
    (
        "Heading3",
        r#"<w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="80"/><w:outlineLvl w:val="2"/></w:pPr><w:rPr><w:b/><w:sz w:val="26"/></w:rPr></w:style>"#,
    ),
    (
        "Heading4",
        r#"<w:style w:type="paragraph" w:styleId="Heading4"><w:name w:val="heading 4"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="200" w:after="80"/><w:outlineLvl w:val="3"/></w:pPr><w:rPr><w:b/><w:sz w:val="24"/></w:rPr></w:style>"#,
    ),
    (
        "Heading5",
        r#"<w:style w:type="paragraph" w:styleId="Heading5"><w:name w:val="heading 5"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="200" w:after="60"/><w:outlineLvl w:val="4"/></w:pPr><w:rPr><w:b/><w:sz w:val="22"/></w:rPr></w:style>"#,
    ),
    (
        "Heading6",
        r#"<w:style w:type="paragraph" w:styleId="Heading6"><w:name w:val="heading 6"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="200" w:after="60"/><w:outlineLvl w:val="5"/></w:pPr><w:rPr><w:b/><w:i/><w:sz w:val="22"/></w:rPr></w:style>"#,
    ),
    (
        "TOC1",
        r#"<w:style w:type="paragraph" w:styleId="TOC1"><w:name w:val="toc 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:pPr><w:spacing w:after="60"/></w:pPr></w:style>"#,
    ),
    (
        "TOC2",
        r#"<w:style w:type="paragraph" w:styleId="TOC2"><w:name w:val="toc 2"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:pPr><w:spacing w:after="60"/><w:ind w:left="220"/></w:pPr></w:style>"#,
    ),
    (
        "TOC3",
        r#"<w:style w:type="paragraph" w:styleId="TOC3"><w:name w:val="toc 3"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:pPr><w:spacing w:after="60"/><w:ind w:left="440"/></w:pPr></w:style>"#,
    ),
    (
        "KilnCode",
        r#"<w:style w:type="paragraph" w:customStyle="1" w:styleId="KilnCode"><w:name w:val="Kiln Code"/><w:basedOn w:val="Normal"/><w:pPr><w:shd w:val="clear" w:color="auto" w:fill="F6F8FA"/><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas" w:cs="Consolas"/><w:sz w:val="19"/></w:rPr></w:style>"#,
    ),
    (
        "KilnCodeChar",
        r#"<w:style w:type="character" w:customStyle="1" w:styleId="KilnCodeChar"><w:name w:val="Kiln Code Char"/><w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas" w:cs="Consolas"/><w:sz w:val="19"/><w:shd w:val="clear" w:color="auto" w:fill="F6F8FA"/></w:rPr></w:style>"#,
    ),
    (
        "KilnProps",
        r#"<w:style w:type="paragraph" w:customStyle="1" w:styleId="KilnProps"><w:name w:val="Kiln Properties"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:rPr><w:color w:val="59636E"/></w:rPr></w:style>"#,
    ),
    (
        "Hyperlink",
        r#"<w:style w:type="character" w:styleId="Hyperlink"><w:name w:val="Hyperlink"/><w:rPr><w:color w:val="0563C1"/><w:u w:val="single"/></w:rPr></w:style>"#,
    ),
];

const DOC_DEFAULTS: &str = r#"<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:eastAsia="Calibri" w:cs="Calibri"/><w:sz w:val="22"/><w:szCs w:val="22"/><w:lang w:val="en-GB"/></w:rPr></w:rPrDefault><w:pPrDefault/></w:docDefaults>"#;

/// Every file of a Word target: (project-relative path, bytes).
// implements: REQ-002
pub fn render(ctx: &FileContext, files: &dyn FileSource) -> Vec<(String, Vec<u8>)> {
    let reference = ctx
        .target
        .docx
        .reference
        .as_deref()
        .and_then(|path| files.read(path).ok())
        .and_then(|bytes| Reference::read(&bytes));
    let mut out: Vec<(String, Vec<u8>)> = ctx
        .split
        .pages
        .iter()
        .map(|page| {
            let file = page_file(ctx.target, ctx.split, &page.key, "docx");
            let mut w = Writer {
                ctx,
                files,
                file: file.clone(),
                page_key: page.key.clone(),
                base: page.base_depth,
                ids: HashMap::new(),
                rels: Vec::new(),
                media: Vec::new(),
                images: HashMap::new(),
                bookmark: 0,
                drawing: 0,
                top_seen: false,
            };
            let mut body = para(
                &Ctx::default(),
                Some("Title"),
                "",
                &run(&page.title, &Fmt::default()),
            );
            body.push_str(&w.nodes(&page.nodes, &Ctx::default()));
            (file, w.package(&page.title, &body, reference.as_ref()))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// What a reference document lends: its styles (with Kiln's missing ones
/// added), theme, numbering and last section's page setup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    styles: String,
    theme: Option<Vec<u8>>,
    numbering: Option<Vec<u8>>,
    sect_pr: Option<String>,
}

impl Reference {
    /// `None` when `bytes` isn't a Word document with a `styles.xml`;
    /// `kiln check` reports a reference that doesn't exist or isn't a zip
    /// (K112), so this only falls back for a damaged one.
    pub fn read(bytes: &[u8]) -> Option<Reference> {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
        let mut part = |name: &str| -> Option<Vec<u8>> {
            let f = zip.by_name(name).ok()?;
            let mut out = Vec::new();
            f.take(MAX_PART).read_to_end(&mut out).ok()?;
            Some(out)
        };
        let styles = String::from_utf8(part("word/styles.xml")?).ok()?;
        let theme = part("word/theme/theme1.xml");
        let numbering = part("word/numbering.xml");
        let sect_pr = part("word/document.xml")
            .and_then(|d| String::from_utf8(d).ok())
            .and_then(|d| last_sect_pr(&d));
        Some(Reference {
            styles: merge_styles(&styles),
            theme,
            numbering,
            sect_pr,
        })
    }
}

/// The document's last `<w:sectPr>`, without header and footer references
/// (their parts aren't copied, and a dangling one makes Word complain).
fn last_sect_pr(document: &str) -> Option<String> {
    let start = document.rfind("<w:sectPr")?;
    let end = document[start..].find("</w:sectPr>")? + start + "</w:sectPr>".len();
    let mut s = document[start..end].to_owned();
    for tag in ["<w:headerReference", "<w:footerReference"] {
        while let Some(i) = s.find(tag) {
            let Some(j) = s[i..].find("/>") else {
                break;
            };
            s.replace_range(i..i + j + 2, "");
        }
    }
    Some(s)
}

/// `styles` with each of Kiln's styles it doesn't define appended.
fn merge_styles(styles: &str) -> String {
    let Some(close) = styles.rfind("</w:styles>") else {
        return built_in_styles();
    };
    let mut out = styles[..close].to_owned();
    for (id, def) in STYLES {
        if !styles.contains(&format!("w:styleId=\"{id}\"")) {
            out.push_str(def);
        }
    }
    out.push_str(&styles[close..]);
    out
}

fn built_in_styles() -> String {
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:styles xmlns:w=\"{W}\">{DOC_DEFAULTS}"
    );
    for (_, def) in STYLES {
        out.push_str(def);
    }
    out.push_str("</w:styles>");
    out
}

/// A relationship of `word/document.xml`.
struct Rel {
    id: String,
    kind: &'static str,
    target: String,
    external: bool,
}

/// How runs are formatted.
#[derive(Debug, Clone, Default)]
struct Fmt {
    bold: bool,
    italic: bool,
    strike: bool,
    code: bool,
    link: bool,
}

/// Where paragraphs sit: their left indent and the bar a quote or callout
/// draws beside them.
#[derive(Debug, Clone, Default)]
struct Ctx {
    indent: u32,
    bar: Option<&'static str>,
}

impl Ctx {
    fn deeper(&self, bar: Option<&'static str>) -> Ctx {
        Ctx {
            indent: self.indent + INDENT,
            bar: bar.or(self.bar),
        }
    }
}

struct Writer<'a> {
    ctx: &'a FileContext<'a>,
    files: &'a dyn FileSource,
    file: String,
    page_key: String,
    base: u8,
    ids: HashMap<String, usize>,
    rels: Vec<Rel>,
    /// `(word/media/… name, bytes)`.
    media: Vec<(String, Vec<u8>)>,
    /// Repository path → relationship id, so an image used twice is
    /// stored once.
    images: HashMap<String, (String, u64, u64)>,
    bookmark: u32,
    drawing: u32,
    top_seen: bool,
}

impl Writer<'_> {
    fn level(&self, depth: u8) -> usize {
        (depth as i32 - self.base as i32 + 1).max(1) as usize
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

    fn rel(&mut self, kind: &'static str, target: String, external: bool) -> String {
        // rId1–rId4 are the fixed parts (see `package`).
        let id = format!("rId{}", self.rels.len() + 5);
        self.rels.push(Rel {
            id: id.clone(),
            kind,
            target,
            external,
        });
        id
    }

    fn bookmarked(&mut self, name: &str, inner: &str) -> String {
        self.bookmark += 1;
        format!(
            "<w:bookmarkStart w:id=\"{0}\" w:name=\"{1}\"/>{inner}<w:bookmarkEnd w:id=\"{0}\"/>",
            self.bookmark,
            esc(&bookmark(name))
        )
    }

    fn nodes(&mut self, nodes: &[Node], c: &Ctx) -> String {
        nodes
            .iter()
            .map(|n| match n {
                Node::Block(b) => self.block(b, c),
                Node::Section(s) => self.section(s, c),
            })
            .collect()
    }

    fn section(&mut self, s: &Section, c: &Ctx) -> String {
        let id = self.section_id(s);
        let level = self.level(s.depth);
        let mut runs = run(&s.heading_text(), &Fmt::default());
        if let Some(m) = s.marker_inline() {
            runs.push_str(&run(" ", &Fmt::default()));
            runs.push_str(&self.inline(&m, &Fmt::default()));
        }
        let runs = self.bookmarked(&id, &runs);
        // Top-level sections after the first start a page, as the HTML
        // print stylesheet does; the first follows the title.
        let extra = if s.depth == self.base && self.top_seen {
            "<w:pageBreakBefore/>"
        } else {
            ""
        };
        if s.depth == self.base {
            self.top_seen = true;
        }
        let mut out = self.heading(level, extra, &runs, c);
        out.push_str(&self.nodes(&s.nodes, c));
        out
    }

    /// A heading paragraph; past level 6, a bold paragraph.
    fn heading(&self, level: usize, extra: &str, runs: &str, c: &Ctx) -> String {
        if level > 6 {
            let mut p = para(c, None, extra, "");
            let bold = runs
                .replace("<w:r><w:rPr>", "<w:r><w:rPr><w:b/>")
                .replace("<w:r><w:t", "<w:r><w:rPr><w:b/></w:rPr><w:t");
            p.insert_str(p.len() - "</w:p>".len(), &bold);
            p
        } else {
            para(c, Some(&format!("Heading{level}")), extra, runs)
        }
    }

    fn blocks(&mut self, blocks: &[Block], c: &Ctx) -> String {
        blocks.iter().map(|b| self.block(b, c)).collect()
    }

    fn block(&mut self, block: &Block, c: &Ctx) -> String {
        match block {
            Block::Heading { level, content } => {
                let level = self.level(*level);
                let runs = self.inlines(content, &Fmt::default());
                self.heading(level, "", &runs, c)
            }
            Block::Paragraph(content) => {
                let runs = self.inlines(content, &Fmt::default());
                para(c, None, "", &runs)
            }
            Block::List { start, items } => {
                let mut out = String::new();
                for (i, item) in items.iter().enumerate() {
                    let mut lead = match start {
                        Some(s) => format!("{}.\t", s + i as u64),
                        None => "•\t".to_owned(),
                    };
                    match item.task {
                        Some(true) => lead.push_str("☑ "),
                        Some(false) => lead.push_str("☐ "),
                        None => {}
                    }
                    let inner = c.deeper(None);
                    let hang = format!(
                        "<w:tabs><w:tab w:val=\"left\" w:pos=\"{}\"/></w:tabs><w:ind w:left=\"{}\" w:hanging=\"{INDENT}\"/>",
                        inner.indent, inner.indent
                    );
                    let (first, rest) = match item.content.split_first() {
                        Some((Block::Paragraph(p), rest)) => (Some(p), rest),
                        _ => (None, item.content.as_slice()),
                    };
                    let mut runs = run(&lead, &Fmt::default());
                    if let Some(p) = first {
                        runs.push_str(&self.inlines(p, &Fmt::default()));
                    }
                    out.push_str(&para_ind(&inner, &hang, &runs));
                    out.push_str(&self.blocks(rest, &inner));
                }
                out
            }
            Block::Quote(blocks) => self.blocks(blocks, &c.deeper(Some("D1D9E0"))),
            Block::Callout { kind, content } => {
                let color = callout_color(kind.name());
                let inner = c.deeper(Some(color));
                let label = run(
                    &callout_title(kind.name()),
                    &Fmt {
                        bold: true,
                        ..Fmt::default()
                    },
                );
                let mut out = para(&inner, None, "<w:keepNext/>", &label);
                out.push_str(&self.blocks(content, &inner));
                out
            }
            Block::Code { text, .. } => code_block(text, c),
            Block::Diagram { source, .. } => match self.ctx.pictures.diagram(source) {
                Ok(p) => {
                    let (w, h) = png_size(&p.png).unwrap_or((800, 600));
                    let scale = crate::pictures::SCALE as u64;
                    let rid = self.media_rel(&p.name, p.png);
                    let drawing = self.drawing(&rid, w.max(1) / scale.max(1), h.max(1) / scale.max(1), "Diagram");
                    para(c, None, "<w:jc w:val=\"center\"/>", &drawing)
                }
                Err(e) => {
                    let inner = c.deeper(Some(callout_color("WARNING")));
                    let mut out = para(
                        &inner,
                        None,
                        "",
                        &run(
                            &format!("This diagram couldn't be drawn: {e}"),
                            &Fmt::default(),
                        ),
                    );
                    out.push_str(&code_block(source, c));
                    out
                }
            },
            Block::Table(t) => {
                let header: Vec<String> = t
                    .header
                    .iter()
                    .map(|h| {
                        self.inlines(
                            h,
                            &Fmt {
                                bold: true,
                                ..Fmt::default()
                            },
                        )
                    })
                    .collect();
                let rows: Vec<Vec<String>> = t
                    .rows
                    .iter()
                    .map(|r| r.iter().map(|cell| self.inlines(cell, &Fmt::default())).collect())
                    .collect();
                table(&header, &rows, &t.align, c)
            }
            Block::Rule => para(
                c,
                None,
                "<w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"D1D9E0\"/></w:pBdr>",
                "",
            ),
            Block::Directive(_) => String::new(),
            Block::Toc => self.toc(c),
            Block::Props(p) => self.props(p, c),
            Block::ChildPages => {
                let children: Vec<(String, String)> = self
                    .ctx
                    .split
                    .children(&self.page_key)
                    .iter()
                    .map(|p| {
                        let target = encode(&relative(
                            &self.file,
                            &page_file(self.ctx.target, self.ctx.split, &p.key, "docx"),
                        ));
                        (target, p.title.clone())
                    })
                    .collect();
                let mut out = String::new();
                for (target, title) in children {
                    let rid = self.rel("hyperlink", target, true);
                    let link = format!(
                        "<w:hyperlink r:id=\"{rid}\" w:history=\"1\">{}</w:hyperlink>",
                        run(&title, &Fmt { link: true, ..Fmt::default() })
                    );
                    let inner = c.deeper(None);
                    let hang = format!(
                        "<w:tabs><w:tab w:val=\"left\" w:pos=\"{0}\"/></w:tabs><w:ind w:left=\"{0}\" w:hanging=\"{INDENT}\"/>",
                        inner.indent
                    );
                    out.push_str(&para_ind(&inner, &hang, &format!("{}{link}", run("•\t", &Fmt::default()))));
                }
                out
            }
        }
    }

    fn props(&mut self, props: &Props, c: &Ctx) -> String {
        let status = self.inline(&props.status, &Fmt::default());
        match props.style {
            ItemStyle::None => String::new(),
            ItemStyle::List => {
                let bold = Fmt {
                    bold: true,
                    ..Fmt::default()
                };
                let mut runs = status;
                for e in &props.entries {
                    runs.push_str("<w:r><w:br/></w:r>");
                    runs.push_str(&run(&format!("{}: ", e.label), &bold));
                    for (i, v) in e.values.iter().enumerate() {
                        if i > 0 {
                            runs.push_str(&run(", ", &Fmt::default()));
                        }
                        runs.push_str(&self.inlines(v, &Fmt::default()));
                    }
                }
                para(c, Some("KilnProps"), "", &runs)
            }
            ItemStyle::Line => {
                let mut runs = status;
                for e in &props.entries {
                    runs.push_str(&run(&format!(" · {} ", e.lead), &Fmt::default()));
                    for (i, v) in e.values.iter().enumerate() {
                        if i > 0 {
                            runs.push_str(&run(", ", &Fmt::default()));
                        }
                        runs.push_str(&self.inlines(v, &Fmt::default()));
                    }
                }
                para(c, Some("KilnProps"), "", &runs)
            }
            ItemStyle::Table => {
                let bold = Fmt {
                    bold: true,
                    ..Fmt::default()
                };
                let mut rows = vec![vec![run("Status", &bold), status]];
                for e in &props.entries {
                    let mut values = String::new();
                    for (i, v) in e.values.iter().enumerate() {
                        if i > 0 {
                            values.push_str(&run(", ", &Fmt::default()));
                        }
                        values.push_str(&self.inlines(v, &Fmt::default()));
                    }
                    rows.push(vec![run(&e.label, &bold), values]);
                }
                table(&[], &rows, &[], c)
            }
        }
    }

    /// The table of contents: a Word TOC field (headings 1–3, as links)
    /// whose result is filled in already, so the document reads the same
    /// before Word updates it; `updateFields` in the settings asks Word to
    /// add the page numbers on opening.
    fn toc(&mut self, c: &Ctx) -> String {
        let Some(page) = self.ctx.split.page(&self.page_key) else {
            return String::new();
        };
        // Ids are handed out in document order, so compute them the same
        // way as the page will.
        let mut ids = HashMap::new();
        fn walk(
            w: &mut Writer,
            ids: &mut HashMap<String, usize>,
            nodes: &[Node],
            depth: usize,
            out: &mut Vec<(usize, String, String)>,
        ) {
            for n in nodes {
                if let Node::Section(s) = n {
                    std::mem::swap(&mut w.ids, ids);
                    let id = w.section_id(s);
                    std::mem::swap(&mut w.ids, ids);
                    if depth <= 2 {
                        out.push((depth, id, s.heading_text()));
                    }
                    walk(w, ids, &s.nodes, depth + 1, out);
                }
            }
        }
        let mut entries = Vec::new();
        walk(self, &mut ids, &page.nodes, 0, &mut entries);
        let count = entries.len();
        let mut out = String::new();
        for (i, (depth, id, text)) in entries.into_iter().enumerate() {
            let mut runs = String::new();
            if i == 0 {
                runs.push_str("<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText xml:space=\"preserve\"> TOC \\o \"1-3\" \\h \\z \\u </w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>");
            }
            runs.push_str(&format!(
                "<w:hyperlink w:anchor=\"{}\" w:history=\"1\">{}</w:hyperlink>",
                esc(&bookmark(&id)),
                run(
                    &text,
                    &Fmt {
                        link: true,
                        ..Fmt::default()
                    }
                )
            ));
            if i + 1 == count {
                runs.push_str("<w:r><w:fldChar w:fldCharType=\"end\"/></w:r>");
            }
            out.push_str(&para(c, Some(&format!("TOC{}", depth + 1)), "", &runs));
        }
        out
    }

    fn inlines(&mut self, inlines: &[Inline], f: &Fmt) -> String {
        inlines.iter().map(|i| self.inline(i, f)).collect()
    }

    fn inline(&mut self, inline: &Inline, f: &Fmt) -> String {
        match inline {
            Inline::Text(s) => run(s, f),
            Inline::Code(s) => run(
                s,
                &Fmt {
                    code: true,
                    ..f.clone()
                },
            ),
            Inline::Emph(c) => self.inlines(
                c,
                &Fmt {
                    italic: true,
                    ..f.clone()
                },
            ),
            Inline::Strong(c) => self.inlines(
                c,
                &Fmt {
                    bold: true,
                    ..f.clone()
                },
            ),
            Inline::Strike(c) => self.inlines(
                c,
                &Fmt {
                    strike: true,
                    ..f.clone()
                },
            ),
            Inline::Link { target, content } => {
                let linked = Fmt {
                    link: true,
                    ..f.clone()
                };
                match self.link(target) {
                    Some(Link::Anchor(a)) => format!(
                        "<w:hyperlink w:anchor=\"{}\" w:history=\"1\">{}</w:hyperlink>",
                        esc(&a),
                        self.inlines(content, &linked)
                    ),
                    Some(Link::External(url)) => {
                        let rid = self.rel("hyperlink", url, true);
                        format!(
                            "<w:hyperlink r:id=\"{rid}\" w:history=\"1\">{}</w:hyperlink>",
                            self.inlines(content, &linked)
                        )
                    }
                    None => self.inlines(content, f),
                }
            }
            Inline::Image { target, alt } => self.image(target, alt).unwrap_or_else(|| run(alt, f)),
            Inline::Ref { id, text } => run(text.as_deref().unwrap_or(&id.to_string()), f),
            Inline::Status { text, tone } => format!(
                "<w:r><w:rPr><w:b/><w:caps/><w:color w:val=\"FFFFFF\"/><w:sz w:val=\"16\"/><w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/></w:rPr><w:t xml:space=\"preserve\">\u{a0}{}\u{a0}</w:t></w:r>",
                tone_color(*tone),
                esc(text)
            ),
            Inline::Jira(key) => match self.ctx.jira_url {
                Some(url) => {
                    let url = format!("{}/browse/{key}", url.trim_end_matches('/'));
                    let rid = self.rel("hyperlink", url, true);
                    format!(
                        "<w:hyperlink r:id=\"{rid}\" w:history=\"1\">{}</w:hyperlink>",
                        run(key, &Fmt { link: true, ..f.clone() })
                    )
                }
                None => run(key, f),
            },
            Inline::Value(_) => String::new(),
            Inline::Break => "<w:r><w:br/></w:r>".to_owned(),
            Inline::Anchor(a) => self.bookmarked(a, ""),
        }
    }

    fn link(&self, target: &Target) -> Option<Link> {
        match target {
            Target::Url(u) => Some(Link::External(u.clone())),
            Target::Repo {
                path,
                line,
                fragment,
            } => {
                let mut url = self.ctx.repo.url(&self.file, path, *line)?;
                if let (Some(f), None) = (fragment, line) {
                    url.push_str(&format!("#{f}"));
                }
                Some(Link::External(url))
            }
            Target::Anchor(a) => Some(Link::Anchor(bookmark(a))),
            Target::Page {
                export,
                key,
                anchor,
            } => {
                let file = self.ctx.file_of(export.as_deref(), key, "docx")?;
                // Bookmark names are Word's; other formats keep the ID.
                let anchor = anchor.as_deref().map(|a| {
                    if file.ends_with(".docx") {
                        bookmark(a)
                    } else {
                        a.to_owned()
                    }
                });
                if file == self.file {
                    return anchor.map(Link::Anchor);
                }
                let mut url = encode(&relative(&self.file, &file));
                if let Some(a) = anchor {
                    url.push_str(&format!("#{a}"));
                }
                Some(Link::External(url))
            }
        }
    }

    fn media_rel(&mut self, name: &str, bytes: Vec<u8>) -> String {
        let name = format!("{}-{name}", self.media.len() + 1);
        self.media.push((name.clone(), bytes));
        self.rel("image", format!("media/{name}"), false)
    }

    /// A repository image, embedded; `None` (its alt text instead) for a
    /// remote one, which is never fetched, or one Word can't show.
    fn image(&mut self, target: &Target, alt: &str) -> Option<String> {
        let Target::Repo { path, .. } = target else {
            return None;
        };
        let (rid, w, h) = match self.images.get(path) {
            Some(v) => v.clone(),
            None => {
                let bytes = self.files.read(path).ok()?;
                let ext = path.rsplit('.').next()?.to_ascii_lowercase();
                let (w, h) = match ext.as_str() {
                    "png" => png_size(&bytes)?,
                    "jpg" | "jpeg" => jpeg_size(&bytes)?,
                    "gif" => gif_size(&bytes)?,
                    _ => return None,
                };
                let file = path.rsplit('/').next().unwrap_or("image");
                let rid = self.media_rel(&media_name(file), bytes);
                self.images.insert(path.clone(), (rid.clone(), w, h));
                (rid, w, h)
            }
        };
        Some(self.drawing(&rid, w, h, alt))
    }

    /// An inline picture `w`×`h` pixels at 96 dpi, scaled down to the
    /// page's width.
    fn drawing(&mut self, rid: &str, w: u64, h: u64, alt: &str) -> String {
        self.drawing += 1;
        let n = self.drawing;
        let (mut cx, mut cy) = (w * EMU_PER_PX, h * EMU_PER_PX);
        if cx > MAX_WIDTH_EMU {
            cy = cy * MAX_WIDTH_EMU / cx;
            cx = MAX_WIDTH_EMU;
        }
        format!(
            "<w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:docPr id=\"{n}\" name=\"Picture {n}\" descr=\"{alt}\"/><wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect=\"1\"/></wp:cNvGraphicFramePr><a:graphic><a:graphicData uri=\"{PIC}\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"{n}\" name=\"Picture {n}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"{rid}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>",
            alt = esc(alt)
        )
    }

    /// The `.docx` bytes: every part in a fixed order, stored at a fixed
    /// time, so the same document is the same bytes.
    fn package(&self, title: &str, body: &str, reference: Option<&Reference>) -> Vec<u8> {
        let sect_pr = reference
            .and_then(|r| r.sect_pr.as_deref())
            .unwrap_or(SECT_PR);
        let document = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document xmlns:w=\"{W}\" xmlns:r=\"{R}\" xmlns:wp=\"{WP}\" xmlns:a=\"{A}\" xmlns:pic=\"{PIC}\"><w:body>{body}{sect_pr}</w:body></w:document>"
        );
        let styles = match reference {
            Some(r) => r.styles.clone(),
            None => built_in_styles(),
        };
        let theme = reference.and_then(|r| r.theme.clone());
        let numbering = reference.and_then(|r| r.numbering.clone());

        let mut rels = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{REL}/styles\" Target=\"styles.xml\"/><Relationship Id=\"rId2\" Type=\"{REL}/settings\" Target=\"settings.xml\"/>"
        );
        if theme.is_some() {
            rels.push_str(&format!(
                "<Relationship Id=\"rId3\" Type=\"{REL}/theme\" Target=\"theme/theme1.xml\"/>"
            ));
        }
        if numbering.is_some() {
            rels.push_str(&format!(
                "<Relationship Id=\"rId4\" Type=\"{REL}/numbering\" Target=\"numbering.xml\"/>"
            ));
        }
        for r in &self.rels {
            let mode = if r.external {
                " TargetMode=\"External\""
            } else {
                ""
            };
            rels.push_str(&format!(
                "<Relationship Id=\"{}\" Type=\"{REL}/{}\" Target=\"{}\"{mode}/>",
                r.id,
                r.kind,
                esc(&r.target)
            ));
        }
        rels.push_str("</Relationships>");

        let mut types = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"png\" ContentType=\"image/png\"/><Default Extension=\"jpeg\" ContentType=\"image/jpeg\"/><Default Extension=\"jpg\" ContentType=\"image/jpeg\"/><Default Extension=\"gif\" ContentType=\"image/gif\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/><Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/><Override PartName=\"/word/settings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml\"/>");
        if theme.is_some() {
            types.push_str("<Override PartName=\"/word/theme/theme1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/>");
        }
        if numbering.is_some() {
            types.push_str("<Override PartName=\"/word/numbering.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml\"/>");
        }
        types.push_str("<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/><Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/></Types>");

        let root_rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/><Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties\" Target=\"docProps/app.xml\"/></Relationships>";
        // No dates: they would change the bytes on every export.
        let core = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>{}</dc:title></cp:coreProperties>",
            esc(title)
        );
        let app = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\"><Application>Kiln</Application></Properties>";
        let settings = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:settings xmlns:w=\"{W}\"><w:updateFields w:val=\"true\"/><w:defaultTabStop w:val=\"720\"/><w:compat><w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" w:val=\"15\"/></w:compat></w:settings>"
        );

        let mut parts: Vec<(String, Vec<u8>)> = vec![
            ("[Content_Types].xml".into(), types.into_bytes()),
            ("_rels/.rels".into(), root_rels.as_bytes().to_vec()),
            ("docProps/core.xml".into(), core.into_bytes()),
            ("docProps/app.xml".into(), app.as_bytes().to_vec()),
            ("word/document.xml".into(), document.into_bytes()),
            ("word/_rels/document.xml.rels".into(), rels.into_bytes()),
            ("word/styles.xml".into(), styles.into_bytes()),
            ("word/settings.xml".into(), settings.into_bytes()),
        ];
        if let Some(t) = theme {
            parts.push(("word/theme/theme1.xml".into(), t));
        }
        if let Some(n) = numbering {
            parts.push(("word/numbering.xml".into(), n));
        }
        for (name, bytes) in &self.media {
            parts.push((format!("word/media/{name}"), bytes.clone()));
        }
        zip_parts(&parts)
    }
}

enum Link {
    Anchor(String),
    External(String),
}

/// Writes `parts` as a zip at a fixed time. Writing to memory with unique
/// names can't fail; if the zip library ever did, an empty file is
/// written and Word reports it rather than Kiln panicking.
fn zip_parts(parts: &[(String, Vec<u8>)]) -> Vec<u8> {
    let write = || -> zip::result::ZipResult<Vec<u8>> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default());
        for (name, bytes) in parts {
            zip.start_file(name.as_str(), options)?;
            zip.write_all(bytes)?;
        }
        Ok(zip.finish()?.into_inner())
    };
    write().unwrap_or_default()
}

/// A paragraph: `style`, the context's indent and bar, `extra` paragraph
/// properties, then `runs`.
fn para(c: &Ctx, style: Option<&str>, extra: &str, runs: &str) -> String {
    let ind = if c.indent > 0 {
        format!("<w:ind w:left=\"{}\"/>", c.indent)
    } else {
        String::new()
    };
    para_props(c, style, &format!("{extra}{ind}"), runs)
}

/// A paragraph whose indentation `ind` sets itself (list items).
fn para_ind(c: &Ctx, ind: &str, runs: &str) -> String {
    para_props(c, None, ind, runs)
}

fn para_props(c: &Ctx, style: Option<&str>, props: &str, runs: &str) -> String {
    // Element order inside <w:pPr> is fixed by the schema: style,
    // keepNext, pageBreakBefore, borders, shading, tabs, spacing, indent,
    // justification.
    let mut ppr = String::new();
    if let Some(s) = style {
        ppr.push_str(&format!("<w:pStyle w:val=\"{}\"/>", esc(s)));
    }
    let mut ordered = props.to_owned();
    if let Some(bar) = c.bar {
        let border = format!(
            "<w:pBdr><w:left w:val=\"single\" w:sz=\"18\" w:space=\"8\" w:color=\"{bar}\"/></w:pBdr>"
        );
        // Before tabs/indent/jc, after keepNext and pageBreakBefore.
        let at = ["<w:tabs>", "<w:ind ", "<w:jc "]
            .iter()
            .filter_map(|t| ordered.find(t))
            .min()
            .unwrap_or(ordered.len());
        ordered.insert_str(at, &border);
    }
    ppr.push_str(&ordered);
    if ppr.is_empty() {
        format!("<w:p>{runs}</w:p>")
    } else {
        format!("<w:p><w:pPr>{ppr}</w:pPr>{runs}</w:p>")
    }
}

/// Runs of `text`: tabs and line breaks as Word's own.
fn run(text: &str, f: &Fmt) -> String {
    let mut rpr = String::new();
    if f.code {
        rpr.push_str("<w:rStyle w:val=\"KilnCodeChar\"/>");
    } else if f.link {
        rpr.push_str("<w:rStyle w:val=\"Hyperlink\"/>");
    }
    if f.bold {
        rpr.push_str("<w:b/>");
    }
    if f.italic {
        rpr.push_str("<w:i/>");
    }
    if f.strike {
        rpr.push_str("<w:strike/>");
    }
    let rpr = if rpr.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{rpr}</w:rPr>")
    };
    let mut content = String::new();
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            content.push_str("<w:br/>");
        }
        for (j, piece) in line.split('\t').enumerate() {
            if j > 0 {
                content.push_str("<w:tab/>");
            }
            if !piece.is_empty() {
                content.push_str(&format!("<w:t xml:space=\"preserve\">{}</w:t>", esc(piece)));
            }
        }
    }
    if content.is_empty() {
        return String::new();
    }
    format!("<w:r>{rpr}{content}</w:r>")
}

/// A code block: a paragraph per line, so Word's line spacing applies.
fn code_block(text: &str, c: &Ctx) -> String {
    let text = text.strip_suffix('\n').unwrap_or(text);
    text.split('\n')
        .map(|line| para(c, Some("KilnCode"), "", &run(line, &Fmt::default())))
        .collect()
}

/// A table with borders of its own (a reference document's table styles
/// can't be assumed), the header row repeated across pages. An empty
/// paragraph follows, so two tables in a row don't merge.
fn table(header: &[String], rows: &[Vec<String>], align: &[Align], c: &Ctx) -> String {
    let cols = header
        .len()
        .max(rows.iter().map(Vec::len).max().unwrap_or(0))
        .max(1);
    let width = 9000 / cols;
    let border = |side: &str| {
        format!("<w:{side} w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"D1D9E0\"/>")
    };
    let mut out = format!(
        "<w:tbl><w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/>{}<w:tblBorders>{}{}{}{}{}{}</w:tblBorders><w:tblLayout w:type=\"autofit\"/><w:tblCellMar><w:top w:w=\"40\" w:type=\"dxa\"/><w:left w:w=\"100\" w:type=\"dxa\"/><w:bottom w:w=\"40\" w:type=\"dxa\"/><w:right w:w=\"100\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr><w:tblGrid>",
        if c.indent > 0 {
            format!("<w:tblInd w:w=\"{}\" w:type=\"dxa\"/>", c.indent)
        } else {
            String::new()
        },
        border("top"),
        border("left"),
        border("bottom"),
        border("right"),
        border("insideH"),
        border("insideV")
    );
    for _ in 0..cols {
        out.push_str(&format!("<w:gridCol w:w=\"{width}\"/>"));
    }
    out.push_str("</w:tblGrid>");
    let cell = |runs: &str, i: usize, head: bool| {
        let shd = if head {
            "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"F6F8FA\"/>"
        } else {
            ""
        };
        let jc = match align.get(i) {
            Some(Align::Center) => "<w:jc w:val=\"center\"/>",
            Some(Align::Right) => "<w:jc w:val=\"right\"/>",
            _ => "",
        };
        format!(
            "<w:tc><w:tcPr><w:tcW w:w=\"0\" w:type=\"auto\"/>{shd}</w:tcPr><w:p><w:pPr><w:spacing w:after=\"0\"/>{jc}</w:pPr>{runs}</w:p></w:tc>"
        )
    };
    let row = |cells: &[String], head: bool| {
        let mut r = String::from("<w:tr>");
        if head {
            r.push_str("<w:trPr><w:tblHeader/></w:trPr>");
        }
        for i in 0..cols {
            r.push_str(&cell(
                cells.get(i).map(String::as_str).unwrap_or(""),
                i,
                head,
            ));
        }
        r.push_str("</w:tr>");
        r
    };
    if !header.is_empty() {
        out.push_str(&row(header, true));
    }
    for r in rows {
        out.push_str(&row(r, false));
    }
    out.push_str("</w:tbl><w:p/>");
    out
}

/// A Word bookmark name for an anchor: letters, digits and `_`, starting
/// with a letter, at most 40 characters (Word drops anything else).
/// `REQ-014` becomes `REQ_014`.
pub fn bookmark(anchor: &str) -> String {
    let mut s: String = anchor
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if !s.starts_with(|c: char| c.is_ascii_alphabetic()) {
        s.insert_str(0, "k_");
    }
    s.truncate(40);
    s
}

/// A media file name Word accepts: the original's letters and digits.
fn media_name(file: &str) -> String {
    file.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn callout_title(kind: &str) -> String {
    let mut c = kind.to_lowercase();
    if let Some(first) = c.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    c
}

fn callout_color(kind: &str) -> &'static str {
    match kind {
        "TIP" => "1A7F37",
        "WARNING" => "9A6700",
        "CAUTION" => "CF222E",
        _ => "0969DA",
    }
}

fn tone_color(tone: Tone) -> &'static str {
    match tone {
        Tone::Grey => "6E7781",
        Tone::Blue => "0969DA",
        Tone::Green => "1A7F37",
        Tone::Yellow => "9A6700",
        Tone::Red => "CF222E",
    }
}

/// A PNG's size in pixels, from its header.
fn png_size(b: &[u8]) -> Option<(u64, u64)> {
    if b.len() < 24 || &b[..8] != b"\x89PNG\r\n\x1a\n" || &b[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(b[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(b[20..24].try_into().ok()?);
    Some((u64::from(w), u64::from(h)))
}

/// A GIF's size in pixels, from its header.
fn gif_size(b: &[u8]) -> Option<(u64, u64)> {
    if b.len() < 10 || !b.starts_with(b"GIF8") {
        return None;
    }
    let w = u16::from_le_bytes([b[6], b[7]]);
    let h = u16::from_le_bytes([b[8], b[9]]);
    Some((u64::from(w), u64::from(h)))
}

/// A JPEG's size in pixels, from its first start-of-frame marker.
fn jpeg_size(b: &[u8]) -> Option<(u64, u64)> {
    if b.len() < 4 || b[0] != 0xFF || b[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 9 < b.len() {
        if b[i] != 0xFF {
            return None;
        }
        let marker = b[i + 1];
        if marker == 0xFF {
            i += 1;
            continue;
        }
        let len = usize::from(u16::from_be_bytes([b[i + 2], b[i + 3]]));
        // SOF0–SOF15, except DHT (C4), JPG (C8) and DAC (CC).
        if (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker) {
            let h = u16::from_be_bytes([b[i + 5], b[i + 6]]);
            let w = u16::from_be_bytes([b[i + 7], b[i + 8]]);
            return Some((u64::from(w), u64::from(h)));
        }
        i += 2 + len;
    }
    None
}

/// XML-escapes text and attribute values, dropping the control
/// characters XML can't hold at all.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bookmarks_are_names_word_keeps() {
        assert_eq!(bookmark("REQ-014"), "REQ_014");
        assert_eq!(bookmark("2-scope"), "k_2_scope");
        assert_eq!(bookmark(&"a".repeat(50)).len(), 40);
    }

    #[test]
    fn escaping_drops_what_xml_cannot_hold() {
        assert_eq!(esc("a<b>&\"c\u{1}"), "a&lt;b&gt;&amp;&quot;c");
    }

    #[test]
    fn image_sizes() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend(300u32.to_be_bytes());
        png.extend(200u32.to_be_bytes());
        assert_eq!(png_size(&png), Some((300, 200)));
        assert_eq!(gif_size(b"GIF89a\x0a\x00\x14\x00"), Some((10, 20)));
        let jpeg = [
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00,
            0x40, 0x00, 0x80, 0x03, 0x00, 0x00,
        ];
        assert_eq!(jpeg_size(&jpeg), Some((128, 64)));
        assert_eq!(png_size(b"nope"), None);
        assert_eq!(jpeg_size(&[0xFF, 0xD8, 0x00]), None);
    }

    #[test]
    fn a_reference_keeps_its_styles_and_gains_the_missing_ones() {
        let styles = format!(
            "<?xml version=\"1.0\"?><w:styles xmlns:w=\"{W}\"><w:style w:type=\"paragraph\" w:styleId=\"Heading1\"><w:name w:val=\"heading 1\"/><w:rPr><w:color w:val=\"FF0000\"/></w:rPr></w:style></w:styles>"
        );
        let merged = merge_styles(&styles);
        assert!(merged.contains("FF0000"));
        assert_eq!(merged.matches("w:styleId=\"Heading1\"").count(), 1);
        assert!(merged.contains("w:styleId=\"Heading4\""));
        assert!(merged.ends_with("</w:styles>"));
    }

    #[test]
    fn page_setup_comes_without_headers() {
        let doc = "<w:body><w:p/><w:sectPr><w:headerReference w:type=\"default\" r:id=\"rId9\"/><w:pgSz w:w=\"12240\" w:h=\"15840\"/></w:sectPr></w:body>";
        assert_eq!(
            last_sect_pr(doc).as_deref(),
            Some("<w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/></w:sectPr>")
        );
    }

    #[test]
    fn runs_keep_tabs_and_breaks() {
        assert_eq!(
            run("a\tb\nc", &Fmt::default()),
            "<w:r><w:t xml:space=\"preserve\">a</w:t><w:tab/><w:t xml:space=\"preserve\">b</w:t><w:br/><w:t xml:space=\"preserve\">c</w:t></w:r>"
        );
        assert_eq!(run("", &Fmt::default()), "");
    }
}
