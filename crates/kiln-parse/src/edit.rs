//! Front-matter edits that keep the author's comments, order and spacing
//! (FORMAT.md, "Edits Kiln makes"): text in, text out. Reading, checking
//! the file didn't change and writing it atomically are the caller's.
//!
//! Each function changes only the keys it is about, through
//! `toml_edit::DocumentMut`, and never touches the body.

use kiln_model::Date;
use toml_edit::{Array, DocumentMut, Item, Table, Value};

use crate::item::sections;

/// A value for [`set_field`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldValue {
    String(String),
    Date(Date),
}

/// Why an edit wasn't made. Messages are one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditError(pub String);

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for EditError {}

fn with_front_matter(
    text: &str,
    edit: impl FnOnce(&mut DocumentMut) -> Result<(), EditError>,
) -> Result<String, EditError> {
    let s = match sections(text) {
        Some(Ok(s)) => s,
        _ => {
            return Err(EditError(
                "the file has no complete `+++` front matter".into(),
            ))
        }
    };
    let (start, end) = s.front_matter;
    let mut doc: DocumentMut = text[start..end]
        .parse()
        .map_err(|_| EditError("the front matter isn't valid TOML (run `kiln check`)".into()))?;
    edit(&mut doc)?;
    let mut fm = doc.to_string();
    if !fm.is_empty() && !fm.ends_with('\n') {
        fm.push('\n');
    }
    Ok(format!("{}{}{}", &text[..start], fm, &text[end..]))
}

/// Sets top-level `key` to `value`, or removes it for `None`. An existing
/// value keeps its surrounding whitespace and trailing comment.
// implements: REQ-013
pub fn set_field(text: &str, key: &str, value: Option<FieldValue>) -> Result<String, EditError> {
    with_front_matter(text, |doc| {
        match value {
            None => {
                doc.remove(key);
            }
            Some(value) => {
                let mut new = match value {
                    FieldValue::String(s) => Value::from(s),
                    FieldValue::Date(d) => {
                        let date = toml_edit::Date {
                            year: d.year as u16,
                            month: d.month,
                            day: d.day,
                        };
                        Value::from(toml_edit::Datetime {
                            date: Some(date),
                            time: None,
                            offset: None,
                        })
                    }
                };
                match doc.get_mut(key).and_then(Item::as_value_mut) {
                    Some(old) => {
                        *new.decor_mut() = old.decor().clone();
                        *old = new;
                    }
                    None => {
                        doc.insert(key, Item::Value(new));
                    }
                }
            }
        }
        Ok(())
    })
}

/// Sets `key = "value"` in `kiln.toml`'s `[table]`, which must exist: the
/// rest of the file, comments and layout included, is left as it is. For
/// what Kiln records there itself (`kiln jira sync` writing `[jira]
/// parent`).
pub fn set_config_key(
    text: &str,
    table: &str,
    key: &str,
    value: &str,
) -> Result<String, EditError> {
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|_| EditError("kiln.toml isn't valid TOML (run `kiln check`)".into()))?;
    let t = doc
        .get_mut(table)
        .and_then(Item::as_table_like_mut)
        .ok_or_else(|| EditError(format!("kiln.toml has no [{table}]")))?;
    match t.get_mut(key).and_then(Item::as_value_mut) {
        Some(old) => {
            let mut new = Value::from(value);
            *new.decor_mut() = old.decor().clone();
            *old = new;
        }
        None => {
            t.insert(key, Item::Value(Value::from(value)));
        }
    }
    Ok(doc.to_string())
}

/// Adds `target` to `[links] role`, creating either as needed. Adding a
/// link that is already there is not an error; the text comes back
/// unchanged.
pub fn add_link(text: &str, role: &str, target: &str) -> Result<String, EditError> {
    with_front_matter(text, |doc| {
        if !doc.contains_key("links") {
            doc.insert("links", Item::Table(Table::new()));
        }
        let links = doc
            .get_mut("links")
            .and_then(Item::as_table_like_mut)
            .ok_or_else(|| EditError("`links` isn't a table".into()))?;
        if !links.contains_key(role) {
            links.insert(role, Item::Value(Value::Array(Array::new())));
        }
        let array = links
            .get_mut(role)
            .and_then(Item::as_array_mut)
            .ok_or_else(|| EditError(format!("`links.{role}` isn't an array")))?;
        if !array.iter().any(|v| v.as_str() == Some(target)) {
            array.push(target);
        }
        Ok(())
    })
}

/// Removes `target` from `[links] role`, dropping the role when it
/// empties and `[links]` when that empties.
pub fn remove_link(text: &str, role: &str, target: &str) -> Result<String, EditError> {
    with_front_matter(text, |doc| {
        let missing = || EditError(format!("there is no `{role}` link to {target}"));
        let links = doc
            .get_mut("links")
            .and_then(Item::as_table_like_mut)
            .ok_or_else(missing)?;
        let array = links
            .get_mut(role)
            .and_then(Item::as_array_mut)
            .ok_or_else(missing)?;
        let before = array.len();
        array.retain(|v| v.as_str() != Some(target));
        if array.len() == before {
            return Err(missing());
        }
        if array.is_empty() {
            links.remove(role);
        }
        if links.is_empty() {
            doc.remove("links");
        }
        Ok(())
    })
}

/// Normalises the front matter for `textDocument/formatting`: top-level
/// keys in FORMAT.md's order (`id`, `type`, `title`, `status`, `parent`,
/// `tags`, `jira`, `owner`, `due`; unknown keys after them, in their
/// order; `[links]` and `[extra]` stay tables at the end), and every
/// string written as a basic `"…"` string. Comments travel with their key;
/// the body is never touched. A file already in shape comes back
/// unchanged.
pub fn format_front_matter(text: &str) -> Result<String, EditError> {
    with_front_matter(text, |doc| {
        let rank = |key: &str| {
            crate::item::KEYS
                .iter()
                .position(|k| *k == key)
                .unwrap_or(crate::item::KEYS.len())
        };
        doc.as_table_mut()
            .sort_values_by(|a, _, b, _| rank(a.get()).cmp(&rank(b.get())));
        requote_table(doc.as_table_mut());
        Ok(())
    })
}

fn requote_table(table: &mut dyn toml_edit::TableLike) {
    for (_, item) in table.iter_mut() {
        requote_item(item);
    }
}

fn requote_item(item: &mut Item) {
    match item {
        Item::Value(v) => requote_value(v),
        Item::Table(t) => requote_table(t),
        Item::ArrayOfTables(a) => a.iter_mut().for_each(|t| requote_table(t)),
        Item::None => {}
    }
}

fn requote_value(value: &mut Value) {
    match value {
        Value::String(s) if !s.display_repr().starts_with('"') => {
            let decor = s.decor().clone();
            let mut basic = Value::from(s.value().as_str());
            *basic.decor_mut() = decor;
            *value = basic;
        }
        Value::Array(a) => a.iter_mut().for_each(requote_value),
        Value::InlineTable(t) => t.iter_mut().for_each(|(_, v)| requote_value(v)),
        _ => {}
    }
}

/// Applies byte-range replacements to `text`. Ranges must not overlap;
/// order doesn't matter.
pub fn splice(text: &str, edits: &[(std::ops::Range<usize>, String)]) -> String {
    let mut edits: Vec<&(std::ops::Range<usize>, String)> = edits.iter().collect();
    edits.sort_by_key(|(r, _)| r.start);
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (range, new) in edits {
        out.push_str(&text[at..range.start]);
        out.push_str(new);
        at = range.end;
    }
    out.push_str(&text[at..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEM: &str = "+++\nid = \"REQ-001\"   # the first\ntype = \"requirement\"\ntitle = \"T\"\nstatus = \"draft\"  # still thinking\n\n[links]\n# why: the risk\nmitigates = [\"RISK-001\"]\n+++\n\nBody stays.\n";

    #[test]
    // verifies: REQ-013
    fn setting_a_value_keeps_its_comment() {
        let out = set_field(ITEM, "status", Some(FieldValue::String("approved".into()))).unwrap();
        assert_eq!(out, ITEM.replace("\"draft\"", "\"approved\""));
    }

    #[test]
    fn new_keys_land_before_tables_and_removal_is_clean() {
        let date = Date {
            year: 2026,
            month: 11,
            day: 30,
        };
        let out = set_field(ITEM, "due", Some(FieldValue::Date(date))).unwrap();
        assert!(
            out.contains("status = \"draft\"  # still thinking\ndue = 2026-11-30\n"),
            "{out}"
        );
        assert_eq!(set_field(&out, "due", None).unwrap(), ITEM);
    }

    #[test]
    // verifies: REQ-013
    fn links_are_added_and_removed_in_place() {
        let out = add_link(ITEM, "mitigates", "RISK-002").unwrap();
        assert!(
            out.contains("# why: the risk\nmitigates = [\"RISK-001\", \"RISK-002\"]\n"),
            "{out}"
        );
        assert_eq!(add_link(&out, "mitigates", "RISK-002").unwrap(), out);
        let out = remove_link(&out, "mitigates", "RISK-002").unwrap();
        assert_eq!(out, ITEM);
        let out = remove_link(&out, "mitigates", "RISK-001").unwrap();
        assert!(!out.contains("[links]"), "{out}");
        assert!(out.ends_with("+++\n\nBody stays.\n"));
        assert!(remove_link(ITEM, "verifies", "REQ-009")
            .unwrap_err()
            .0
            .contains("no `verifies` link"));
    }

    #[test]
    fn a_links_table_is_created_when_missing() {
        let plain = "+++\nid = \"TEST-001\"\n+++\nbody\n";
        let out = add_link(plain, "verifies", "REQ-001").unwrap();
        assert_eq!(
            out,
            "+++\nid = \"TEST-001\"\n\n[links]\nverifies = [\"REQ-001\"]\n+++\nbody\n"
        );
    }

    #[test]
    fn splice_applies_edits_in_any_order() {
        assert_eq!(
            splice(
                "a REQ-001 b REQ-001",
                &[(12..19, "REQ-002".into()), (2..9, "REQ-002".into())]
            ),
            "a REQ-002 b REQ-002"
        );
    }

    #[test]
    fn formatting_orders_keys_and_quotes_strings_only() {
        let messy = "+++\nstatus = 'draft' # still\ntitle = \"Fast\"\nzzz = 1\nid = \"REQ-001\"\ntags = ['a', \"b\"]\ntype = \"requirement\"\n\n[links]\nverifies = ['REQ-002']\n+++\nBody 'quoted' stays.\n";
        let out = format_front_matter(messy).unwrap();
        assert_eq!(
            out,
            "+++\nid = \"REQ-001\"\ntype = \"requirement\"\ntitle = \"Fast\"\nstatus = \"draft\" # still\ntags = [\"a\", \"b\"]\nzzz = 1\n\n[links]\nverifies = [\"REQ-002\"]\n+++\nBody 'quoted' stays.\n"
        );
        assert_eq!(format_front_matter(&out).unwrap(), out, "idempotent");
    }
}
