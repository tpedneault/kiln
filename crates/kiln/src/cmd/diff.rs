//! `kiln diff REV[..REV]`: the items added, removed and changed between
//! two revisions, or between a revision and the working tree (uncommitted
//! changes included), item by item (DESIGN.md, "Baseline").

use std::path::Path;

use kiln_index::{ChangeKind, ItemChange};
use serde_json::json;

use super::json_out;
use crate::project::{at_revision, load, root};
use crate::{Fail, TextOrJson};

pub fn run(
    project: Option<&Path>,
    range: &str,
    type_name: Option<String>,
    format: TextOrJson,
) -> Result<u8, Fail> {
    let root = root(project)?;
    let (from, to) = match range.split_once("..") {
        Some((a, b)) if !b.is_empty() => (a, Some(b)),
        Some((a, _)) => (a, None),
        None => (range, None),
    };
    let (old, old_commit) = at_revision(&root, from)?;
    let (new, to_label) = match to {
        Some(rev) => {
            let (p, c) = at_revision(&root, rev)?;
            (p, format!("{rev} ({})", short(&c)))
        }
        None => (load(Some(&root))?.project, "the working tree".to_owned()),
    };
    if let Some(t) = &type_name {
        if new.config.type_named(t).is_none() && old.config.type_named(t).is_none() {
            return Err(Fail::usage(format!("`{t}` isn't a type")));
        }
    }
    let changes: Vec<ItemChange> = kiln_index::diff(&old, &new)
        .into_iter()
        .filter(|c| type_name.as_ref().is_none_or(|t| &c.type_name == t))
        .collect();
    match format {
        TextOrJson::Json => {
            let items: Vec<_> = changes
                .iter()
                .map(|c| {
                    json!({
                        "id": c.id.to_string(), "change": c.kind.as_str(), "type": c.type_name,
                        "title": c.title, "path": c.path,
                        "details": c.details.iter().map(|d| d.describe()).collect::<Vec<_>>(),
                    })
                })
                .collect();
            print!(
                "{}",
                json_out(
                    &json!({ "from": from, "fromCommit": old_commit, "to": to.unwrap_or("working tree"), "items": items })
                )
            );
        }
        TextOrJson::Text => {
            println!("{from} ({}) → {to_label}", short(&old_commit));
            if changes.is_empty() {
                println!("no item changed");
            }
            for c in &changes {
                let mark = match c.kind {
                    ChangeKind::Added => '+',
                    ChangeKind::Removed => '-',
                    ChangeKind::Changed => '~',
                };
                let what = match c.kind {
                    ChangeKind::Changed => format!(": {}", c.summary()),
                    _ => format!(" ({})", c.type_name),
                };
                println!("{mark} {} {}{what}", c.id, c.title);
            }
            let count = |k| changes.iter().filter(|c| c.kind == k).count();
            println!(
                "{} added, {} changed, {} removed",
                count(ChangeKind::Added),
                count(ChangeKind::Changed),
                count(ChangeKind::Removed)
            );
        }
    }
    Ok(0)
}

fn short(commit: &str) -> &str {
    &commit[..commit.len().min(7)]
}
