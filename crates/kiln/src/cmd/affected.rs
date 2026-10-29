//! `kiln affected [REV]`: the items a change reaches (K18, REQ-023), from
//! the lines `git diff` says changed since `REV` (default `HEAD`: what
//! isn't committed yet) to the items whose code or file changed
//! (`Project::affected`, ADR-022), each with what verifies it, to
//! re-check. In CI, `kiln affected origin/main --format github` puts a
//! notice on each changed reference, so a reviewer sees which
//! requirements a code change touches without opening Kiln.

use std::path::Path;

use kiln_index::{Project, Source, Touch};
use serde_json::{json, Value};

use super::{json_out, labelled};
use crate::project::load;
use crate::{AffectedFormat, Fail};

// implements: REQ-023
pub fn run(project: Option<&Path>, rev: &str, format: AffectedFormat) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let root = loaded.source.path("");
    let commit = kiln_git::resolve(&root, rev).map_err(|e| Fail::usage(e.to_string()))?;
    let changed = kiln_git::changed_lines(&root, &commit)
        .map_err(|e| Fail::usage(format!("git diff {rev}: {e}")))?;
    let reached = p.affected(&changed);
    let short = &commit[..commit.len().min(7)];
    let recheck = |s: &Source| -> Value {
        match s {
            Source::Item(i) => super::show::summary(&p.items[*i]),
            Source::Code(i) => {
                let c = &p.code_refs[*i];
                json!({ "path": c.path, "line": c.target.span.line })
            }
        }
    };
    match format {
        AffectedFormat::Json => {
            let items: Vec<Value> = reached
                .iter()
                .map(|r| {
                    let mut v = super::show::summary(r.item);
                    v["type"] = json!(r.item.type_name);
                    v["changed"] =
                        json!(r
                        .touches
                        .iter()
                        .map(|t| match t {
                            Touch::Code { path, line, role, from, to } => json!({
                                "path": path, "line": line, "role": role, "from": from, "to": to,
                            }),
                            Touch::File { path } => json!({ "path": path, "file": true }),
                        })
                        .collect::<Vec<_>>());
                    v["recheck"] = json!(r.recheck.iter().map(recheck).collect::<Vec<_>>());
                    v
                })
                .collect();
            print!(
                "{}",
                json_out(&json!({ "since": rev, "commit": commit, "items": items }))
            );
        }
        AffectedFormat::Github => {
            for r in &reached {
                let again = recheck_text(p, &r.recheck);
                for t in &r.touches {
                    let (path, line, what) = match t {
                        Touch::Code {
                            path, from, role, ..
                        } => (
                            path.as_str(),
                            *from,
                            format!("changes code that `{role}` {}", labelled(r.item)),
                        ),
                        Touch::File { path } => {
                            (path.as_str(), 1, format!("changes {}", labelled(r.item)))
                        }
                    };
                    println!(
                        "::notice file={path},line={line},title=Kiln affected::{}{}",
                        annotation(&what),
                        if again.is_empty() {
                            String::new()
                        } else {
                            annotation(&format!("; re-check {again}"))
                        }
                    );
                }
            }
        }
        AffectedFormat::Text => {
            for (n, r) in reached.iter().enumerate() {
                if n > 0 {
                    println!();
                }
                println!("{}  ({})", labelled(r.item), r.item.status);
                for t in &r.touches {
                    match t {
                        Touch::Code {
                            path,
                            line,
                            role,
                            from,
                            to,
                        } => {
                            let lines = if from == to {
                                format!("{from}")
                            } else {
                                format!("{from}-{to}")
                            };
                            println!("  changed   {path}:{lines} (under `{role}` at line {line})");
                        }
                        Touch::File { path } => println!("  changed   {path} (the item)"),
                    }
                }
                let again = recheck_text(p, &r.recheck);
                if !again.is_empty() {
                    println!("  re-check  {again}");
                }
            }
        }
    }
    let verb = if reached.len() == 1 { "item" } else { "items" };
    eprintln!(
        "kiln: since {rev} ({short}), {} {verb} reached",
        reached.len()
    );
    Ok(0)
}

fn recheck_text(p: &Project, sources: &[Source]) -> String {
    sources
        .iter()
        .map(|s| match s {
            Source::Item(i) => labelled(&p.items[*i]),
            Source::Code(_) => p.describe(*s),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// GitHub's workflow-command escaping for a message.
fn annotation(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}
