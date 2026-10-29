//! `kiln trace`: per non-closed item of a type, what covers it for each
//! coverage role, and the gaps; with `--levels`, the requirement levels
//! as chains from the top level down (`Project::level_trace`).

use std::path::Path;

use kiln_index::{LevelNode, Project};
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
        None if !p.config.levels().is_empty() => p.config.levels()[0].to_owned(),
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
                    let gap = r.gaps.iter().any(|g| g == role);
                    let value = if !sources.is_empty() && gap {
                        format!("{} — gap", sources.join(", "))
                    } else if !sources.is_empty() {
                        sources.join(", ")
                    } else if gap {
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
                .filter(|l| l.role == *role)
                .filter_map(|l| {
                    // A test the results say failed or didn't run is shown,
                    // marked, though it doesn't count (K18).
                    let verdict = match l.source {
                        kiln_index::Source::Code(i) => p.verdict(i),
                        kiln_index::Source::Item(_) => None,
                    };
                    match verdict {
                        Some(kiln_index::Verdict::Failed) => {
                            Some(format!("{} (failed)", p.describe(l.source)))
                        }
                        Some(kiln_index::Verdict::NotRun) => {
                            Some(format!("{} (didn't run)", p.describe(l.source)))
                        }
                        Some(kiln_index::Verdict::Passed) => {
                            Some(format!("{} (passed)", p.describe(l.source)))
                        }
                        None => p.source_is_open(l.source).then(|| p.describe(l.source)),
                    }
                })
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

/// Per coverage role, the open sources; then the roles with a gap.
type Coverage = (Vec<(String, Vec<String>)>, Vec<String>);
/// An item's [`Coverage`] at its level.
type CoverageOf<'a> = dyn Fn(&Item, usize) -> Coverage + 'a;

/// `kiln trace --levels`.
pub fn run_levels(project: Option<&Path>, format: TraceFormat) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let config = &p.config;
    if config.trace().is_none() {
        return Err(Fail::usage(
            "kiln.toml declares one requirement level; list more under [requirements] levels, with the role that traces them (FORMAT.md, \"Requirement levels\")",
        ));
    }
    let trace = p.level_trace();
    let lowest = trace.levels.len() - 1;
    let trace_role = config.trace_role.as_str();
    let title = |name: &str| {
        config
            .type_named(name)
            .map_or(name.to_owned(), |t| t.title.clone())
    };
    let inverse = |r: &str| {
        config
            .role(r)
            .map(|role| role.inverse.clone())
            .unwrap_or_default()
    };
    // Per item: (role, open sources) for each coverage role that can point
    // at its type, and the roles it has a gap in, refinement included.
    let coverage = |item: &Item, level: usize| {
        let roles: Vec<&str> = config
            .coverage_roles
            .iter()
            .map(String::as_str)
            .filter(|r| *r != trace_role)
            .filter(|r| {
                config
                    .role(r)
                    .is_some_and(|role| role.allows_to(&item.type_name))
            })
            .collect();
        let row = row(p, item, &roles);
        let mut gaps = row.gaps;
        if level < lowest && !p.covered(&item.id, trace_role) {
            gaps.push(trace_role.to_owned());
        }
        let sources: Vec<(String, Vec<String>)> = roles
            .iter()
            .map(|r| (*r).to_owned())
            .zip(row.sources)
            .collect();
        (sources, gaps)
    };
    let commit = kiln_git::head_commit(loaded.source.root()).ok();
    let sections: [(&str, &[LevelNode]); 3] = [
        ("chains", &trace.roots),
        ("derived", &trace.derived),
        ("untraced", &trace.untraced),
    ];

    match format {
        TraceFormat::Json => {
            fn node_json(n: &LevelNode, coverage: &CoverageOf) -> serde_json::Value {
                let (sources, gaps) = coverage(n.item, n.level);
                let covered: serde_json::Map<String, serde_json::Value> =
                    sources.into_iter().map(|(r, s)| (r, json!(s))).collect();
                json!({
                    "id": n.item.id.to_string(),
                    "type": n.item.type_name,
                    "title": n.item.title,
                    "status": n.item.status,
                    "level": n.level + 1,
                    "derived": n.item.derived.is_some(),
                    "coverage": covered,
                    "gaps": gaps,
                    "below": n.below.iter().map(|b| node_json(b, coverage)).collect::<Vec<_>>(),
                })
            }
            let mut out = serde_json::Map::new();
            out.insert("commit".into(), json!(commit));
            out.insert("levels".into(), json!(trace.levels));
            out.insert("trace".into(), json!(trace_role));
            for (name, nodes) in sections {
                out.insert(
                    name.into(),
                    json!(nodes
                        .iter()
                        .map(|n| node_json(n, &coverage))
                        .collect::<Vec<_>>()),
                );
            }
            print!("{}", json_out(&serde_json::Value::Object(out)));
        }
        TraceFormat::Csv => {
            // A traceability matrix: one row per chain, one column per
            // level, then what covers the row's last requirement.
            let roles: Vec<&str> = config
                .coverage_roles
                .iter()
                .map(String::as_str)
                .filter(|r| *r != trace_role)
                .collect();
            let mut header: Vec<String> = trace.levels.iter().map(|l| title(l)).collect();
            header.extend(roles.iter().map(|r| inverse(r)));
            header.push("gaps".to_owned());
            println!(
                "{}",
                header.iter().map(|h| csv(h)).collect::<Vec<_>>().join(",")
            );
            for (name, nodes) in sections {
                for path in nodes.iter().flat_map(LevelNode::paths) {
                    let mut fields = vec![String::new(); trace.levels.len()];
                    let first_level = config.level_of(&path[0].type_name).unwrap_or(0);
                    if name == "derived" {
                        for f in fields.iter_mut().take(first_level) {
                            *f = "derived".to_owned();
                        }
                    }
                    for item in &path {
                        if let Some(l) = config.level_of(&item.type_name) {
                            fields[l] = item.id.to_string();
                        }
                    }
                    let leaf = path[path.len() - 1];
                    let level = config.level_of(&leaf.type_name).unwrap_or(0);
                    let (sources, mut gaps) = coverage(leaf, level);
                    if name == "untraced" {
                        gaps.insert(0, "untraced".to_owned());
                    }
                    for role in &roles {
                        fields.push(
                            sources
                                .iter()
                                .find(|(r, _)| r == role)
                                .map(|(_, s)| s.join("; "))
                                .unwrap_or_default(),
                        );
                    }
                    fields.push(gaps.join("; "));
                    println!(
                        "{}",
                        fields.iter().map(|f| csv(f)).collect::<Vec<_>>().join(",")
                    );
                }
            }
        }
        TraceFormat::Text => {
            let at = commit
                .as_deref()
                .map(|c| format!(" at {}", &c[..c.len().min(12)]))
                .unwrap_or_default();
            let names: Vec<String> = trace.levels.iter().map(|l| title(l)).collect();
            println!("Requirement levels{at}: {}", names.join(" → "));
            let mut gaps_total = 0;
            fn print_node(
                n: &LevelNode,
                depth: usize,
                coverage: &CoverageOf,
                inverse: &dyn Fn(&str) -> String,
                gaps_total: &mut usize,
            ) {
                let indent = "  ".repeat(depth);
                let derived = if n.item.derived.is_some() {
                    ", derived"
                } else {
                    ""
                };
                println!(
                    "{indent}{}  {}  ({}{derived})",
                    n.item.id, n.item.title, n.item.status
                );
                let (sources, gaps) = coverage(n.item, n.level);
                for (role, sources) in &sources {
                    let value = if !sources.is_empty() {
                        sources.join(", ")
                    } else if gaps.contains(role) {
                        "— gap".to_owned()
                    } else {
                        "(through its children)".to_owned()
                    };
                    println!("{indent}    {}: {value}", inverse(role));
                }
                for gap in gaps
                    .iter()
                    .filter(|g| !sources.iter().any(|(r, _)| r == *g))
                {
                    println!("{indent}    {}: — gap", inverse(gap));
                }
                *gaps_total += gaps.len();
                for b in &n.below {
                    print_node(b, depth + 1, coverage, inverse, gaps_total);
                }
            }
            for (name, nodes) in sections {
                if nodes.is_empty() {
                    continue;
                }
                println!();
                match name {
                    "derived" => println!("Derived (traced to no level above, on purpose)"),
                    "untraced" => println!("Not traced to the level above"),
                    _ => {}
                }
                for n in nodes {
                    let depth = usize::from(name != "chains");
                    print_node(n, depth, &coverage, &inverse, &mut gaps_total);
                }
            }
            let counts: Vec<String> = trace
                .levels
                .iter()
                .map(|l| {
                    let n = p
                        .items
                        .iter()
                        .filter(|i| i.type_name == *l && !p.is_closed(i))
                        .count();
                    let plural = config
                        .type_named(l)
                        .map_or(l.to_string(), |t| t.plural.clone());
                    format!("{n} {}", plural.to_lowercase())
                })
                .collect();
            println!();
            println!(
                "{}; {} derived, {} not traced, {gaps_total} gaps",
                counts.join(", "),
                trace.derived.len(),
                trace.untraced.len()
            );
        }
    }
    Ok(0)
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
