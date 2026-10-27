//! Findings: what `kiln check` reports, and the rules that produce them
//! (docs/FORMAT.md, "Check rules").
//!
//! Every producer (the config and item parsers, the code scanner, the
//! check rules) emits findings at their rule's default severity;
//! `kiln-check` applies the project's `[check]` overrides once, at the
//! end, so no producer needs the configuration to report a problem.

use std::fmt;

/// How much a finding matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Severity {
    /// Fails `kiln check` (exit 1).
    Error,
    Warning,
    /// Not reported.
    Off,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Off => "off",
        }
    }

    pub fn parse(s: &str) -> Option<Severity> {
        match s {
            "error" => Some(Severity::Error),
            "warning" => Some(Severity::Warning),
            "off" => Some(Severity::Off),
            _ => None,
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

macro_rules! rules {
    ($($name:ident $sev:ident $milestone:literal $summary:literal,)*) => {
        /// A check rule. The set is FORMAT.md's table; a rule whose
        /// milestone hasn't arrived exists (so `[check]` may name it) but
        /// nothing produces it yet.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Rule {
            $($name,)*
        }

        impl Rule {
            pub const ALL: &'static [Rule] = &[$(Rule::$name,)*];

            /// `K102`.
            pub fn code(self) -> &'static str {
                match self {
                    $(Rule::$name => stringify!($name),)*
                }
            }

            pub fn default_severity(self) -> Severity {
                match self {
                    $(Rule::$name => Severity::$sev,)*
                }
            }

            /// The milestone that starts producing it (`K1`).
            pub fn milestone(self) -> &'static str {
                match self {
                    $(Rule::$name => $milestone,)*
                }
            }

            /// One line, for help texts and reports.
            pub fn summary(self) -> &'static str {
                match self {
                    $(Rule::$name => $summary,)*
                }
            }
        }
    };
}

rules! {
    K001 Error "K1" "kiln.toml can't be parsed",
    K002 Error "K1" "kiln.toml is invalid",
    K003 Error "K1" "unknown key in kiln.toml",
    K004 Error "K1" "something that looks like a secret in kiln.toml",
    K005 Error "K2" "an [[export]] target is invalid",
    K100 Error "K1" "front matter can't be parsed",
    K101 Error "K1" "duplicate ID",
    K102 Error "K1" "reference to an ID that doesn't exist",
    K103 Error "K1" "parent cycle",
    K104 Error "K1" "unknown front-matter key, missing required key, wrong value",
    K105 Warning "K1" "file name doesn't start with the ID",
    K106 Warning "K1" "ID prefix doesn't match type",
    K107 Error "K1" "status not allowed for the type",
    K108 Error "K1" "link role or parent not allowed",
    K109 Warning "K1" "a link is written on its inverse side",
    K110 Error "K1" "relative link or image to a file that doesn't exist",
    K111 Error "K2" "a Mermaid block fails to parse",
    K112 Error "K2" "an export template is invalid",
    K201 Warning "K1" "a requirement with no verifies coverage",
    K202 Warning "K1" "a requirement with no implements coverage",
    K203 Warning "K1" "a non-closed item links to a closed one",
    K204 Warning "K1" "a superseded decision that nothing supersedes",
    K205 Warning "K1" "status verified but no passing verifies source",
    K206 Warning "K1" "a milestone past its due date and not done",
    K207 Off "K8" "a done work item whose Jira ticket isn't resolved, or the reverse",
    K208 Warning "K13" "a requirement below the top level that traces to nothing above it",
    K209 Warning "K13" "a trace link that doesn't point at the level directly above",
    K210 Warning "K13" "a requirement above the lowest level that nothing below refines",
}

impl Rule {
    pub fn parse(code: &str) -> Option<Rule> {
        Rule::ALL.iter().copied().find(|r| r.code() == code)
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// A place in a file: a byte range in the text Kiln parsed (line endings
/// normalised to `\n` for item files and `kiln.toml`, raw for code), and
/// the 1-based line and column (in characters) where it starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: u32,
    pub column: u32,
}

/// One problem, where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
// implements: REQ-011
pub struct Finding {
    pub rule: Rule,
    pub severity: Severity,
    /// Relative to the project root, with `/` separators.
    pub path: String,
    pub line: u32,
    pub column: u32,
    /// One line, naming what is wrong and, when it helps, the fix.
    pub message: String,
}

impl Finding {
    /// At the rule's default severity.
    pub fn new(
        rule: Rule,
        path: impl Into<String>,
        span: Span,
        message: impl Into<String>,
    ) -> Self {
        Finding {
            rule,
            severity: rule.default_severity(),
            path: path.into(),
            line: span.line.max(1),
            column: span.column.max(1),
            message: message.into(),
        }
    }

    /// Ordered by file, place, then rule: the order every output uses.
    pub fn sort_key(&self) -> (&str, u32, u32, Rule, &str) {
        (&self.path, self.line, self.column, self.rule, &self.message)
    }
}

impl fmt::Display for Finding {
    /// `path:line:col: K102 error: message`, the text output format.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {} {}: {}",
            self.path, self.line, self.column, self.rule, self.severity, self.message
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_round_trip_through_their_code() {
        for rule in Rule::ALL {
            assert_eq!(Rule::parse(rule.code()), Some(*rule));
        }
        assert_eq!(Rule::parse("K999"), None);
    }

    #[test]
    // verifies: REQ-011
    fn text_form_is_path_line_col_code_severity_message() {
        let span = Span {
            start: 0,
            end: 1,
            line: 3,
            column: 7,
        };
        let f = Finding::new(Rule::K102, "design/a.md", span, "REQ-099 doesn't exist");
        assert_eq!(
            f.to_string(),
            "design/a.md:3:7: K102 error: REQ-099 doesn't exist"
        );
    }
}
