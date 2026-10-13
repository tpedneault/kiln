//! Items, references and code references as parsed: every value with the
//! span it came from, so a finding can point at it and an edit can
//! replace exactly it.

use std::fmt;

use crate::finding::Span;
use crate::id::{IdError, ItemId};

/// An ID as written somewhere: front matter, a body, a code comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ref {
    /// The text as written (`REQ-014`, or `REQ-14` when misspelt).
    pub raw: String,
    /// The ID, or why the text isn't one.
    pub id: Result<ItemId, IdError>,
    /// The ID text itself, without quotes or brackets.
    pub span: Span,
}

impl Ref {
    pub fn new(raw: impl Into<String>, span: Span) -> Self {
        let raw = raw.into();
        let id = raw.parse();
        Ref { raw, id, span }
    }
}

/// A TOML local date (`due = 2026-11-30`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl Date {
    /// The civil date `days` days after 1970-01-01 (negative before), so
    /// the caller can pass today without this crate reading a clock.
    pub fn from_unix_days(days: i64) -> Date {
        // Howard Hinnant's days-to-civil algorithm.
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
        Date { year, month, day }
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// A `[links]` entry: one target of one role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkEntry {
    pub role: String,
    pub role_span: Span,
    pub target: Ref,
}

/// A reference in a body: a mention, not a link (FORMAT.md, "Body").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    pub target: Ref,
    /// `[[ID]]` rather than a bare ID in prose.
    pub explicit: bool,
}

/// A relative link or image destination in a body (`../img/x.png`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelativeLink {
    /// As written, without `#fragment` or `?query`, percent-decoded.
    pub dest: String,
    pub span: Span,
}

/// One item file. `type_name`, `title` and `status` are empty when the
/// front matter left them out (a K104 finding says so).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Relative to the project root, with `/` separators.
    pub path: String,
    pub id: ItemId,
    pub id_span: Span,
    pub type_name: String,
    pub type_span: Span,
    pub title: String,
    pub title_span: Span,
    pub status: String,
    pub status_span: Span,
    pub parent: Option<Ref>,
    pub tags: Vec<String>,
    pub jira: Vec<String>,
    pub owner: Option<String>,
    pub due: Option<Date>,
    pub links: Vec<LinkEntry>,
    pub mentions: Vec<Mention>,
    pub relative_links: Vec<RelativeLink>,
}

/// A code reference: a source line matching a code pattern, which makes
/// that location the source of a link (FORMAT.md, "Code references").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeRef {
    /// Relative to the project root, with `/` separators.
    pub path: String,
    /// The word before the colon (`implements`).
    pub role: String,
    pub role_span: Span,
    pub target: Ref,
}

impl CodeRef {
    /// `crates/x/src/lib.rs:12`.
    pub fn location(&self) -> String {
        format!("{}:{}", self.path, self.target.span.line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_days_become_civil_dates() {
        assert_eq!(Date::from_unix_days(0).to_string(), "1970-01-01");
        assert_eq!(Date::from_unix_days(-1).to_string(), "1969-12-31");
        assert_eq!(Date::from_unix_days(11_016).to_string(), "2000-02-29");
        assert_eq!(Date::from_unix_days(20_739).to_string(), "2026-10-13");
    }
}
