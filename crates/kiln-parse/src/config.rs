//! `kiln.toml` to [`Config`], with K001–K004 findings.
//!
//! Lenient where it can be: an invalid type, role or pattern is reported
//! and left out, and the rest of the configuration is still returned, so
//! one typo doesn't hide every other finding. Only a file that isn't TOML,
//! or whose `format` this Kiln doesn't read, yields no configuration.

use std::collections::{BTreeMap, HashSet};

use kiln_model::{
    is_prefix, CodeConfig, Config, Finding, LinkRole, Rule, Severity, Span, TypeDef, CODE,
};
use toml_edit::{ImDocument, Item, TableLike};

use crate::code::compile_pattern;
use crate::text::LineIndex;
use crate::toml_walk::Walk;

/// The only `format` this Kiln reads.
pub const FORMAT: i64 = 1;

pub const CONFIG_FILE: &str = "kiln.toml";

const TOP_KEYS: &[&str] = &[
    "format",
    "name",
    "paths",
    "code",
    "repository",
    "types",
    "links",
    "coverage",
    "render",
    "jira",
    "check",
    "confluence",
    "export",
];
const TYPE_KEYS: &[&str] = &[
    "prefix", "title", "plural", "statuses", "initial", "done", "closed", "parents", "template",
];

/// Parses `kiln.toml` (already normalised text).
pub fn parse_config(text: &str) -> (Option<Config>, Vec<Finding>) {
    let lines = LineIndex::new(text);
    let mut w = Walk {
        path: CONFIG_FILE,
        lines: &lines,
        offset: 0,
        findings: Vec::new(),
    };
    let doc = match ImDocument::parse(text) {
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
                Rule::K001,
                span,
                format!("kiln.toml isn't valid TOML: {message}"),
            );
            return (None, w.findings);
        }
    };
    let root = doc.as_table();
    secrets(&mut w, root);
    w.unknown_keys(root, TOP_KEYS, Rule::K003, "in kiln.toml");

    let format = match root.get("format") {
        None => {
            w.report(
                Rule::K002,
                w.span(None),
                format!("`format` is required (format = {FORMAT})"),
            );
            return (None, w.findings);
        }
        Some(item) => match item.as_integer() {
            Some(FORMAT) => FORMAT as u32,
            Some(n) => {
                let span = w.item_span(item);
                let message = if n > FORMAT {
                    format!("format {n} is newer than this Kiln reads ({FORMAT}); update Kiln")
                } else {
                    format!("format {n} isn't a Kiln format; the current one is {FORMAT}")
                };
                w.report(Rule::K002, span, message);
                return (None, w.findings);
            }
            None => {
                let span = w.item_span(item);
                w.report(Rule::K002, span, "`format` must be an integer");
                return (None, w.findings);
            }
        },
    };

    let name = match root.get("name") {
        Some(item) => w
            .string(item, Rule::K002, "`name`")
            .map(|(s, _)| s)
            .unwrap_or_default(),
        None => {
            w.report(
                Rule::K002,
                w.span(None),
                "`name` is required: the project's name in reports",
            );
            String::new()
        }
    };

    let paths = match root.get("paths") {
        Some(item) => relative_paths(&mut w, item, "`paths`"),
        None => vec!["design".to_owned()],
    };

    let code = root
        .get("code")
        .map(|item| code_config(&mut w, item))
        .unwrap_or_default();

    let mut repository_web = None;
    if let Some(table) = root
        .get("repository")
        .and_then(|i| w.table(i, Rule::K002, "`[repository]`"))
    {
        w.unknown_keys(table, &["web"], Rule::K003, "in [repository]");
        if let Some(item) = table.get("web") {
            repository_web = w
                .string(item, Rule::K002, "`repository.web`")
                .map(|(s, _)| s);
        }
    }

    let types = match root
        .get("types")
        .and_then(|i| w.table(i, Rule::K002, "`[types]`"))
    {
        Some(table) => types(&mut w, table),
        None => Vec::new(),
    };
    if types.is_empty() {
        let span = w.span_of_key(root, "types");
        w.report(Rule::K002, span, "at least one [types.<name>] is required");
    }

    let links = match root
        .get("links")
        .and_then(|i| w.table(i, Rule::K002, "`[links]`"))
    {
        Some(table) => links(&mut w, table, &types),
        None => Vec::new(),
    };

    let coverage_roles = coverage(&mut w, root.get("coverage"), &links);

    let mut link_bare_ids = true;
    if let Some(table) = root
        .get("render")
        .and_then(|i| w.table(i, Rule::K002, "`[render]`"))
    {
        w.unknown_keys(table, &["link_bare_ids"], Rule::K003, "in [render]");
        if let Some(item) = table.get("link_bare_ids") {
            link_bare_ids = w
                .boolean(item, Rule::K002, "`render.link_bare_ids`")
                .unwrap_or(true);
        }
    }

    let mut jira_url = None;
    if let Some(table) = root
        .get("jira")
        .and_then(|i| w.table(i, Rule::K002, "`[jira]`"))
    {
        w.unknown_keys(table, &["url"], Rule::K003, "in [jira]");
        if let Some(item) = table.get("url") {
            jira_url = w.string(item, Rule::K002, "`jira.url`").map(|(s, _)| s);
        }
    }

    let severities = match root
        .get("check")
        .and_then(|i| w.table(i, Rule::K002, "`[check]`"))
    {
        Some(table) => severities(&mut w, table),
        None => BTreeMap::new(),
    };

    let config = Config {
        format,
        name,
        paths,
        code,
        repository_web,
        types,
        links,
        coverage_roles,
        link_bare_ids,
        jira_url,
        severities,
    };
    (Some(config), w.findings)
}

/// K004: a key named like a credential, anywhere in the file.
// implements: REQ-014
fn secrets(w: &mut Walk, table: &dyn TableLike) {
    for (key, item) in table.iter() {
        let lower = key.to_ascii_lowercase();
        if ["token", "password", "passwd", "secret", "api_key", "apikey"]
            .iter()
            .any(|s| lower.contains(s))
        {
            let span = w.span_of_key(table, key);
            w.report(
                Rule::K004,
                span,
                format!("`{key}` looks like a secret; kiln.toml is committed, so credentials go in the environment or the OS credential store"),
            );
        }
        match item {
            Item::Table(t) => secrets(w, t),
            Item::Value(toml_edit::Value::InlineTable(t)) => secrets(w, t),
            Item::ArrayOfTables(array) => {
                for t in array.iter() {
                    secrets(w, t);
                }
            }
            _ => {}
        }
    }
}

/// Relative paths inside the project: no absolute paths, no `..`.
fn relative_paths(w: &mut Walk, item: &Item, what: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (path, span) in w.strings(item, Rule::K002, what).unwrap_or_default() {
        match confined(&path) {
            Some(p) => out.push(p),
            None => w.report(
                Rule::K002,
                span,
                format!("{what}: `{path}` must be relative and inside the project (no `..`)"),
            ),
        }
    }
    out
}

/// `path` with `/` separators and no trailing `/`, when it stays inside
/// the project root; `None` for absolute paths and any `..`.
pub fn confined(path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    let bytes = path.as_bytes();
    let absolute = path.starts_with('/') || (bytes.len() >= 2 && bytes[1] == b':');
    if absolute || path.is_empty() || path.split('/').any(|part| part == "..") {
        return None;
    }
    let trimmed = path.trim_end_matches('/');
    let trimmed = trimmed.strip_prefix("./").unwrap_or(trimmed);
    Some(if trimmed.is_empty() {
        ".".to_owned()
    } else {
        trimmed.to_owned()
    })
}

fn code_config(w: &mut Walk, item: &Item) -> CodeConfig {
    let Some(table) = w.table(item, Rule::K002, "`[code]`") else {
        return CodeConfig::default();
    };
    w.unknown_keys(
        table,
        &["paths", "exclude", "patterns"],
        Rule::K003,
        "in [code]",
    );
    let paths = table
        .get("paths")
        .map(|i| relative_paths(w, i, "`code.paths`"))
        .unwrap_or_default();
    let exclude = table
        .get("exclude")
        .and_then(|i| w.strings(i, Rule::K002, "`code.exclude`"))
        .unwrap_or_default()
        .into_iter()
        .map(|(s, _)| s)
        .collect();
    let mut patterns = Vec::new();
    if let Some(item) = table.get("patterns") {
        for (pattern, span) in w
            .strings(item, Rule::K002, "`code.patterns`")
            .unwrap_or_default()
        {
            match compile_pattern(&pattern) {
                Ok(_) => patterns.push(pattern),
                Err(message) => w.report(Rule::K002, span, format!("code pattern: {message}")),
            }
        }
    }
    CodeConfig {
        paths,
        exclude,
        patterns,
    }
}

/// Lowercase ASCII words joined by `-`: type names, roles, statuses.
pub fn is_word(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn capitalised(name: &str) -> String {
    let words: Vec<String> = name
        .split('-')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(first) => first.to_uppercase().chain(c).collect(),
                None => String::new(),
            }
        })
        .collect();
    words.join(" ")
}

fn types(w: &mut Walk, table: &dyn TableLike) -> Vec<TypeDef> {
    let mut out: Vec<TypeDef> = Vec::new();
    let mut prefixes: HashSet<String> = HashSet::new();
    // Names first, so `parents` can name a type declared after it.
    let names: Vec<String> = table.iter().map(|(k, _)| k.to_owned()).collect();
    for (name, item) in table.iter() {
        let key_span = w.span_of_key(table, name);
        if name == CODE || !is_word(name) {
            let why = if name == CODE {
                "is reserved for code references"
            } else {
                "must be lowercase letters, digits and `-`"
            };
            w.report(Rule::K002, key_span, format!("type name `{name}` {why}"));
            continue;
        }
        let Some(t) = w.table(item, Rule::K002, &format!("`[types.{name}]`")) else {
            continue;
        };
        w.unknown_keys(t, TYPE_KEYS, Rule::K003, &format!("in [types.{name}]"));
        let prefix = match t.get("prefix") {
            Some(i) => w.string(i, Rule::K002, "`prefix`"),
            None => {
                w.report(
                    Rule::K002,
                    key_span,
                    format!("[types.{name}] needs a `prefix` (REQ)"),
                );
                None
            }
        };
        let Some((prefix, prefix_span)) = prefix else {
            continue;
        };
        if !is_prefix(&prefix) {
            w.report(
                Rule::K002,
                prefix_span,
                format!("prefix `{prefix}` must be uppercase letters and digits, starting with a letter"),
            );
            continue;
        }
        if !prefixes.insert(prefix.clone()) {
            w.report(
                Rule::K002,
                prefix_span,
                format!("prefix `{prefix}` is used by another type"),
            );
            continue;
        }
        let statuses = word_list(w, t, "statuses", name);
        if statuses.is_empty() {
            w.report(
                Rule::K002,
                key_span,
                format!("[types.{name}] needs `statuses`"),
            );
            continue;
        }
        let initial = match t
            .get("initial")
            .and_then(|i| w.string(i, Rule::K002, "`initial`"))
        {
            Some((s, span)) if !statuses.contains(&s) => {
                w.report(
                    Rule::K002,
                    span,
                    format!("initial status `{s}` isn't one of the type's statuses"),
                );
                statuses[0].clone()
            }
            Some((s, _)) => s,
            None => statuses[0].clone(),
        };
        let done = subset(w, t, "done", &statuses, name);
        let closed = subset(w, t, "closed", &statuses, name);
        let mut parents = Vec::new();
        if let Some(item) = t.get("parents") {
            for (parent, span) in w.strings(item, Rule::K002, "`parents`").unwrap_or_default() {
                if names.contains(&parent) && parent != CODE {
                    parents.push(parent);
                } else {
                    w.report(
                        Rule::K002,
                        span,
                        format!("parents: `{parent}` isn't a type"),
                    );
                }
            }
        }
        let title = t
            .get("title")
            .and_then(|i| w.string(i, Rule::K002, "`title`"))
            .map(|(s, _)| s)
            .unwrap_or_else(|| capitalised(name));
        let plural = t
            .get("plural")
            .and_then(|i| w.string(i, Rule::K002, "`plural`"))
            .map(|(s, _)| s)
            .unwrap_or_else(|| format!("{title}s"));
        let template = match t
            .get("template")
            .and_then(|i| w.string(i, Rule::K002, "`template`"))
        {
            Some((path, span)) => match confined(&path) {
                Some(p) => Some(p),
                None => {
                    w.report(
                        Rule::K002,
                        span,
                        format!("template `{path}` must be relative and inside the project"),
                    );
                    None
                }
            },
            None => None,
        };
        out.push(TypeDef {
            name: name.to_owned(),
            prefix,
            title,
            plural,
            statuses,
            initial,
            done,
            closed,
            parents,
            template,
        });
    }
    out
}

fn word_list(w: &mut Walk, t: &dyn TableLike, key: &str, type_name: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let Some(item) = t.get(key) else { return out };
    for (s, span) in w
        .strings(item, Rule::K002, &format!("`{key}`"))
        .unwrap_or_default()
    {
        if !is_word(&s) {
            w.report(
                Rule::K002,
                span,
                format!("status `{s}` must be lowercase letters, digits and `-`"),
            );
        } else if out.contains(&s) {
            w.report(
                Rule::K002,
                span,
                format!("status `{s}` is listed twice in [types.{type_name}]"),
            );
        } else {
            out.push(s);
        }
    }
    out
}

fn subset(
    w: &mut Walk,
    t: &dyn TableLike,
    key: &str,
    statuses: &[String],
    type_name: &str,
) -> Vec<String> {
    let mut out = Vec::new();
    let Some(item) = t.get(key) else { return out };
    for (s, span) in w
        .strings(item, Rule::K002, &format!("`{key}`"))
        .unwrap_or_default()
    {
        if statuses.contains(&s) {
            out.push(s);
        } else {
            w.report(
                Rule::K002,
                span,
                format!("{key}: `{s}` isn't a status of [types.{type_name}]"),
            );
        }
    }
    out
}

fn links(w: &mut Walk, table: &dyn TableLike, types: &[TypeDef]) -> Vec<LinkRole> {
    let mut out = Vec::new();
    let known = |t: &str| types.iter().any(|d| d.name == t);
    for (name, item) in table.iter() {
        let key_span = w.span_of_key(table, name);
        if !is_word(name) {
            w.report(
                Rule::K002,
                key_span,
                format!("link role `{name}` must be lowercase letters, digits and `-`"),
            );
            continue;
        }
        let Some(t) = w.table(item, Rule::K002, &format!("`[links.{name}]`")) else {
            continue;
        };
        w.unknown_keys(
            t,
            &["from", "to", "inverse"],
            Rule::K003,
            &format!("in [links.{name}]"),
        );
        let mut from = Vec::new();
        for (ty, span) in t
            .get("from")
            .and_then(|i| w.strings(i, Rule::K002, "`from`"))
            .unwrap_or_default()
        {
            if ty == CODE || known(&ty) {
                from.push(ty);
            } else {
                w.report(Rule::K002, span, format!("from: `{ty}` isn't a type"));
            }
        }
        let mut to = Vec::new();
        for (ty, span) in t
            .get("to")
            .and_then(|i| w.strings(i, Rule::K002, "`to`"))
            .unwrap_or_default()
        {
            if ty == CODE {
                w.report(Rule::K002, span, "to: `code` can only be a link's source");
            } else if known(&ty) {
                to.push(ty);
            } else {
                w.report(Rule::K002, span, format!("to: `{ty}` isn't a type"));
            }
        }
        let inverse = t
            .get("inverse")
            .and_then(|i| w.string(i, Rule::K002, "`inverse`"))
            .map(|(s, _)| s);
        let (Some(inverse), false, false) = (inverse, from.is_empty(), to.is_empty()) else {
            w.report(
                Rule::K002,
                key_span,
                format!("[links.{name}] needs `from`, `to` and `inverse`, naming types that exist"),
            );
            continue;
        };
        out.push(LinkRole {
            name: name.to_owned(),
            from,
            to,
            inverse,
        });
    }
    out
}

fn coverage(w: &mut Walk, item: Option<&Item>, links: &[LinkRole]) -> Vec<String> {
    let default = || {
        ["implements", "verifies"]
            .iter()
            .filter(|r| links.iter().any(|l| l.name == **r))
            .map(|r| (*r).to_owned())
            .collect()
    };
    let Some(table) = item.and_then(|i| w.table(i, Rule::K002, "`[coverage]`")) else {
        return default();
    };
    w.unknown_keys(table, &["roles"], Rule::K003, "in [coverage]");
    let Some(item) = table.get("roles") else {
        return default();
    };
    let mut out = Vec::new();
    for (role, span) in w
        .strings(item, Rule::K002, "`coverage.roles`")
        .unwrap_or_default()
    {
        if links.iter().any(|l| l.name == role) {
            out.push(role);
        } else {
            w.report(
                Rule::K002,
                span,
                format!("coverage: `{role}` isn't a link role"),
            );
        }
    }
    out
}

fn severities(w: &mut Walk, table: &dyn TableLike) -> BTreeMap<Rule, Severity> {
    let mut out = BTreeMap::new();
    for (code, item) in table.iter() {
        let key_span: Span = w.span_of_key(table, code);
        let Some(rule) = Rule::parse(code) else {
            w.report(
                Rule::K002,
                key_span,
                format!("[check]: `{code}` isn't a check rule"),
            );
            continue;
        };
        let value = w.string(item, Rule::K002, &format!("`check.{code}`"));
        match value.as_ref().and_then(|(s, _)| Severity::parse(s)) {
            Some(severity) => {
                out.insert(rule, severity);
            }
            None => {
                if let Some((s, span)) = value {
                    w.report(
                        Rule::K002,
                        span,
                        format!("[check] {code}: `{s}` isn't error, warning or off"),
                    );
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(findings: &[Finding]) -> Vec<String> {
        findings
            .iter()
            .map(|f| format!("{}@{}", f.rule, f.line))
            .collect()
    }

    #[test]
    fn the_default_configuration_is_clean() {
        let (config, findings) = parse_config(&kiln_model::default_config("Flint"));
        assert_eq!(findings, vec![]);
        let config = config.unwrap();
        assert_eq!(config.name, "Flint");
        assert_eq!(config.types.len(), 7);
        assert_eq!(config.links.len(), 6);
        assert_eq!(config.coverage_roles, ["implements", "verifies"]);
        let req = config.type_named("requirement").unwrap();
        assert_eq!(
            (req.title.as_str(), req.plural.as_str()),
            ("Requirement", "Requirements")
        );
        assert_eq!(req.folder(), "requirements");
        assert_eq!(config.type_named("design").unwrap().initial, "draft");
    }

    #[test]
    fn not_toml_is_k001_at_the_error() {
        let (config, findings) = parse_config("format = 1\nname = \n");
        assert!(config.is_none());
        assert_eq!(codes(&findings), ["K001@2"]);
    }

    #[test]
    fn a_newer_format_is_refused_with_the_reason() {
        let (config, findings) = parse_config("format = 2\nname = \"x\"\n");
        assert!(config.is_none());
        assert!(
            findings[0].message.contains("newer than this Kiln"),
            "{}",
            findings[0].message
        );
    }

    #[test]
    // verifies: REQ-014
    fn mistakes_are_reported_where_they_are_and_the_rest_survives() {
        let text = r#"format = 1
name = "x"
paths = ["../outside"]
colour = "red"
api_token = "abc"

[types.requirement]
prefix = "REQ"
statuses = ["draft", "approved"]
done = ["finished"]
parents = ["epic"]

[types.test]
prefix = "REQ"
statuses = ["draft"]

[types.risk]
prefix = "RISK"
statuses = ["open"]
colour = 1

[links.verifies]
from = ["test"]
to = ["code"]
inverse = "verified by"

[code]
patterns = ["no ids group here", "(?P<ids>"]

[check]
K201 = "loud"
K999 = "off"
K105 = "off"
"#;
        let (config, findings) = parse_config(text);
        let config = config.unwrap();
        let mut got = findings.clone();
        got.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
        // 5: an unknown key that is also a secret; 22–24: `test` was
        // dropped for its duplicate prefix, so the role has no `from`.
        assert_eq!(
            codes(&got),
            [
                "K002@3", "K003@4", "K003@5", "K004@5", "K002@10", "K002@11", "K002@14", "K003@20",
                "K002@22", "K002@23", "K002@24", "K002@28", "K002@28", "K002@31", "K002@32"
            ]
        );
        assert_eq!(
            config
                .types
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>(),
            ["requirement", "risk"]
        );
        assert!(config.links.is_empty());
        assert_eq!(config.severity(Rule::K105), Severity::Off);
        assert!(config.paths.is_empty());
    }

    #[test]
    fn paths_are_confined_to_the_project() {
        assert_eq!(confined("design/").as_deref(), Some("design"));
        assert_eq!(confined("./a\\b").as_deref(), Some("a/b"));
        assert_eq!(confined("."), Some(".".to_owned()));
        for bad in ["/etc", "C:/x", "a/../../b", ""] {
            assert_eq!(confined(bad), None, "{bad}");
        }
    }
}
