//! `kiln log ID`: an item's history from git (K19, REQ-024, ADR-023):
//! each commit that changed it, oldest first, with its date and author
//! and what changed (`kiln_index::item_details`, the same words as `kiln
//! diff`), then what isn't committed yet. Read from `git log --follow`
//! and the file at each commit; nothing is stored in the item.

use std::path::Path;

use kiln_index::{item_details, Project};
use kiln_model::Item;
use kiln_parse::{normalize, parse_item, ItemOptions};
use serde_json::json;

use super::{existing, json_out, labelled, parse_id};
use crate::project::load;
use crate::{Fail, TextOrJson};

/// One line of the history.
struct Event {
    commit: Option<String>,
    date: String,
    author: String,
    changes: Vec<String>,
}

fn parse(p: &Project, path: &str, bytes: &[u8]) -> Option<Item> {
    let prefixes = p.config.prefixes();
    let options = ItemOptions {
        prefixes: &prefixes,
        link_bare_ids: p.config.link_bare_ids,
    };
    parse_item(path, &normalize(bytes)?, &options).0
}

// implements: REQ-024
pub fn run(project: Option<&Path>, id: &str, format: TextOrJson) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let id = parse_id(id)?;
    let item = existing(p, &id)?;
    let root = loaded.source.path("");
    let history =
        kiln_git::file_history(&root, &item.path).map_err(|e| Fail::usage(e.to_string()))?;
    let mut events = Vec::new();
    let mut previous: Option<Item> = None;
    for rev in &history {
        let Some(bytes) = kiln_git::file_at(&root, &rev.commit, &rev.path)
            .map_err(|e| Fail::usage(e.to_string()))?
        else {
            continue;
        };
        let Some(now) = parse(p, &rev.path, &bytes) else {
            continue;
        };
        let changes: Vec<String> = match &previous {
            None => vec![format!("created ({})", now.status)],
            Some(before) => item_details(before, &now)
                .iter()
                .map(|d| d.describe())
                .collect(),
        };
        if !changes.is_empty() {
            events.push(Event {
                commit: Some(rev.commit.clone()),
                date: rev.date.clone(),
                author: rev.author.clone(),
                changes,
            });
        }
        previous = Some(now);
    }
    let uncommitted: Vec<String> = match &previous {
        None => vec![format!("created ({})", item.status)],
        Some(before) => item_details(before, item)
            .iter()
            .map(|d| d.describe())
            .collect(),
    };
    if !uncommitted.is_empty() {
        events.push(Event {
            commit: None,
            date: String::new(),
            author: String::new(),
            changes: uncommitted,
        });
    }
    match format {
        TextOrJson::Json => {
            let rows: Vec<serde_json::Value> = events
                .iter()
                .map(|e| {
                    json!({
                        "commit": e.commit,
                        "date": (!e.date.is_empty()).then_some(&e.date),
                        "author": (!e.author.is_empty()).then_some(&e.author),
                        "changes": e.changes,
                    })
                })
                .collect();
            print!(
                "{}",
                json_out(&json!({
                    "id": item.id.to_string(),
                    "title": item.title,
                    "path": item.path,
                    "events": rows,
                }))
            );
        }
        TextOrJson::Text => {
            println!("{}", labelled(item));
            let author_w = events
                .iter()
                .map(|e| e.author.chars().count())
                .max()
                .unwrap_or(0);
            for e in &events {
                let head = match &e.commit {
                    Some(c) => format!(
                        "{}  {}  {:author_w$}",
                        e.date,
                        &c[..c.len().min(7)],
                        e.author
                    ),
                    None => format!("{:<10}  {:7}  {:author_w$}", "uncommitted", "", ""),
                };
                println!("{head}  {}", e.changes.join("; "));
            }
        }
    }
    if kiln_git::is_shallow(&root).unwrap_or(false) {
        eprintln!(
            "kiln: this clone is shallow, so older history isn't here (`git fetch --unshallow`)"
        );
    }
    Ok(0)
}
