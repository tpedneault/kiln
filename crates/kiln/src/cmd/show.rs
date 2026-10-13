//! `kiln show`: one item with everything computed about it: links both
//! ways (inverses from items and code), children, mentions, coverage,
//! milestone progress.

use std::path::Path;

use serde_json::{json, Map, Value};

use super::{existing, json_out, labelled, parse_id};
use crate::project::load;
use crate::{Fail, TextOrJson};

pub fn run(project: Option<&Path>, id: &str, format: TextOrJson) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let id = parse_id(id)?;
    let item = existing(p, &id)?;
    let rows = p.facts(item);
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
