//! Item files to [`Item`]: the `+++` front matter (K100, K104) and the
//! references and relative links in the body.
//!
//! Only what the file alone can tell is checked here. Whether a status
//! belongs to the type, a link role is allowed or a target exists needs
//! the configuration and the whole project, which is `kiln-check`.

use kiln_model::{Date, Finding, Item, LinkEntry, Mention, Ref, RelativeLink, Rule, Span};
use pulldown_cmark::{Event, Options, Parser, Tag};
use regex::Regex;
use toml_edit::{ImDocument, Item as TomlItem};

use crate::text::LineIndex;
use crate::toml_walk::{inside_quotes, Walk};

/// The front-matter keys, in the order formatting writes them.
pub(crate) const KEYS: &[&str] = &[
    "id", "type", "title", "status", "parent", "tags", "jira", "owner", "due", "links", "extra",
];

/// The longest title, in characters.
pub const MAX_TITLE: usize = 200;

/// What reading an item needs from the project.
pub struct ItemOptions<'a> {
    /// The configured ID prefixes: only `PREFIX-N` with one of these is a
    /// bare reference, so `UTF-8` or a Jira key in prose is not.
    pub prefixes: &'a [String],
    /// `[render] link_bare_ids`.
    pub link_bare_ids: bool,
}

/// Where the front matter is, in a normalised item file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sections {
    /// The TOML between the `+++` lines.
    pub front_matter: (usize, usize),
    /// Everything after the closing `+++` line.
    pub body: (usize, usize),
}

/// `None` when the file isn't an item (its first line isn't `+++`);
/// `Some(Err(()))` when the closing `+++` is missing.
pub fn sections(text: &str) -> Option<Result<Sections, ()>> {
    let first_end = text.find('\n').unwrap_or(text.len());
    if text[..first_end].trim_end() != "+++" {
        return None;
    }
    let fm_start = (first_end + 1).min(text.len());
    let mut pos = fm_start;
    while pos < text.len() {
        let end = text[pos..]
            .find('\n')
            .map(|i| pos + i)
            .unwrap_or(text.len());
        if text[pos..end].trim_end() == "+++" {
            let body_start = (end + 1).min(text.len());
            return Some(Ok(Sections {
                front_matter: (fm_start, pos),
                body: (body_start, text.len()),
            }));
        }
        pos = end + 1;
    }
    Some(Err(()))
}

/// Parses one normalised item file. `None` (with findings) when the file
/// has front matter but no usable `id`; `(None, [])` when it isn't an
/// item at all.
pub fn parse_item(path: &str, text: &str, options: &ItemOptions) -> (Option<Item>, Vec<Finding>) {
    let lines = LineIndex::new(text);
    let mut w = Walk {
        path,
        lines: &lines,
        offset: 0,
        findings: Vec::new(),
    };
    let sections = match sections(text) {
        None => return (None, Vec::new()),
        Some(Err(())) => {
            w.report(
                Rule::K100,
                lines.span(0, 3),
                "front matter has no closing `+++` line",
            );
            return (None, w.findings);
        }
        Some(Ok(s)) => s,
    };
    let (fm_start, fm_end) = sections.front_matter;
    w.offset = fm_start;
    let doc = match ImDocument::parse(&text[fm_start..fm_end]) {
        Ok(doc) => doc,
        Err(e) => {
            let span = w.span(e.span());
            let message = e
                .message()
                .lines()
                .next()
                .unwrap_or("not valid TOML")
                .trim()
                .to_owned();
            w.report(
                Rule::K100,
                span,
                format!("front matter isn't valid TOML: {message}"),
            );
            return (None, w.findings);
        }
    };
    let root = doc.as_table();
    w.unknown_keys(root, KEYS, Rule::K104, "in front matter");

    let quoted = |w: &Walk, span: Span| inside_quotes(text, span, w.lines);
    let required = |w: &mut Walk, key: &str| -> Option<(String, Span)> {
        match root.get(key) {
            Some(item) => w.string(item, Rule::K104, &format!("`{key}`")),
            None => {
                let span = w
                    .lines
                    .span(fm_start.saturating_sub(4), fm_start.saturating_sub(1));
                w.report(Rule::K104, span, format!("front matter needs `{key}`"));
                None
            }
        }
    };

    let id = match required(&mut w, "id") {
        Some((raw, span)) => {
            let span = quoted(&w, span);
            match raw.parse() {
                Ok(id) => Some((id, span)),
                Err(e) => {
                    w.report(Rule::K104, span, format!("id `{raw}`: {e}"));
                    None
                }
            }
        }
        None => None,
    };
    let (type_name, type_span) = required(&mut w, "type").unwrap_or_default();
    let (title, title_span) = required(&mut w, "title").unwrap_or_default();
    if title.contains('\n') {
        w.report(Rule::K104, title_span, "`title` must be one line");
    } else if title.chars().count() > MAX_TITLE {
        w.report(
            Rule::K104,
            title_span,
            format!("`title` is longer than {MAX_TITLE} characters"),
        );
    }
    let (status, status_span) = required(&mut w, "status").unwrap_or_default();

    let parent = root
        .get("parent")
        .and_then(|i| w.string(i, Rule::K104, "`parent`"))
        .map(|(raw, span)| Ref::new(raw, quoted(&w, span)));

    let mut tags = Vec::new();
    if let Some(item) = root.get("tags") {
        for (tag, span) in w.strings(item, Rule::K104, "`tags`").unwrap_or_default() {
            if tag.chars().any(char::is_uppercase) {
                w.report(Rule::K104, span, format!("tag `{tag}` must be lowercase"));
            }
            tags.push(tag);
        }
    }

    let jira = match root.get("jira") {
        None => Vec::new(),
        Some(item) if item.is_str() => vec![item.as_str().unwrap_or_default().to_owned()],
        Some(item) => w
            .strings(item, Rule::K104, "`jira`")
            .unwrap_or_default()
            .into_iter()
            .map(|(s, _)| s)
            .collect(),
    };

    let owner = root
        .get("owner")
        .and_then(|i| w.string(i, Rule::K104, "`owner`"))
        .map(|(s, _)| s);

    let due = root.get("due").and_then(|item| {
        let date = item
            .as_datetime()
            .filter(|d| d.time.is_none() && d.offset.is_none())
            .and_then(|d| d.date);
        if date.is_none() {
            let span = w.item_span(item);
            w.report(Rule::K104, span, "`due` must be a date (due = 2026-11-30)");
        }
        date.map(|d| Date {
            year: i32::from(d.year),
            month: d.month,
            day: d.day,
        })
    });

    let mut links = Vec::new();
    if let Some(table) = root
        .get("links")
        .and_then(|i| w.table(i, Rule::K104, "`links`"))
    {
        for (role, item) in table.iter() {
            let role_span = w.span_of_key(table, role);
            for (raw, span) in w
                .strings(item, Rule::K104, &format!("`links.{role}`"))
                .unwrap_or_default()
            {
                links.push(LinkEntry {
                    role: role.to_owned(),
                    role_span,
                    target: Ref::new(raw, quoted(&w, span)),
                });
            }
        }
    }

    if let Some(item) = root.get("extra") {
        if !matches!(item, TomlItem::Table(_)) && item.as_inline_table().is_none() {
            let span = w.item_span(item);
            w.report(Rule::K104, span, "`extra` must be a table");
        }
    }

    let Some((id, id_span)) = id else {
        return (None, w.findings);
    };
    let (body_start, body_end) = sections.body;
    let (mentions, relative_links, diagrams) =
        body_references(text, body_start, body_end, &lines, options);
    let item = Item {
        path: path.to_owned(),
        id,
        id_span,
        type_name,
        type_span,
        title,
        title_span,
        status,
        status_span,
        parent,
        tags,
        jira,
        owner,
        due,
        links,
        mentions,
        relative_links,
        diagrams,
        body: text[body_start..body_end].to_owned(),
        body_line: lines.span(body_start, body_start).line,
    };
    (Some(item), w.findings)
}

type BodyRefs = (Vec<Mention>, Vec<RelativeLink>, Vec<(String, Span)>);

/// References (`[[ID]]`, `[[ID|text]]`, and bare IDs with a configured
/// prefix when `link_bare_ids`) outside code, relative link and image
/// destinations, and Mermaid blocks.
fn body_references(
    text: &str,
    start: usize,
    end: usize,
    lines: &LineIndex,
    options: &ItemOptions,
) -> BodyRefs {
    let body = &text[start..end];
    let mut code: Vec<(usize, usize)> = Vec::new();
    let mut relative = Vec::new();
    let mut diagrams = Vec::new();
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    for (event, range) in Parser::new_ext(body, opts).into_offset_iter() {
        match event {
            Event::Code(_) | Event::Html(_) | Event::InlineHtml(_) => {
                code.push((range.start, range.end))
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                code.push((range.start, range.end));
                if let pulldown_cmark::CodeBlockKind::Fenced(info) = &kind {
                    if info.split_whitespace().next() == Some("mermaid") {
                        let block = &body[range.clone()];
                        // The source starts on the line after the fence.
                        let first = block.find('\n').map(|i| i + 1).unwrap_or(block.len());
                        let inner = &block[first..];
                        let close = inner.trim_end().rfind('\n').map(|i| i + 1).unwrap_or(0);
                        let fence_closed =
                            inner.trim_end().ends_with("```") || inner.trim_end().ends_with("~~~");
                        let source = if fence_closed { &inner[..close] } else { inner };
                        let at = start + range.start + first;
                        diagrams.push((source.to_owned(), lines.span(at, at + source.len())));
                    }
                }
            }
            Event::Start(Tag::Link { dest_url, .. })
            | Event::Start(Tag::Image { dest_url, .. }) => {
                if let Some(dest) = relative_destination(&dest_url) {
                    let at = body[range.clone()]
                        .rfind(dest_url.as_ref())
                        .map(|i| range.start + i)
                        .unwrap_or(range.start);
                    relative.push(RelativeLink {
                        dest,
                        span: lines.span(start + at, start + at + dest_url.len()),
                    });
                }
            }
            _ => {}
        }
    }
    code.sort_unstable();
    let in_code = |i: usize| {
        let k = code.partition_point(|&(s, _)| s <= i);
        k > 0 && i < code[k - 1].1
    };

    let mut mentions = Vec::new();
    let mut explicit_ranges = Vec::new();
    let explicit = explicit_regex();
    for caps in explicit.captures_iter(body) {
        let (Some(whole), Some(id)) = (caps.get(0), caps.get(1)) else {
            continue;
        };
        if in_code(whole.start()) {
            continue;
        }
        explicit_ranges.push((whole.start(), whole.end()));
        let raw = id.as_str().trim();
        let lead = id.as_str().len() - id.as_str().trim_start().len();
        let at = start + id.start() + lead;
        mentions.push(Mention {
            target: Ref::new(raw, lines.span(at, at + raw.len())),
            explicit: true,
        });
    }
    if options.link_bare_ids && !options.prefixes.is_empty() {
        let bare = bare_regex(options.prefixes);
        for m in bare.find_iter(body) {
            let inside_explicit = explicit_ranges
                .iter()
                .any(|&(s, e)| m.start() >= s && m.start() < e);
            if in_code(m.start()) || inside_explicit || !bare_boundary(body, m.start(), m.end()) {
                continue;
            }
            let at = start + m.start();
            mentions.push(Mention {
                target: Ref::new(m.as_str(), lines.span(at, at + m.len())),
                explicit: false,
            });
        }
    }
    mentions.sort_by_key(|m| m.target.span.start);
    (mentions, relative, diagrams)
}

// Compiled once per process (per prefix set for bare IDs): compiling them
// for every item cost more than parsing the item. `Regex` clones share the
// compiled program.
fn explicit_regex() -> Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\[\[([^\]\|\n]+)(?:\|[^\]\n]*)?\]\]").expect("a valid literal regex")
    })
    .clone()
}

fn bare_regex(prefixes: &[String]) -> Regex {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<Vec<String>, Regex>>> = OnceLock::new();
    let compile = || {
        let alternatives: Vec<String> = prefixes.iter().map(|p| regex::escape(p)).collect();
        Regex::new(&format!(r"\b(?:{})-\d+\b", alternatives.join("|")))
            .expect("prefixes are validated letters and digits")
    };
    match CACHE.get_or_init(|| Mutex::new(HashMap::new())).lock() {
        Ok(mut map) => map.entry(prefixes.to_vec()).or_insert_with(compile).clone(),
        Err(_) => compile(),
    }
}

/// A bare ID is a whole word: not part of `X-REQ-014`, `REQ-014-foo` or a
/// path segment like `REQ-014-title.md`.
fn bare_boundary(body: &str, start: usize, end: usize) -> bool {
    let before = body[..start].chars().next_back();
    let after = body[end..].chars().next();
    !matches!(before, Some(c) if c == '-' || c == '/' || c == '_' || c.is_alphanumeric())
        && !matches!(after, Some(c) if c == '-' || c == '_' || c.is_alphanumeric())
}

/// A destination that names a file in the repository: not a URL, an
/// e-mail address or a bare `#anchor`. Without `#fragment` and `?query`,
/// percent-decoded.
pub fn relative_destination(dest: &str) -> Option<String> {
    let dest = dest.trim();
    let has_scheme = dest.split_once(':').is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && scheme.len() > 1
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    });
    if dest.is_empty() || dest.starts_with('#') || dest.starts_with("//") || has_scheme {
        return None;
    }
    let cut = dest.find(['#', '?']).unwrap_or(dest.len());
    let path = &dest[..cut];
    if path.is_empty() {
        return None;
    }
    Some(percent_decode(path))
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

    fn opts() -> (Vec<String>, bool) {
        (vec!["REQ".to_owned(), "ADR".to_owned()], true)
    }

    fn parse(text: &str) -> (Option<Item>, Vec<Finding>) {
        let (prefixes, bare) = opts();
        parse_item(
            "design/x.md",
            text,
            &ItemOptions {
                prefixes: &prefixes,
                link_bare_ids: bare,
            },
        )
    }

    fn codes(findings: &[Finding]) -> Vec<String> {
        findings
            .iter()
            .map(|f| format!("{}@{}:{}", f.rule, f.line, f.column))
            .collect()
    }

    const GOOD: &str = r#"+++
id = "REQ-014"
type = "requirement"
title = "Flint appears within 50 ms of the hotkey"
status = "approved"
parent = "REQ-002"
tags = ["performance"]
jira = "FLINT-88"
due = 2026-11-30

[links]
mitigates = ["RISK-003"]
+++

Measured on the reference laptop; see [[ADR-004]] and REQ-002, not
`REQ-099` or FLINT-88, and [[REQ-001|the parent]].

```text
REQ-098 in code
```

![diagram](../img/flow%20chart.png) and [site](https://example.com) and [here](#top).
"#;

    #[test]
    fn a_complete_item_parses_with_spans() {
        let (item, findings) = parse(GOOD);
        assert_eq!(findings, vec![]);
        let item = item.unwrap();
        assert_eq!(item.id.to_string(), "REQ-014");
        assert_eq!((item.id_span.line, item.id_span.column), (2, 7));
        assert_eq!(&GOOD[item.id_span.start..item.id_span.end], "REQ-014");
        assert_eq!(item.parent.as_ref().unwrap().raw, "REQ-002");
        assert_eq!(item.jira, ["FLINT-88"]);
        assert_eq!(item.due.unwrap().to_string(), "2026-11-30");
        assert_eq!(item.links[0].role, "mitigates");
        assert_eq!(item.links[0].target.span.line, 12);
        let mentions: Vec<(String, bool, u32)> = item
            .mentions
            .iter()
            .map(|m| (m.target.raw.clone(), m.explicit, m.target.span.line))
            .collect();
        assert_eq!(
            mentions,
            [
                ("ADR-004".into(), true, 15),
                ("REQ-002".into(), false, 15),
                ("REQ-001".into(), true, 16)
            ]
        );
        assert_eq!(item.relative_links.len(), 1);
        assert_eq!(item.relative_links[0].dest, "../img/flow chart.png");
        assert_eq!(item.relative_links[0].span.line, 22);
        assert_eq!(item.body_line, 14);
        assert!(item.body.starts_with("\nMeasured"));
    }

    #[test]
    fn mermaid_blocks_are_kept_with_their_line() {
        let text = "+++\nid = \"REQ-001\"\ntype = \"requirement\"\ntitle = \"t\"\nstatus = \"draft\"\n+++\n\n```mermaid\nflowchart TD\n  a --> b\n```\n";
        let (item, _) = parse(text);
        let item = item.unwrap();
        assert_eq!(item.diagrams.len(), 1);
        assert_eq!(item.diagrams[0].0, "flowchart TD\n  a --> b\n");
        assert_eq!(item.diagrams[0].1.line, 9);
    }

    #[test]
    fn files_without_front_matter_are_not_items() {
        assert_eq!(parse("# Readme\n"), (None, vec![]));
    }

    #[test]
    fn broken_front_matter_is_k100() {
        let (item, findings) = parse("+++\nid = \"REQ-001\"\n");
        assert!(item.is_none());
        assert_eq!(codes(&findings), ["K100@1:1"]);
        let (_, findings) = parse("+++\nid = \n+++\n");
        assert_eq!(codes(&findings), ["K100@2:6"]);
    }

    #[test]
    fn missing_unknown_and_wrong_keys_are_k104() {
        let text = "+++\nid = \"REQ-14\"\ntitle = 3\nstatus = \"draft\"\ncolour = \"red\"\ntags = [\"Perf\"]\ndue = \"soon\"\n+++\n";
        let (item, findings) = parse(text);
        assert!(item.is_none());
        assert_eq!(
            codes(&findings),
            ["K104@5:1", "K104@2:7", "K104@1:1", "K104@3:9", "K104@6:9", "K104@7:7"]
        );
        assert!(
            findings[1].message.contains("write it REQ-014"),
            "{}",
            findings[1].message
        );
    }

    #[test]
    fn bare_ids_need_a_known_prefix_and_word_boundaries() {
        let text = "+++\nid = \"REQ-001\"\ntype = \"requirement\"\ntitle = \"t\"\nstatus = \"draft\"\n+++\nREQ-002 X-REQ-003 REQ-004-x design/REQ-005.md DES-001 REQ-6\n";
        let (item, _) = parse(text);
        let raws: Vec<String> = item
            .unwrap()
            .mentions
            .iter()
            .map(|m| m.target.raw.clone())
            .collect();
        assert_eq!(raws, ["REQ-002", "REQ-6"]);
    }

    #[test]
    fn relative_destinations() {
        assert_eq!(relative_destination("a/b.md#x").as_deref(), Some("a/b.md"));
        assert_eq!(
            relative_destination("/docs/x.md?raw=1").as_deref(),
            Some("/docs/x.md")
        );
        for none in ["https://x", "mailto:a@b", "#top", "//cdn/x", ""] {
            assert_eq!(relative_destination(none), None, "{none}");
        }
        assert_eq!(relative_destination("C:/x").as_deref(), Some("C:/x"));
    }
}
