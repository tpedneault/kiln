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
    /// `[requirements] levels`: the requirement types, highest level
    /// first. Empty when not written; see [`Config::levels`].
    pub levels: Vec<String>,
    /// `[requirements] trace`: the role a requirement uses to point at
    /// the level above (`refines`).
    pub trace_role: String,
    /// `[render] link_bare_ids`.
    pub link_bare_ids: bool,
    /// `[jira] url`.
    pub jira_url: Option<String>,
    /// `[jira] project` and what goes with it: Kiln pushes these items to
    /// Jira (`kiln jira sync`). `None` without `project`: links only.
    pub jira_sync: Option<JiraSync>,
    /// `[check]` overrides.
    pub severities: BTreeMap<Rule, Severity>,
    /// `[confluence]`, when present and usable.
    pub confluence: Option<ConfluenceConfig>,
    /// `[[export]]`, the valid ones, in order.
    pub exports: Vec<ExportTarget>,
    /// `[signoff]`: type → the statuses that are a person's call (K19,
    /// `kiln signoff`). `None` when not written; see
    /// [`Config::signoff_statuses`].
    pub signoff: Option<BTreeMap<String, Vec<String>>>,
}

/// The sign-off statuses when `[signoff]` isn't written: the agent
/// guide's rule (approving a requirement, accepting or rejecting a
/// decision, accepting or closing a risk, finishing a milestone), where
/// the type has those statuses. A requirement's `verified` counts too: it
/// says the evidence was accepted, and a requirement moved straight to it
/// would otherwise skip the approval unseen.
const DEFAULT_SIGNOFF: &[(&str, &[&str])] = &[
    ("requirement", &["approved", "verified"]),
    ("decision", &["accepted", "rejected"]),
    ("risk", &["accepted", "closed"]),
    ("milestone", &["done"]),
];

/// What `kiln jira sync` pushes, and where (INTEGRATION.md, "Jira").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JiraSync {
    /// The Jira project's key, where tickets are created.
    pub project: String,
    /// The issue standing for this Kiln project; its sub-tasks are the
    /// tickets Kiln manages. Written by the first sync when absent.
    pub parent: Option<String>,
    /// The project issue's type, when the sync creates it.
    pub parent_type: String,
    /// The sub-tasks' issue type; `None`: the project's sub-task type.
    pub subtask_type: Option<String>,
    /// The Kiln types synced.
    pub types: Vec<String>,
    /// Kiln status → Jira status name; a status not here maps to a
    /// status category.
    pub status: BTreeMap<String, String>,
}

impl Default for JiraSync {
    fn default() -> Self {
        JiraSync {
            project: String::new(),
            parent: None,
            parent_type: "Task".into(),
            subtask_type: None,
            types: vec!["task".into()],
            status: BTreeMap::new(),
        }
    }
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
    /// The statuses of `type_name` that are sign-off: `[signoff]`'s list
    /// when it is written (and then only what it says), else the defaults
    /// the type actually has.
    pub fn signoff_statuses(&self, type_name: &str) -> Vec<&str> {
        match &self.signoff {
            Some(map) => map
                .get(type_name)
                .map(|v| v.iter().map(String::as_str).collect())
                .unwrap_or_default(),
            None => {
                let Some(ty) = self.type_named(type_name) else {
                    return Vec::new();
                };
                DEFAULT_SIGNOFF
                    .iter()
                    .filter(|(t, _)| *t == type_name)
                    .flat_map(|(_, statuses)| statuses.iter().copied())
                    .filter(|s| ty.statuses.iter().any(|x| x == s))
                    .collect()
            }
        }
    }

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

    /// The requirement levels, highest first: `[requirements] levels`, or
    /// without it the one type named `requirement`, so a project that
    /// never wrote `[requirements]` is checked as before.
    pub fn levels(&self) -> Vec<&str> {
        if !self.levels.is_empty() {
            return self.levels.iter().map(String::as_str).collect();
        }
        self.type_named("requirement")
            .map(|t| vec![t.name.as_str()])
            .unwrap_or_default()
    }

    /// The level of a type, 0 for the highest; `None` for a type that
    /// isn't a requirement level.
    pub fn level_of(&self, type_name: &str) -> Option<usize> {
        self.levels().iter().position(|t| *t == type_name)
    }

    /// The trace role, when there is more than one level and it is
    /// declared: with one level, nothing traces anywhere.
    pub fn trace(&self) -> Option<&LinkRole> {
        (self.levels().len() > 1)
            .then(|| self.role(&self.trace_role))
            .flatten()
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

/// The body templates `kiln init` writes for the default types, by type
/// name: a few bold labels or headings saying what the item should say,
/// so a new item starts with a shape rather than an empty body (REQ-021).
/// Like a preset's, they are copied into the project and its own from
/// then on (ADR-015); Kiln never rewrites them.
pub const ITEM_TEMPLATES: &[(&str, &str)] = &[
    ("requirement", include_str!("../templates/requirement.md")),
    ("design", include_str!("../templates/design.md")),
    ("decision", include_str!("../templates/decision.md")),
    ("milestone", include_str!("../templates/milestone.md")),
    ("task", include_str!("../templates/task.md")),
    ("test", include_str!("../templates/test.md")),
    ("risk", include_str!("../templates/risk.md")),
];

/// Where `kiln init` puts a type's body template: `<dir>/templates/<type>.md`.
pub fn item_template_path(dir: &str, type_name: &str) -> String {
    format!("{dir}/templates/{type_name}.md")
}

/// The `kiln.toml` `kiln init` writes: [`default_config`] with each
/// default type's `template` set to the file [`ITEM_TEMPLATES`] gives
/// it, under `dir` (the first of `paths`). Kept apart from
/// `default_config`, which is FORMAT.md's defaults and what tests build
/// projects from without template files.
pub fn init_config(name: &str, dir: &str) -> String {
    let mut out = String::new();
    let mut in_type: Option<&str> = None;
    for line in default_config(name).split_inclusive('\n') {
        out.push_str(line);
        if let Some(t) = line
            .trim_end()
            .strip_prefix("[types.")
            .and_then(|r| r.strip_suffix(']'))
        {
            in_type = ITEM_TEMPLATES
                .iter()
                .find(|(n, _)| *n == t)
                .map(|(n, _)| *n);
        } else if line.starts_with("prefix = ") {
            if let Some(t) = in_type.take() {
                out.push_str(&format!("template = \"{}\"\n", item_template_path(dir, t)));
            }
        }
    }
    out
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
    fn init_names_a_template_for_each_default_type() {
        let text = init_config("Demo", "design");
        for (t, _) in ITEM_TEMPLATES {
            assert!(text.contains(&format!("[types.{t}]\nprefix = ")), "{t}");
            assert_eq!(
                text.matches(&format!("template = \"design/templates/{t}.md\"\n"))
                    .count(),
                1,
                "{t}"
            );
        }
        assert_eq!(text.matches("template = ").count(), ITEM_TEMPLATES.len());
    }

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
            levels: vec![],
            trace_role: "refines".into(),
            link_bare_ids: true,
            jira_url: None,
            jira_sync: None,
            severities: BTreeMap::new(),
            confluence: None,
            exports: vec![],
            signoff: None,
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
