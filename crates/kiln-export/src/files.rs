//! The file destination (EXPORT.md, "Running exports"): a file is
//! written only when its bytes change, atomically; files a target wrote
//! earlier and no longer produces are removed, tracked in a `.kiln-export`
//! list beside them; files Kiln didn't write are never removed.
//!
//! Planning is pure (desired files against what the file source holds);
//! [`apply`] is the one place this crate writes.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use kiln_index::FileSource;
use kiln_model::{ExportTarget, Layout};

/// The name of the list of files Kiln wrote, beside them.
pub const MANIFEST: &str = ".kiln-export";

/// What exporting a file target would change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilePlan {
    /// New or changed files: (project-relative path, bytes, new).
    pub writes: Vec<(String, Vec<u8>, bool)>,
    pub unchanged: Vec<String>,
    /// Files this target wrote before and no longer produces.
    pub removes: Vec<String>,
    /// The `.kiln-export` list, when it changes: `Some(None)` removes it.
    pub manifest: Option<(String, Option<Vec<u8>>)>,
}

impl FilePlan {
    /// Whether exporting would change anything (`--check`).
    pub fn changes(&self) -> bool {
        !self.writes.is_empty() || !self.removes.is_empty() || self.manifest.is_some()
    }
}

/// The folder holding a target's files (and its `.kiln-export`).
pub fn output_dir(target: &ExportTarget) -> String {
    let path = target.path.clone().unwrap_or_default();
    match target.layout {
        Layout::Document => path
            .rsplit_once('/')
            .map(|(d, _)| d.to_owned())
            .unwrap_or_default(),
        _ => path,
    }
}

/// Compares `files` (what the target produces now) with `source`.
pub fn plan_files(
    target: &ExportTarget,
    files: Vec<(String, Vec<u8>)>,
    source: &dyn FileSource,
) -> FilePlan {
    let mut plan = FilePlan::default();
    let produced: BTreeSet<String> = files.iter().map(|(p, _)| p.clone()).collect();
    for (path, bytes) in files {
        match source.read(&path) {
            Ok(existing) if existing == bytes => plan.unchanged.push(path),
            Ok(_) => plan.writes.push((path, bytes, false)),
            Err(_) => plan.writes.push((path, bytes, true)),
        }
    }

    let dir = output_dir(target);
    let manifest_path = if dir.is_empty() {
        MANIFEST.to_owned()
    } else {
        format!("{dir}/{MANIFEST}")
    };
    let old_text = source
        .read(&manifest_path)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned());
    let mut others: Vec<String> = Vec::new();
    let mut previous: BTreeSet<String> = BTreeSet::new();
    for line in old_text.as_deref().unwrap_or("").lines() {
        match line.split_once('\t') {
            Some((name, path)) if name == target.name => {
                previous.insert(path.to_owned());
            }
            Some(_) => others.push(line.to_owned()),
            None => {}
        }
    }
    for stale in previous.difference(&produced) {
        if source.exists(stale) {
            plan.removes.push(stale.clone());
        }
    }
    // A single file needs no list: nothing of it can go stale but itself.
    let mut lines = others;
    if produced.len() > 1 {
        lines.extend(produced.iter().map(|p| format!("{}\t{p}", target.name)));
    }
    lines.sort();
    let new_text = if lines.is_empty() {
        None
    } else {
        Some(format!("{}\n", lines.join("\n")))
    };
    if new_text != old_text {
        plan.manifest = Some((manifest_path, new_text.map(String::into_bytes)));
    }
    plan
}

/// Carries out `plan` under `root`: writes atomically, removes stale
/// files, updates the list.
pub fn apply(root: &Path, plan: &FilePlan) -> io::Result<()> {
    for (path, bytes, _) in &plan.writes {
        let full = root.join(path);
        if let Some(dir) = full.parent() {
            std::fs::create_dir_all(dir)?;
        }
        fenix_storage::write(&full, bytes)?;
    }
    for path in &plan.removes {
        match std::fs::remove_file(root.join(path)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    if let Some((path, contents)) = &plan.manifest {
        let full = root.join(path);
        match contents {
            Some(bytes) => {
                if let Some(dir) = full.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                fenix_storage::write(&full, bytes)?;
            }
            None => match std::fs::remove_file(&full) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                _ => {}
            },
        }
    }
    Ok(())
}
