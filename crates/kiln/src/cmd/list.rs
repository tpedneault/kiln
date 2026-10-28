//! `kiln list`: items, filtered by type, status and tag; with `--jira`,
//! each linked ticket's status and assignee from the Jira mirror.
//!
//! The JSON form carries each item's outgoing links, and with `--body`
//! its body, so a program loads the whole plan in one call (REQ-020)
//! rather than one `kiln show` per item. Inverse links aren't repeated:
//! they are the same links read from the other end.

use std::path::Path;

use serde_json::json;

use super::json_out;
use crate::project::load;
use crate::{Fail, TextOrJson, EXIT_NETWORK};

/// What `kiln list` keeps.
#[derive(Debug, Default)]
pub struct Filters {
    pub type_name: Option<String>,
    pub status: Option<String>,
    pub tag: Option<String>,
}

// implements: REQ-020
pub fn run(
    project: Option<&Path>,
    filters: Filters,
    jira: bool,
    body: bool,
    format: TextOrJson,
) -> Result<u8, Fail> {
    let Filters {
        type_name,
        status,
        tag,
    } = filters;
    let loaded = load(project)?;
    let p = &loaded.project;
    if let Some(t) = &type_name {
        if p.config.type_named(t).is_none() {
            return Err(Fail::usage(format!("`{t}` isn't a type")));
        }
    }
    let items: Vec<_> = p
        .items
        .iter()
        .filter(|i| type_name.as_ref().is_none_or(|t| &i.type_name == t))
        .filter(|i| status.as_ref().is_none_or(|s| &i.status == s))
        .filter(|i| tag.as_ref().is_none_or(|t| i.tags.contains(t)))
        .collect();
    let mirror = if jira {
        if p.config.jira_url.is_none() {
            return Err(Fail::usage(
                "kiln.toml has no [jira] url to read tickets from",
            ));
        }
        Some(super::jira::mirror(p).map_err(|e| Fail::new(EXIT_NETWORK, e))?)
    } else {
        None
    };
    let tickets = |keys: &[String]| -> Vec<serde_json::Value> {
        let Some(m) = &mirror else {
            return Vec::new();
        };
        keys.iter()
            .map(|k| match m.ticket(k) {
                Some(t) => json!({
                    "key": t.key,
                    "summary": t.summary,
                    "status": t.status,
                    "resolved": t.resolved,
                    "assignee": t.assignee,
                }),
                None => json!({ "key": k, "missing": true }),
            })
            .collect()
    };
    let column = |keys: &[String]| -> String {
        let Some(m) = &mirror else {
            return String::new();
        };
        let cells: Vec<String> = keys
            .iter()
            .map(|k| match m.ticket(k) {
                Some(t) => format!("{k} {}", t.line()),
                None => format!("{k} (not in Jira)"),
            })
            .collect();
        if cells.is_empty() {
            String::new()
        } else {
            format!("  [{}]", cells.join("; "))
        }
    };
    let links = |i: &kiln_model::Item| -> serde_json::Map<String, serde_json::Value> {
        let mut out = serde_json::Map::new();
        for l in &i.links {
            let entry = out.entry(l.role.clone()).or_insert_with(|| json!([]));
            if let serde_json::Value::Array(a) = entry {
                a.push(json!(l.target.raw));
            }
        }
        out
    };
    match format {
        TextOrJson::Json => {
            let array: Vec<_> = items
                .iter()
                .map(|i| {
                    let mut v = json!({
                        "id": i.id.to_string(),
                        "type": i.type_name,
                        "title": i.title,
                        "status": i.status,
                        "parent": i.parent.as_ref().map(|r| r.raw.clone()),
                        "tags": i.tags,
                        "jira_keys": i.jira,
                        "owner": i.owner,
                        "due": i.due.map(|d| d.to_string()),
                        "done": p.is_done(i),
                        "closed": p.is_closed(i),
                        "links": links(i),
                        "path": i.path,
                        "line": i.id_span.line,
                    });
                    if body {
                        v["body"] = json!(i.body);
                    }
                    if mirror.is_some() {
                        v["jira"] = json!(tickets(&i.jira));
                    }
                    v
                })
                .collect();
            print!("{}", json_out(&json!(array)));
        }
        TextOrJson::Text => {
            let id_width = items
                .iter()
                .map(|i| i.id.to_string().len())
                .max()
                .unwrap_or(0);
            let status_width = items
                .iter()
                .map(|i| i.status.chars().count())
                .max()
                .unwrap_or(0);
            for i in &items {
                println!(
                    "{:id_width$}  {:status_width$}  {}{}",
                    i.id.to_string(),
                    i.status,
                    i.title,
                    column(&i.jira)
                );
            }
        }
    }
    Ok(0)
}
