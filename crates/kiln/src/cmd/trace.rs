//! `kiln trace`: per non-closed item of a type, what covers it for each
//! coverage role, and the gaps.

use std::path::Path;

use kiln_index::Project;
use kiln_model::Item;
use serde_json::json;

use super::json_out;
use crate::project::load;
use crate::{Fail, TraceFormat};

struct Row<'a> {
    item: &'a Item,
    /// Per coverage role: the open sources linking with it.
    sources: Vec<Vec<String>>,
    /// Coverage roles not covered.
    gaps: Vec<String>,
}

pub fn run(
    project: Option<&Path>,
    type_name: Option<String>,
    format: TraceFormat,
) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let type_name = match type_name {
        Some(t) => t,
        None if p.config.type_named("requirement").is_some() => "requirement".to_owned(),
        None => p
            .config
            .types
            .first()
            .map(|t| t.name.clone())
            .ok_or_else(|| Fail::usage("kiln.toml declares no types"))?,
    };
    let ty = p
        .config
        .type_named(&type_name)
        .ok_or_else(|| Fail::usage(format!("`{type_name}` isn't a type")))?;
    let roles: Vec<&str> = p
        .config
        .coverage_roles
        .iter()
        .filter(|r| {
            p.config
                .role(r)
                .is_some_and(|role| role.allows_to(&ty.name))
        })
        .map(String::as_str)
        .collect();
    let rows: Vec<Row> = p
        .items
        .iter()
        .filter(|i| i.type_name == ty.name && !ty.is_closed(&i.status))
        .map(|item| row(p, item, &roles))
        .collect();
    let commit = kiln_git::head_commit(loaded.source.root()).ok();
    let inverse = |r: &str| {
        p.config
            .role(r)
            .map(|role| role.inverse.clone())
            .unwrap_or_default()
    };

    match format {
        TraceFormat::Json => {
            let items: Vec<_> = rows
                .iter()
                .map(|r| {
                    let mut covered = serde_json::Map::new();
                    for (role, sources) in roles.iter().zip(&r.sources) {
                        covered.insert((*role).to_owned(), json!(sources));
                    }
                    json!({
                        "id": r.item.id.to_string(),
                        "title": r.item.title,
                        "status": r.item.status,
                        "coverage": covered,
                        "gaps": r.gaps,
                    })
                })
                .collect();
            print!(
                "{}",
                json_out(
                    &json!({ "commit": commit, "type": ty.name, "roles": roles, "items": items })
                )
            );
        }
        TraceFormat::Csv => {
            let mut header = vec!["id".to_owned(), "title".to_owned(), "status".to_owned()];
            header.extend(roles.iter().map(|r| inverse(r)));
            header.push("gaps".to_owned());
            println!(
                "{}",
                header.iter().map(|h| csv(h)).collect::<Vec<_>>().join(",")
            );
            for r in &rows {
                let mut fields = vec![
                    r.item.id.to_string(),
                    r.item.title.clone(),
                    r.item.status.clone(),
                ];
                fields.extend(r.sources.iter().map(|s| s.join("; ")));
                fields.push(r.gaps.join("; "));
                println!(
                    "{}",
                    fields.iter().map(|f| csv(f)).collect::<Vec<_>>().join(",")
                );
            }
        }
        TraceFormat::Text => {
            let at = commit
                .as_deref()
                .map(|c| format!(" at {}", &c[..c.len().min(12)]))
                .unwrap_or_default();
            println!("{} coverage{at}", ty.plural);
            let id_width = rows
                .iter()
                .map(|r| r.item.id.to_string().len())
                .max()
                .unwrap_or(0);
            for r in &rows {
                println!();
                println!(
                    "{:id_width$}  {}  ({})",
                    r.item.id.to_string(),
                    r.item.title,
                    r.item.status
                );
                for (role, sources) in roles.iter().zip(&r.sources) {
                    let value = if !sources.is_empty() {
                        sources.join(", ")
                    } else if r.gaps.iter().any(|g| g == role) {
                        "— gap".to_owned()
                    } else {
                        "(through its children)".to_owned()
                    };
                    println!("{:id_width$}  {}: {value}", "", inverse(role));
                }
            }
            let totals: Vec<String> = roles
                .iter()
                .map(|role| {
                    let n = rows
                        .iter()
                        .filter(|r| !r.gaps.iter().any(|g| g == role))
                        .count();
                    format!("{role} {n}/{}", rows.len())
                })
                .collect();
            println!();
            println!(
                "{} {}; covered: {}",
                rows.len(),
                ty.plural.to_lowercase(),
                totals.join(", ")
            );
        }
    }
    Ok(0)
}

fn row<'a>(p: &Project, item: &'a Item, roles: &[&str]) -> Row<'a> {
    let inbound = p.inbound(&item.id);
    let sources = roles
        .iter()
        .map(|role| {
            inbound
                .iter()
                .filter(|l| l.role == *role && p.source_is_open(l.source))
                .map(|l| p.describe(l.source))
                .collect()
        })
        .collect();
    let gaps = roles
        .iter()
        .filter(|r| !p.covered(&item.id, r))
        .map(|r| (*r).to_owned())
        .collect();
    Row {
        item,
        sources,
        gaps,
    }
}

/// A CSV field, quoted when it needs to be (RFC 4180).
fn csv(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::csv;

    #[test]
    fn csv_quotes_only_when_needed() {
        assert_eq!(csv("plain"), "plain");
        assert_eq!(csv("a, \"b\""), "\"a, \"\"b\"\"\"");
    }
}
