//! `kiln renumber`: moving an item to another ID and rewriting every
//! reference Kiln counts (front matter, body mentions, code references),
//! and nothing else. Computed here as a plan of byte-range edits; reading
//! fresh copies and writing them is the caller's.
//!
//! The cure for two branches that allocated the same number. When the ID
//! is duplicated, references can't say which of the two they meant, so
//! only the chosen item's own ID and file name move, and the references
//! are listed for a person to check.

use std::collections::BTreeMap;
use std::ops::Range;

use kiln_model::{ItemId, Span};

use crate::project::Project;

/// Edits to one file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileEdits {
    /// Byte ranges in the file's normalised text (item files) or raw text
    /// (code files), and their replacement.
    pub edits: Vec<(Range<usize>, String)>,
    /// A code file: edits apply to raw bytes, not normalised text.
    pub code: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenumberPlan {
    pub from: ItemId,
    pub to: ItemId,
    /// The moved item's file.
    pub item_path: String,
    /// Its new name, when the old one started with the old ID.
    pub new_path: Option<String>,
    /// By path.
    pub files: BTreeMap<String, FileEdits>,
    /// References not rewritten, as `path:line`, with why.
    pub left_alone: Vec<String>,
}

/// Plans moving `from` to `to`. `file` picks the item when `from` is
/// duplicated (and must name one of them if given).
// implements: REQ-015
pub fn plan_renumber(
    project: &Project,
    from: &ItemId,
    to: &ItemId,
    file: Option<&str>,
) -> Result<RenumberPlan, String> {
    let candidates = project.indices(from);
    if candidates.is_empty() {
        return Err(format!("{from} doesn't exist"));
    }
    if to.prefix() != from.prefix() {
        return Err(format!(
            "{to} has another prefix than {from}; an item keeps its type's prefix"
        ));
    }
    if let Some(existing) = project.item(to) {
        return Err(format!("{to} already exists ({})", existing.path));
    }
    let chosen = match file {
        Some(f) => {
            let f = f.replace('\\', "/");
            *candidates
                .iter()
                .find(|&&i| project.items[i].path == f)
                .ok_or_else(|| format!("{f} isn't the file of {from}"))?
        }
        None if candidates.len() > 1 => {
            let paths: Vec<&str> = candidates
                .iter()
                .map(|&i| project.items[i].path.as_str())
                .collect();
            return Err(format!(
                "{from} is in {} files ({}); pick one with --file",
                paths.len(),
                paths.join(", ")
            ));
        }
        None => candidates[0],
    };
    let duplicated = candidates.len() > 1;
    let item = &project.items[chosen];
    let new_text = to.to_string();
    let mut files: BTreeMap<String, FileEdits> = BTreeMap::new();
    let mut left_alone = Vec::new();
    let mut edit = |path: &str, span: Span, code: bool| {
        let entry = files.entry(path.to_owned()).or_default();
        entry.code = code;
        entry.edits.push((span.start..span.end, new_text.clone()));
    };

    edit(&item.path, item.id_span, false);

    for other in &project.items {
        let mut refs: Vec<Span> = Vec::new();
        if let Some(parent) = &other.parent {
            if parent.id.as_ref() == Ok(from) {
                refs.push(parent.span);
            }
        }
        refs.extend(
            other
                .links
                .iter()
                .filter(|l| l.target.id.as_ref() == Ok(from))
                .map(|l| l.target.span),
        );
        refs.extend(
            other
                .mentions
                .iter()
                .filter(|m| m.target.id.as_ref() == Ok(from))
                .map(|m| m.target.span),
        );
        for span in refs {
            if duplicated {
                left_alone.push(format!(
                    "{}:{} (may mean either {from})",
                    other.path, span.line
                ));
            } else {
                edit(&other.path, span, false);
            }
        }
    }
    for code in &project.code_refs {
        if code.target.id.as_ref() != Ok(from) {
            continue;
        }
        if duplicated {
            left_alone.push(format!("{} (may mean either {from})", code.location()));
        } else if project.lossy_code_files.contains(&code.path) {
            left_alone.push(format!("{} (the file isn't UTF-8)", code.location()));
        } else {
            edit(&code.path, code.target.span, true);
        }
    }

    let new_path = {
        let (dir, name) = match item.path.rsplit_once('/') {
            Some((d, n)) => (format!("{d}/"), n),
            None => (String::new(), item.path.as_str()),
        };
        name.strip_prefix(&from.to_string())
            .map(|rest| format!("{dir}{to}{rest}"))
    };
    for f in files.values_mut() {
        f.edits.sort_by_key(|(r, _)| r.start);
        f.edits.dedup();
    }
    Ok(RenumberPlan {
        from: from.clone(),
        to: to.clone(),
        item_path: item.path.clone(),
        new_path,
        files,
        left_alone,
    })
}
