//! `kiln renumber`: applies `kiln-index`'s plan. Every file is checked
//! unchanged since the index read it before any is written, so a
//! refusal leaves the project as it was; the item's file is renamed last.

use std::path::Path;

use kiln_index::plan_renumber;
use kiln_parse::splice;

use super::parse_id;
use crate::project::{load, write_back};
use crate::{Fail, EXIT_FINDINGS};

pub fn run(project: Option<&Path>, id: &str, to: &str, file: Option<&str>) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let source = &loaded.source;
    let from = parse_id(id)?;
    let to = parse_id(to)?;
    let plan = plan_renumber(p, &from, &to, file).map_err(Fail::usage)?;
    if let Some(new_path) = &plan.new_path {
        if source.path(new_path).exists() {
            return Err(Fail::usage(format!("{new_path} already exists")));
        }
    }

    // Read and edit everything first; write only if every file is as the
    // index saw it.
    let mut writes = Vec::new();
    for (rel, edits) in &plan.files {
        let path = source.path(rel);
        let stamp = source
            .stamp(rel)
            .ok_or_else(|| Fail::usage(format!("{rel} wasn't read while loading")))?;
        let bytes = std::fs::read(&path).map_err(|e| Fail::usage(format!("{rel}: {e}")))?;
        let current =
            kiln_index::Stamp::of(&path).map_err(|e| Fail::usage(format!("{rel}: {e}")))?;
        if current != stamp {
            return Err(Fail::new(
                EXIT_FINDINGS,
                format!(
                    "{rel} changed on disk while renumbering; nothing was written, run it again"
                ),
            ));
        }
        let text = if edits.code {
            String::from_utf8(bytes.clone())
                .map_err(|_| Fail::usage(format!("{rel} isn't UTF-8")))?
        } else {
            kiln_parse::normalize(&bytes)
                .ok_or_else(|| Fail::usage(format!("{rel} isn't UTF-8")))?
        };
        let in_bounds = edits.edits.iter().all(|(r, _)| {
            r.end <= text.len()
                && text.is_char_boundary(r.start)
                && text.is_char_boundary(r.end)
                && text[r.clone()] == plan.from.to_string()
        });
        if !in_bounds {
            return Err(Fail::new(
                EXIT_FINDINGS,
                format!("{rel} doesn't match what was indexed; nothing was written, run it again"),
            ));
        }
        let new = splice(&text, &edits.edits);
        writes.push((
            path,
            rel.clone(),
            bytes,
            stamp,
            new,
            !edits.code,
            edits.edits.len(),
        ));
    }
    let mut references = 0;
    for (path, rel, bytes, stamp, new, normalised, count) in &writes {
        write_back(path, rel, bytes, *stamp, new, *normalised)?;
        if *rel != plan.item_path {
            references += count;
        }
    }
    let files = writes.iter().filter(|w| w.1 != plan.item_path).count();
    let mut summary = format!(
        "{} → {}: {references} reference{} in {files} other file{}",
        plan.from,
        plan.to,
        if references == 1 { "" } else { "s" },
        if files == 1 { "" } else { "s" }
    );
    if let Some(new_path) = &plan.new_path {
        std::fs::rename(source.path(&plan.item_path), source.path(new_path)).map_err(|e| {
            Fail::usage(format!(
                "renumbered, but couldn't rename {} to {new_path}: {e}",
                plan.item_path
            ))
        })?;
        summary.push_str(&format!("; {} → {new_path}", plan.item_path));
    }
    println!("{summary}");
    for line in &plan.left_alone {
        eprintln!("kiln: left alone: {line}");
    }
    Ok(0)
}
