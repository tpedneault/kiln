//! `kiln impact`: everything that reaches an item by links or parent
//! edges, transitively, each with the path that reaches it.

use std::path::Path;

use kiln_index::{Project, Source, Step};
use serde_json::json;

use super::{existing, json_out, labelled, parse_id};
use crate::project::load;
use crate::{Fail, TextOrJson};

pub fn run(project: Option<&Path>, id: &str, format: TextOrJson) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let id = parse_id(id)?;
    existing(p, &id)?;
    let impact = p.impact(&id);
    match format {
        TextOrJson::Json => {
            let array: Vec<_> = impact
                .iter()
                .map(|i| {
                    let path: Vec<_> = i
                        .path
                        .iter()
                        .map(|s| json!({ "from": p.describe(s.from), "relation": s.relation, "to": s.to.to_string() }))
                        .collect();
                    json!({ "source": p.describe(i.source), "path": path })
                })
                .collect();
            print!("{}", json_out(&json!(array)));
        }
        TextOrJson::Text => {
            if impact.is_empty() {
                println!("nothing reaches {id}");
            }
            let width = impact
                .iter()
                .map(|i| p.describe(i.source).chars().count())
                .max()
                .unwrap_or(0);
            for i in &impact {
                println!(
                    "{:width$}  {}",
                    p.describe(i.source),
                    path_text(p, &i.path, i.source)
                );
            }
        }
    }
    Ok(0)
}

/// `verifies REQ-002, child of REQ-001`, with the source's title first
/// for items.
fn path_text(p: &Project, path: &[Step], source: Source) -> String {
    let steps: Vec<String> = path
        .iter()
        .map(|s| match s.relation.as_str() {
            "parent" => format!("child of {}", s.to),
            role => format!("{role} {}", s.to),
        })
        .collect();
    match source {
        Source::Item(i) => format!("{} — {}", steps.join(", "), labelled(&p.items[i])),
        Source::Code(_) => steps.join(", "),
    }
}
