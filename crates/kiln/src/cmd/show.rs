//! `kiln show`: one item with everything computed about it: links both
//! ways (inverses from items and code), children, mentions, coverage,
//! milestone progress, and its body.
//!
//! The text form is for reading: [`Project::facts`]' rows, the same the
//! language server's hover shows, then the body. The JSON form is for
//! programs (coding agents first, REQ-020): every reference an object
//! with the target's ID, title and status, coverage as booleans, progress
//! as numbers, so nothing has to be parsed back out of a label.

use std::path::Path;

use kiln_index::{Project, Source};
use kiln_model::{Item, Ref};
use serde_json::{json, Map, Value};

use super::{existing, json_out, labelled, parse_id};
use crate::project::load;
use crate::{Fail, TextOrJson};

pub fn run(project: Option<&Path>, id: &str, format: TextOrJson) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let id = parse_id(id)?;
    let item = existing(p, &id)?;
    match format {
        TextOrJson::Text => {
            let rows = p.facts(item);
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
            let body = item.body.trim();
            if !body.is_empty() {
                println!("\n{body}");
            }
        }
        TextOrJson::Json => print!("{}", json_out(&item_json(p, item))),
    }
    Ok(0)
}

/// An item as `{id, title, status}`, what every reference in the JSON is.
pub fn summary(item: &Item) -> Value {
    json!({ "id": item.id.to_string(), "title": item.title, "status": item.status })
}

/// A reference as written: the item it names, or `{id, missing: true}`.
fn reference(p: &Project, r: &Ref) -> Value {
    match r.id.as_ref().ok().and_then(|id| p.item(id)) {
        Some(target) => summary(target),
        None => json!({ "id": r.raw, "missing": true }),
    }
}

fn item_json(p: &Project, item: &Item) -> Value {
    let mut links = Map::new();
    for l in &item.links {
        let entry = links
            .entry(l.role.clone())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(a) = entry {
            a.push(reference(p, &l.target));
        }
    }
    let mut inbound = Map::new();
    for l in p.inbound(&item.id) {
        let value = match l.source {
            Source::Item(i) => summary(&p.items[i]),
            Source::Code(i) => {
                let c = &p.code_refs[i];
                json!({ "path": c.path, "line": c.target.span.line })
            }
        };
        let entry = inbound
            .entry(l.role.clone())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(a) = entry {
            a.push(value);
        }
    }
    let coverage: Map<String, Value> = p
        .config
        .coverage_roles
        .iter()
        .filter(|r| {
            p.config
                .role(r)
                .is_some_and(|role| role.allows_to(&item.type_name))
        })
        .map(|r| (r.clone(), json!(p.covered(&item.id, r))))
        .collect();
    let also_in: Vec<&str> = p
        .indices(&item.id)
        .iter()
        .map(|&i| p.items[i].path.as_str())
        .filter(|path| *path != item.path)
        .collect();
    let level = (p.config.levels().len() > 1)
        .then(|| p.config.level_of(&item.type_name))
        .flatten()
        .map(|l| l + 1);
    json!({
        "id": item.id.to_string(),
        "type": item.type_name,
        "title": item.title,
        "status": item.status,
        "done": p.is_done(item),
        "closed": p.is_closed(item),
        "path": item.path,
        "line": item.id_span.line,
        "also_in": also_in,
        "parent": item.parent.as_ref().map(|r| reference(p, r)),
        "children": p.children(&item.id).into_iter().map(summary).collect::<Vec<_>>(),
        "tags": item.tags,
        "jira": item.jira,
        "owner": item.owner,
        "due": item.due.map(|d| d.to_string()),
        "level": level,
        "derived": item.derived.is_some(),
        "links": links,
        "inbound": inbound,
        "mentioned_by": p.mentioned_by(&item.id).into_iter().map(summary).collect::<Vec<_>>(),
        "coverage": coverage,
        "progress": p.progress(&item.id).map(|g| json!({ "done": g.done, "total": g.total })),
        "body": item.body,
    })
}
