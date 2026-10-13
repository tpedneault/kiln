//! What each request computes, as functions of the world and a position:
//! no protocol plumbing (that is `server.rs`), so each is tested on its
//! own text.

pub mod actions;
pub mod complete;
pub mod hover;
pub mod navigate;

use kiln_index::Project;
use kiln_model::{Item, ItemId};
use lsp_types::TextEdit;

use crate::text::Doc;

/// The ID written around byte `offset`: its byte range and value. Only
/// `PREFIX-NNN` with a configured prefix counts, as in a body.
pub fn id_at(text: &str, offset: usize, prefixes: &[String]) -> Option<(usize, usize, ItemId)> {
    let is_id_char = |c: char| c.is_ascii_alphanumeric() || c == '-';
    let offset = offset.min(text.len());
    if !text.is_char_boundary(offset) {
        return None;
    }
    let start = text[..offset]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_id_char(*c))
        .last()
        .map_or(offset, |(i, _)| i);
    let end = text[offset..]
        .char_indices()
        .find(|(_, c)| !is_id_char(*c))
        .map_or(text.len(), |(i, _)| offset + i);
    let word = &text[start..end];
    let id: ItemId = word.parse().ok()?;
    prefixes
        .iter()
        .any(|p| p == id.prefix())
        .then_some((start, end, id))
}

/// The item whose file is `rel`.
pub fn item_at<'p>(project: &'p Project, rel: Option<&str>) -> Option<&'p Item> {
    let rel = rel?;
    project.items.iter().find(|i| i.path == rel)
}

/// One edit turning `old` into `new`: the changed middle only, so the
/// editor keeps the cursor and undo history sensible.
pub fn edit_between(doc: &Doc, old: &str, new: &str) -> Option<TextEdit> {
    if old == new {
        return None;
    }
    let mut prefix = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(prefix) || !new.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let max_suffix = old.len().min(new.len()) - prefix;
    let mut suffix = old
        .bytes()
        .rev()
        .zip(new.bytes().rev())
        .take(max_suffix)
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(old.len() - suffix) || !new.is_char_boundary(new.len() - suffix) {
        suffix -= 1;
    }
    Some(TextEdit {
        range: doc.range(prefix, old.len() - suffix),
        new_text: doc.outgoing(&new[prefix..new.len() - suffix]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_found_around_the_cursor() {
        let prefixes = vec!["REQ".to_owned()];
        let text = "See [[REQ-014]] and REQ-7, UTF-8.";
        let at = text.find("014").unwrap();
        assert_eq!(
            id_at(text, at, &prefixes).map(|(s, e, _)| &text[s..e]),
            Some("REQ-014")
        );
        assert_eq!(id_at(text, text.find("REQ-7").unwrap(), &prefixes), None);
        assert_eq!(id_at(text, text.find("UTF").unwrap(), &prefixes), None);
        assert_eq!(id_at(text, text.len(), &prefixes), None);
    }

    #[test]
    fn edits_cover_only_what_changed() {
        let doc = Doc::new("a\nstatus = \"draft\"\nz\n");
        let e = edit_between(&doc, &doc.text, "a\nstatus = \"approved\"\nz\n").unwrap();
        assert_eq!(e.new_text, "approved");
        assert_eq!(
            (
                e.range.start.line,
                e.range.start.character,
                e.range.end.character
            ),
            (1, 10, 15)
        );
        assert!(edit_between(&doc, "x", "x").is_none());
    }
}
