//! Reading values out of a spanned `toml_edit` document while collecting
//! findings, shared by the `kiln.toml` and front-matter parsers.

use kiln_model::{Finding, Rule, Span};
use toml_edit::{Item, Key, TableLike, Value};

use crate::text::LineIndex;

/// One document being read: where spans come from, and where findings go.
pub struct Walk<'a> {
    pub path: &'a str,
    pub lines: &'a LineIndex<'a>,
    /// Added to every `toml_edit` span: the front matter's offset in its
    /// file.
    pub offset: usize,
    pub findings: Vec<Finding>,
}

impl<'a> Walk<'a> {
    pub fn span(&self, range: Option<std::ops::Range<usize>>) -> Span {
        match range {
            Some(r) => self.lines.span(r.start + self.offset, r.end + self.offset),
            None => self.lines.span(self.offset, self.offset),
        }
    }

    pub fn item_span(&self, item: &Item) -> Span {
        self.span(item.span())
    }

    pub fn value_span(&self, value: &Value) -> Span {
        self.span(value.span())
    }

    pub fn key_span(&self, key: &Key) -> Span {
        self.span(key.span())
    }

    pub fn report(&mut self, rule: Rule, span: Span, message: impl Into<String>) {
        self.findings
            .push(Finding::new(rule, self.path, span, message));
    }

    /// The span of `key` in `table`, falling back to the table's.
    pub fn span_of_key(&self, table: &dyn TableLike, key: &str) -> Span {
        match table.get_key_value(key) {
            Some((k, item)) => match k.span() {
                Some(_) => self.key_span(k),
                None => self.item_span(item),
            },
            None => self.span(None),
        }
    }

    /// Every key of `table` not in `known`, reported as `rule`.
    pub fn unknown_keys(&mut self, table: &dyn TableLike, known: &[&str], rule: Rule, place: &str) {
        let unknown: Vec<(String, Span)> = table
            .iter()
            .filter(|(k, _)| !known.contains(k))
            .map(|(k, _)| (k.to_owned(), self.span_of_key(table, k)))
            .collect();
        for (key, span) in unknown {
            self.report(rule, span, format!("unknown key `{key}` {place}"));
        }
    }

    /// A string value; `wrong` reports a value of another type.
    pub fn string(&mut self, item: &Item, rule: Rule, what: &str) -> Option<(String, Span)> {
        match item.as_str() {
            Some(s) => Some((s.to_owned(), self.item_span(item))),
            None => {
                let span = self.item_span(item);
                self.report(rule, span, format!("{what} must be a string"));
                None
            }
        }
    }

    /// An array of strings. Non-string elements are reported and skipped.
    pub fn strings(&mut self, item: &Item, rule: Rule, what: &str) -> Option<Vec<(String, Span)>> {
        let Some(array) = item.as_array() else {
            let span = self.item_span(item);
            self.report(rule, span, format!("{what} must be an array of strings"));
            return None;
        };
        let mut out = Vec::new();
        for value in array.iter() {
            match value.as_str() {
                Some(s) => out.push((s.to_owned(), self.value_span(value))),
                None => {
                    let span = self.value_span(value);
                    self.report(rule, span, format!("{what} must hold only strings"));
                }
            }
        }
        Some(out)
    }

    pub fn boolean(&mut self, item: &Item, rule: Rule, what: &str) -> Option<bool> {
        let value = item.as_bool();
        if value.is_none() {
            let span = self.item_span(item);
            self.report(rule, span, format!("{what} must be true or false"));
        }
        value
    }

    pub fn table<'t>(
        &mut self,
        item: &'t Item,
        rule: Rule,
        what: &str,
    ) -> Option<&'t dyn TableLike> {
        let table = item.as_table_like();
        if table.is_none() {
            let span = self.item_span(item);
            self.report(rule, span, format!("{what} must be a table"));
        }
        table
    }
}

/// The value's text span inside its quotes, when `span` covers a basic or
/// literal string written on one line; otherwise `span` unchanged. Edits
/// and findings point at the ID itself, not its quotes.
pub fn inside_quotes(text: &str, span: kiln_model::Span, lines: &LineIndex) -> kiln_model::Span {
    let raw = &text[span.start..span.end];
    let quoted = raw.len() >= 2
        && ((raw.starts_with('"') && raw.ends_with('"') && !raw.starts_with("\"\"\""))
            || (raw.starts_with('\'') && raw.ends_with('\'') && !raw.starts_with("'''")));
    if quoted {
        lines.span(span.start + 1, span.end - 1)
    } else {
        span
    }
}
