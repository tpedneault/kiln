//! `kiln list`: items, filtered by type, status and tag.

use std::path::Path;

use serde_json::json;

use super::json_out;
use crate::project::load;
use crate::{Fail, TextOrJson};

pub fn run(
    project: Option<&Path>,
    type_name: Option<String>,
    status: Option<String>,
    tag: Option<String>,
    format: TextOrJson,
) -> Result<u8, Fail> {
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
    match format {
        TextOrJson::Json => {
            let array: Vec<_> = items
                .iter()
                .map(|i| {
                    json!({
                        "id": i.id.to_string(),
                        "type": i.type_name,
                        "title": i.title,
                        "status": i.status,
                        "parent": i.parent.as_ref().map(|r| r.raw.clone()),
                        "tags": i.tags,
                        "path": i.path,
                        "line": i.id_span.line,
                    })
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
                    "{:id_width$}  {:status_width$}  {}",
                    i.id.to_string(),
                    i.status,
                    i.title
                );
            }
        }
    }
    Ok(0)
}
