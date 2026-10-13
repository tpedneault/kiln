//! Completion (INTEGRATION.md, "Editing items"): IDs after `[[` in a
//! body, in `parent = "`, and in `[links]` arrays filtered by what the
//! role allows; statuses in `status = "`; types in `type = "`; link
//! roles at the start of a `[links]` line; tags already used. In a
//! Markdown file that isn't an item, only IDs after `[[`.
//!
//! The context is read from the line up to the cursor, not from a parse:
//! the file being typed is usually not valid TOML at that moment.

use std::collections::BTreeSet;

use kiln_index::Project;
use kiln_model::{Item, TypeDef};
use lsp_types::{CompletionItem, CompletionItemKind, CompletionTextEdit, Position, TextEdit};

use super::item_at;
use crate::text::Doc;

/// Completions at `pos` in `doc` (the file `rel` of `project`).
pub fn complete(
    project: &Project,
    rel: Option<&str>,
    doc: &Doc,
    pos: Position,
) -> Vec<CompletionItem> {
    let text = &doc.text;
    let offset = doc.offset(pos);
    let line_start = text[..offset].rfind('\n').map_or(0, |i| i + 1);
    let before = &text[line_start..offset];
    let after = &text[offset..];
    let item = item_at(project, rel);

    let front_matter = match kiln_parse::sections(text) {
        Some(Ok(s)) if offset >= s.front_matter.0 && offset <= s.front_matter.1 => Some(s),
        _ => None,
    };
    let Some(fm) = front_matter else {
        return body(project, doc, before, after, offset, item);
    };
    let type_name = item
        .map(|i| i.type_name.clone())
        .or_else(|| written_type(&text[fm.front_matter.0..fm.front_matter.1]));
    let ty = type_name
        .as_deref()
        .and_then(|t| project.config.type_named(t));
    let table = text[fm.front_matter.0..line_start]
        .lines()
        .map(str::trim)
        .rfind(|l| l.starts_with('['))
        .map(|l| l.trim_matches(|c| c == '[' || c == ']').trim().to_owned());
    let own_id = item.map(|i| i.id.to_string());
    let replace = |partial: &str| (offset - partial.len(), offset);

    let (key, value) = match before.split_once('=') {
        Some((k, v)) => (k.trim(), Some(v)),
        None => (before.trim(), None),
    };
    let open_quote = value.filter(|v| v.matches('"').count() % 2 == 1);
    let partial = open_quote.map(|v| v.rsplit('"').next().unwrap_or(""));

    match (table.as_deref(), partial) {
        (None, Some(partial)) => {
            let (from, to) = replace(partial);
            let words: Vec<(String, String)> = match key {
                "parent" => ids(
                    project,
                    |t| ty.is_some_and(|ty| ty.parents.contains(&t.type_name)),
                    own_id.as_deref(),
                ),
                "status" => ty
                    .map(|ty| {
                        ty.statuses
                            .iter()
                            .map(|s| (s.clone(), status_detail(ty, s)))
                            .collect()
                    })
                    .unwrap_or_default(),
                "type" => project
                    .config
                    .types
                    .iter()
                    .map(|t| (t.name.clone(), format!("{} ({}-…)", t.title, t.prefix)))
                    .collect(),
                "tags" => {
                    let used: BTreeSet<&String> =
                        project.items.iter().flat_map(|i| &i.tags).collect();
                    used.into_iter()
                        .map(|t| (t.clone(), "tag".to_owned()))
                        .collect()
                }
                _ => Vec::new(),
            };
            let kind = if key == "parent" {
                CompletionItemKind::REFERENCE
            } else {
                CompletionItemKind::ENUM_MEMBER
            };
            items(doc, words, from, to, kind)
        }
        (Some("links"), Some(partial)) => {
            let Some(role) = project.config.role(key) else {
                return Vec::new();
            };
            let listed: Vec<&str> = value.unwrap_or("").split('"').skip(1).step_by(2).collect();
            let (from, to) = replace(partial);
            let words = ids(
                project,
                |t| role.allows_to(&t.type_name) && !listed.contains(&t.id.to_string().as_str()),
                own_id.as_deref(),
            );
            items(doc, words, from, to, CompletionItemKind::REFERENCE)
        }
        (Some("links"), None)
            if value.is_none() && key.chars().all(|c| c.is_ascii_lowercase() || c == '-') =>
        {
            let (from, to) = replace(key);
            let words = project
                .config
                .links
                .iter()
                .filter(|r| ty.is_none_or(|ty| r.allows_from(&ty.name)))
                .map(|r| {
                    (
                        r.name.clone(),
                        format!("→ {} (read back: {})", r.to.join(", "), r.inverse),
                    )
                })
                .collect();
            items(doc, words, from, to, CompletionItemKind::PROPERTY)
        }
        _ => Vec::new(),
    }
}

/// `[[` in a body: every ID, closing the brackets unless they are there.
fn body(
    project: &Project,
    doc: &Doc,
    before: &str,
    after: &str,
    offset: usize,
    item: Option<&Item>,
) -> Vec<CompletionItem> {
    let Some(open) = before.rfind("[[") else {
        return Vec::new();
    };
    let partial = &before[open + 2..];
    if !partial
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Vec::new();
    }
    let own = item.map(|i| i.id.to_string());
    let words = ids(project, |_| true, own.as_deref());
    let close = if after.starts_with("]]") { "" } else { "]]" };
    let mut out = items(
        doc,
        words,
        offset - partial.len(),
        offset,
        CompletionItemKind::REFERENCE,
    );
    for c in &mut out {
        if let Some(CompletionTextEdit::Edit(e)) = &mut c.text_edit {
            e.new_text.push_str(close);
        }
    }
    out
}

/// IDs of the items `keep` accepts, without `own`, with title and status.
fn ids(
    project: &Project,
    keep: impl Fn(&Item) -> bool,
    own: Option<&str>,
) -> Vec<(String, String)> {
    let mut seen = BTreeSet::new();
    project
        .items
        .iter()
        .filter(|i| keep(i) && Some(i.id.to_string().as_str()) != own)
        .filter(|i| seen.insert(i.id.clone()))
        .map(|i| (i.id.to_string(), format!("{} ({})", i.title, i.status)))
        .collect()
}

fn status_detail(ty: &TypeDef, status: &str) -> String {
    if ty.is_closed(status) {
        "closed".into()
    } else if ty.is_done(status) {
        "done".into()
    } else if ty.initial == status {
        "initial".into()
    } else {
        ty.title.clone()
    }
}

fn written_type(front_matter: &str) -> Option<String> {
    front_matter.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == "type").then(|| v.trim().trim_matches(['"', '\'']).to_owned())
    })
}

fn items(
    doc: &Doc,
    words: Vec<(String, String)>,
    from: usize,
    to: usize,
    kind: CompletionItemKind,
) -> Vec<CompletionItem> {
    let range = doc.range(from, to);
    words
        .into_iter()
        .enumerate()
        .map(|(n, (word, detail))| CompletionItem {
            label: word.clone(),
            kind: Some(kind),
            // Typing words of a title finds its ID.
            filter_text: Some(format!("{word} {detail}")),
            sort_text: Some(format!("{n:05}")),
            detail: Some(detail),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range,
                new_text: word,
            })),
            ..Default::default()
        })
        .collect()
}
