//! `kiln.toml`, as a value (docs/FORMAT.md, "`kiln.toml`").
//!
//! Only what Kiln interprets is kept.

use std::collections::BTreeMap;

use crate::export::{ConfluenceConfig, ExportTarget};
use crate::finding::{Rule, Severity};

/// The pseudo-type of code references; only ever a link's `from`.
pub const CODE: &str = "code";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub format: u32,
    pub name: String,
    /// Where items live, relative to the project root.
    pub paths: Vec<String>,
    pub code: CodeConfig,
    /// `[repository] web`: a file's web URL, with `{commit}`, `{path}`,
    /// `{line}`.
    pub repository_web: Option<String>,
    /// In `kiln.toml` order, which is the order types are listed in.
    pub types: Vec<TypeDef>,
    /// In `kiln.toml` order.
    pub links: Vec<LinkRole>,
    /// `[coverage] roles`.
    pub coverage_roles: Vec<String>,
    /// `[render] link_bare_ids`.
    pub link_bare_ids: bool,
    /// `[jira] url`.
    pub jira_url: Option<String>,
    /// `[check]` overrides.
    pub severities: BTreeMap<Rule, Severity>,
    /// `[confluence]`, when present and usable.
    pub confluence: Option<ConfluenceConfig>,
    /// `[[export]]`, the valid ones, in order.
    pub exports: Vec<ExportTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CodeConfig {
    pub paths: Vec<String>,
    /// Globs (gitignore syntax) left out of the scan: fixtures that hold
    /// code references on purpose, vendored code.
    pub exclude: Vec<String>,
    /// Regexes with a named group `ids`. Empty means the default pattern
    /// ([`Config::code_patterns`]).
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDef {
    /// `requirement`.
    pub name: String,
    pub prefix: String,
    /// `Requirement`.
    pub title: String,
    /// `Requirements`.
    pub plural: String,
    pub statuses: Vec<String>,
    /// The status `kiln new` writes.
    pub initial: String,
    pub done: Vec<String>,
    pub closed: Vec<String>,
    /// Types allowed as `parent`.
    pub parents: Vec<String>,
    /// The body of a new item, relative to the project.
    pub template: Option<String>,
}

impl TypeDef {
    pub fn is_closed(&self, status: &str) -> bool {
        self.closed.iter().any(|s| s == status)
    }

    pub fn is_done(&self, status: &str) -> bool {
        self.done.iter().any(|s| s == status)
    }

    /// The folder `kiln new` writes into: the plural, lowercased, words
    /// joined by `-` (`requirements`).
    pub fn folder(&self) -> String {
        slug(&self.plural, usize::MAX)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRole {
    /// `verifies`.
    pub name: String,
    /// Type names, possibly [`CODE`].
    pub from: Vec<String>,
    pub to: Vec<String>,
    /// `verified by`: how the link reads from its target.
    pub inverse: String,
}

impl LinkRole {
    pub fn allows_from(&self, type_name: &str) -> bool {
        self.from.iter().any(|t| t == type_name)
    }

    pub fn allows_to(&self, type_name: &str) -> bool {
        self.to.iter().any(|t| t == type_name)
    }

    /// The inverse as a key someone might write by mistake
    /// (`verified-by`), for K109.
    pub fn inverse_key(&self) -> String {
        self.inverse
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("-")
    }
}

impl Config {
    pub fn type_named(&self, name: &str) -> Option<&TypeDef> {
        self.types.iter().find(|t| t.name == name)
    }

    pub fn type_for_prefix(&self, prefix: &str) -> Option<&TypeDef> {
        self.types.iter().find(|t| t.prefix == prefix)
    }

    pub fn role(&self, name: &str) -> Option<&LinkRole> {
        self.links.iter().find(|r| r.name == name)
    }

    /// The role whose inverse is written as `key` (`verified-by`).
    pub fn role_by_inverse_key(&self, key: &str) -> Option<&LinkRole> {
        self.links.iter().find(|r| r.inverse_key() == key)
    }

    pub fn export(&self, name: &str) -> Option<&ExportTarget> {
        self.exports.iter().find(|e| e.name == name)
    }

    pub fn prefixes(&self) -> Vec<String> {
        self.types.iter().map(|t| t.prefix.clone()).collect()
    }

    /// The configured patterns, or the default: one pattern matching
    /// `<role>: ID[, ID…]` for every role whose `from` includes `code`.
    /// Empty when there are no patterns and no such role.
    pub fn code_patterns(&self) -> Vec<String> {
        if !self.code.patterns.is_empty() {
            return self.code.patterns.clone();
        }
        let roles: Vec<&str> = self
            .links
            .iter()
            .filter(|r| r.allows_from(CODE))
            .map(|r| r.name.as_str())
            .collect();
        if roles.is_empty() {
            return Vec::new();
        }
        let id = r"[A-Z][A-Z0-9]*-\d+";
        vec![format!(
            r"\b(?:{}):\s*(?P<ids>{id}(?:\s*,\s*{id})*)",
            roles
                .iter()
                .map(|r| regex_escape(r))
                .collect::<Vec<_>>()
                .join("|")
        )]
    }

    /// The severity of `rule` in this project.
    pub fn severity(&self, rule: Rule) -> Severity {
        self.severities
            .get(&rule)
            .copied()
            .unwrap_or(rule.default_severity())
    }
}

fn regex_escape(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            let escape = !c.is_ascii_alphanumeric() && c != '_';
            escape.then_some('\\').into_iter().chain([c])
        })
        .collect()
}

/// Lowercase ASCII letters and digits, everything else one `-`, trimmed,
/// cut at a word boundary to at most `max` bytes (when a boundary exists).
pub fn slug(text: &str, max: usize) -> String {
    let mut out = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.len() > max {
        let cut = out[..=max].rfind('-').unwrap_or(max);
        out.truncate(cut);
    }
    out
}

/// The `kiln.toml` that `kiln init` writes: FORMAT.md's defaults.
pub fn default_config(name: &str) -> String {
    let name = name.replace('\\', "\\\\").replace('"', "\\\"");
    DEFAULT_CONFIG.replace("{name}", &name)
}

const DEFAULT_CONFIG: &str = r#"# Kiln project configuration: docs/FORMAT.md in Kiln's repository.
format = 1
name = "{name}"
paths = ["design"]

# Where source code is scanned for code references: a comment with
# `implements:` or `verifies:` and item IDs. Uncomment and list your
# source folders.
# [code]
# paths = ["src"]
# exclude = ["tests/fixtures"]

[types.requirement]
prefix = "REQ"
statuses = ["draft", "proposed", "approved", "implemented", "verified", "obsolete"]
initial = "draft"
done = ["verified"]
closed = ["obsolete"]
parents = ["requirement"]

[types.design]
prefix = "DES"
statuses = ["draft", "accepted", "obsolete"]
closed = ["obsolete"]
parents = ["design"]

[types.decision]
prefix = "ADR"
statuses = ["proposed", "accepted", "rejected", "superseded"]
closed = ["rejected", "superseded"]

[types.milestone]
prefix = "MS"
statuses = ["planned", "active", "done", "dropped"]
done = ["done"]
closed = ["dropped"]

[types.task]
prefix = "TASK"
statuses = ["todo", "doing", "blocked", "done", "dropped"]
done = ["done"]
closed = ["dropped"]
parents = ["task"]

[types.test]
prefix = "TEST"
statuses = ["draft", "ready", "passing", "failing", "obsolete"]
closed = ["obsolete"]

[types.risk]
prefix = "RISK"
statuses = ["open", "mitigated", "accepted", "closed"]
closed = ["closed"]

[links.implements]
from = ["design", "task", "code"]
to = ["requirement"]
inverse = "implemented by"

[links.verifies]
from = ["test", "code"]
to = ["requirement"]
inverse = "verified by"

[links.part-of]
from = ["task"]
to = ["milestone"]
inverse = "includes"

[links.supersedes]
from = ["decision"]
to = ["decision"]
inverse = "superseded by"

[links.mitigates]
from = ["requirement", "design", "task"]
to = ["risk"]
inverse = "mitigated by"

[links.depends-on]
from = ["task", "milestone"]
to = ["task", "milestone"]
inverse = "blocks"
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_lowercase_words_joined_by_dashes() {
        assert_eq!(
            slug("Flint appears within 50 ms!", 100),
            "flint-appears-within-50-ms"
        );
        assert_eq!(slug("  Ünïcode -- and   spaces ", 100), "n-code-and-spaces");
        assert_eq!(slug("one two three", 9), "one-two");
        assert_eq!(slug("abcdefghij", 4), "abcd");
    }

    #[test]
    fn default_pattern_covers_roles_from_code() {
        let config = Config {
            format: 1,
            name: "x".into(),
            paths: vec![],
            code: CodeConfig::default(),
            repository_web: None,
            types: vec![],
            links: vec![
                LinkRole {
                    name: "implements".into(),
                    from: vec![CODE.into()],
                    to: vec![],
                    inverse: "implemented by".into(),
                },
                LinkRole {
                    name: "part-of".into(),
                    from: vec!["task".into()],
                    to: vec![],
                    inverse: "includes".into(),
                },
                LinkRole {
                    name: "verifies".into(),
                    from: vec![CODE.into()],
                    to: vec![],
                    inverse: "verified by".into(),
                },
            ],
            coverage_roles: vec![],
            link_bare_ids: true,
            jira_url: None,
            severities: BTreeMap::new(),
            confluence: None,
            exports: vec![],
        };
        let patterns = config.code_patterns();
        assert_eq!(patterns.len(), 1);
        assert!(
            patterns[0].starts_with(r"\b(?:implements|verifies):"),
            "{}",
            patterns[0]
        );
        assert_eq!(config.links[0].inverse_key(), "implemented-by");
    }
}
