//! `kiln next` and `kiln status`: the plan from the command line
//! (REQ-020, ADR-018). `next` is the flat list of work ready to start,
//! earliest milestone first, what a coding agent asks before taking a
//! task; `status` is the plan by milestone, with what waits on what and
//! how `kiln check` finds the project, what a person asks at a stand-up.
//! Both print [`Project::plan`], so they never disagree.

use std::path::Path;

use kiln_check::{check, summary};
use kiln_index::{Group, Project};
use kiln_model::{Item, Severity};
use serde_json::{json, Value};

use super::show::summary as item_summary;
use super::{existing, json_out, labelled, parse_id};
use crate::project::{load, today};
use crate::{Fail, TextOrJson};

/// The plan, or why there is none to show: no `part-of` role.
fn plan(p: &Project) -> Result<Vec<Group<'_>>, Fail> {
    if p.work_types().is_empty() {
        return Err(Fail::usage(format!(
            "kiln.toml has no `{}` link role, so nothing is work in a milestone",
            kiln_index::PART_OF
        )));
    }
    Ok(p.plan())
}

/// `kiln next [--milestone ID]`.
// implements: REQ-020
pub fn next(
    project: Option<&Path>,
    milestone: Option<&str>,
    format: TextOrJson,
) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let mut groups = plan(p)?;
    if let Some(m) = milestone {
        let id = parse_id(m)?;
        let item = existing(p, &id)?;
        groups.retain(|g| g.milestone.is_some_and(|g| g.id == item.id));
    }
    let ready: Vec<(&Item, Option<&Item>)> = groups
        .iter()
        .flat_map(|g| g.ready.iter().map(move |i| (*i, g.milestone)))
        .collect();
    let waiting: usize = groups.iter().map(|g| g.waiting.len()).sum();
    match format {
        TextOrJson::Json => {
            let rows: Vec<Value> = ready
                .iter()
                .map(|(i, m)| {
                    let mut v = item_summary(i);
                    v["type"] = json!(i.type_name);
                    v["path"] = json!(i.path);
                    v["line"] = json!(i.id_span.line);
                    v["milestone"] = m.map_or(Value::Null, item_summary);
                    v
                })
                .collect();
            print!("{}", json_out(&json!(rows)));
        }
        TextOrJson::Text => {
            let id_w = width(ready.iter().map(|(i, _)| i.id.to_string()));
            let st_w = width(ready.iter().map(|(i, _)| i.status.clone()));
            for (i, m) in &ready {
                let place = m.map(|m| format!("  ({})", m.id)).unwrap_or_default();
                println!(
                    "{:id_w$}  {:st_w$}  {}{place}",
                    i.id.to_string(),
                    i.status,
                    i.title
                );
            }
        }
    }
    if ready.is_empty() {
        eprintln!(
            "kiln: no work is ready{}",
            match waiting {
                0 => String::new(),
                n => format!("; {n} waiting (`kiln status` shows on what)"),
            }
        );
    }
    Ok(0)
}

/// `kiln status`.
// implements: REQ-020
pub fn status(project: Option<&Path>, format: TextOrJson) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let groups = plan(p)?;
    let findings = check(p, &loaded.source, today());
    let count = |s: Severity| findings.iter().filter(|f| f.severity == s).count();
    match format {
        TextOrJson::Json => {
            let rows: Vec<Value> = groups
                .iter()
                .map(|g| {
                    json!({
                        "milestone": g.milestone.map(|m| {
                            let mut v = item_summary(m);
                            v["due"] = json!(m.due.map(|d| d.to_string()));
                            v
                        }),
                        "done": g.progress.done,
                        "total": g.progress.total,
                        "ready": g.ready.iter().map(|i| item_summary(i)).collect::<Vec<_>>(),
                        "waiting": g.waiting.iter().map(|w| {
                            let mut v = item_summary(w.item);
                            v["waits_on"] = json!(w.on.iter().map(|i| item_summary(i)).collect::<Vec<_>>());
                            v["blocked"] = json!(w.blocked);
                            v
                        }).collect::<Vec<_>>(),
                    })
                })
                .collect();
            print!(
                "{}",
                json_out(&json!({
                    "project": p.config.name,
                    "milestones": rows,
                    "findings": {
                        "errors": count(Severity::Error),
                        "warnings": count(Severity::Warning),
                    },
                }))
            );
        }
        TextOrJson::Text => {
            let rows: Vec<&Item> = groups
                .iter()
                .flat_map(|g| {
                    g.ready
                        .iter()
                        .copied()
                        .chain(g.waiting.iter().map(|w| w.item))
                })
                .collect();
            let id_w = width(rows.iter().map(|i| i.id.to_string()));
            let st_w = width(rows.iter().map(|i| i.status.clone()));
            for (n, g) in groups.iter().enumerate() {
                if n > 0 {
                    println!();
                }
                let progress = format!("{} of {} done", g.progress.done, g.progress.total);
                match g.milestone {
                    Some(m) => {
                        let due = m.due.map(|d| format!(" · due {d}")).unwrap_or_default();
                        println!("{} · {} · {progress}{due}", labelled(m), m.status);
                    }
                    None => println!("In no milestone · {progress}"),
                }
                for (k, i) in g.ready.iter().enumerate() {
                    let label = if k == 0 { "ready  " } else { "       " };
                    println!(
                        "  {label}  {:id_w$}  {:st_w$}  {}",
                        i.id.to_string(),
                        i.status,
                        i.title
                    );
                }
                for (k, w) in g.waiting.iter().enumerate() {
                    let label = if k == 0 { "waiting" } else { "       " };
                    let on: Vec<String> = w.on.iter().map(|i| i.id.to_string()).collect();
                    let why = match (w.blocked, on.is_empty()) {
                        (true, true) => "blocked".to_owned(),
                        (true, false) => format!("blocked; on {}", on.join(", ")),
                        _ => format!("on {}", on.join(", ")),
                    };
                    println!(
                        "  {label}  {:id_w$}  {:st_w$}  {}  ({why})",
                        w.item.id.to_string(),
                        w.item.status,
                        w.item.title,
                    );
                }
            }
            if groups.is_empty() {
                println!("No open milestones and no open work.");
            }
            println!("\nkiln check: {}", summary(&findings));
        }
    }
    Ok(0)
}

fn width(values: impl Iterator<Item = String>) -> usize {
    values.map(|s| s.chars().count()).max().unwrap_or(0)
}
