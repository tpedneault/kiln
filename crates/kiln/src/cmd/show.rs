//! `kiln show`: one item with everything computed about it: links both
//! ways (inverses from items and code), children, mentions, coverage,
//! milestone progress.

use std::path::Path;

use kiln_index::{Project, Source};
use kiln_model::Item;
use serde_json::{json, Map, Value};

use super::{existing, json_out, labelled, parse_id};
use crate::project::load;
use crate::{Fail, TextOrJson};

pub fn run(project: Option<&Path>, id: &str, format: TextOrJson) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let id = parse_id(id)?;
    let item = existing(p, &id)?;
    let rows = rows(p, item);
    match format {
        TextOrJson::Text => {
            println!("{}", labelled(item));
            println!(
                "{} · {} · {}:{}",
                item.type_name, item.status, item.path, item.id_span.line
            );
            let width = rows
                .iter()
                .map(|(k, _)| k.chars().count())
                .max()
                .unwrap_or(0);
            for (key, values) in &rows {
                println!("{key:width$}  {}", values.join(", "));
            }
        }
        TextOrJson::Json => {
            let mut map = Map::new();
            map.insert("id".into(), json!(item.id.to_string()));
            map.insert("type".into(), json!(item.type_name));
            map.insert("title".into(), json!(item.title));
            map.insert("status".into(), json!(item.status));
            map.insert("path".into(), json!(item.path));
            map.insert("line".into(), json!(item.id_span.line));
            for (key, values) in rows {
                map.insert(key, json!(values));
            }
            print!("{}", json_out(&Value::Object(map)));
        }
    }
    Ok(0)
}

/// Each row a label and its values; empty rows left out.
fn rows(p: &Project, item: &Item) -> Vec<(String, Vec<String>)> {
    let mut rows: Vec<(String, Vec<String>)> = Vec::new();
    let mut push = |key: &str, values: Vec<String>| {
        if !values.is_empty() {
            rows.push((key.to_owned(), values));
        }
    };
    let others: Vec<String> = p
        .indices(&item.id)
        .iter()
        .map(|&i| &p.items[i])
        .filter(|i| i.path != item.path)
        .map(|i| i.path.clone())
        .collect();
    push("also in", others);
    let describe = |r: &kiln_model::Ref| match r.id.as_ref().ok().and_then(|id| p.item(id)) {
        Some(target) => labelled(target),
        None => format!("{} (doesn't exist)", r.raw),
    };
    push("parent", item.parent.iter().map(describe).collect());
    push(
        "children",
        p.children(&item.id).into_iter().map(labelled).collect(),
    );
    push("tags", item.tags.clone());
    push("jira", item.jira.clone());
    push("owner", item.owner.iter().cloned().collect());
    push("due", item.due.iter().map(ToString::to_string).collect());

    let mut roles: Vec<&str> = item.links.iter().map(|l| l.role.as_str()).collect();
    roles.dedup();
    for role in roles {
        let targets = item
            .links
            .iter()
            .filter(|l| l.role == role)
            .map(|l| describe(&l.target))
            .collect();
        push(role, targets);
    }
    let inbound = p.inbound(&item.id);
    let mut inbound_roles: Vec<&str> = p.config.links.iter().map(|r| r.name.as_str()).collect();
    for link in inbound {
        if !inbound_roles.contains(&link.role.as_str()) {
            inbound_roles.push(&link.role);
        }
    }
    for role in inbound_roles {
        let label = p
            .config
            .role(role)
            .map(|r| r.inverse.clone())
            .unwrap_or_else(|| format!("{role} (from)"));
        let sources = inbound
            .iter()
            .filter(|l| l.role == role)
            .map(|l| match l.source {
                Source::Item(i) => labelled(&p.items[i]),
                Source::Code(_) => p.describe(l.source),
            })
            .collect();
        push(&label, sources);
    }
    push(
        "mentioned by",
        p.mentioned_by(&item.id).into_iter().map(labelled).collect(),
    );

    if item.type_name == "requirement"
        || p.config.coverage_roles.iter().any(|r| {
            p.config
                .role(r)
                .is_some_and(|role| role.allows_to(&item.type_name))
        })
    {
        let coverage = p
            .config
            .coverage_roles
            .iter()
            .filter(|r| {
                p.config
                    .role(r)
                    .is_some_and(|role| role.allows_to(&item.type_name))
            })
            .map(|r| format!("{r}: {}", if p.covered(&item.id, r) { "yes" } else { "no" }))
            .collect();
        push("coverage", coverage);
    }
    if let Some(progress) = p.progress(&item.id) {
        push(
            "progress",
            vec![format!("{} of {} done", progress.done, progress.total)],
        );
    }
    rows
}
