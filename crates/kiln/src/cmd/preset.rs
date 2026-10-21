//! `kiln preset list` and `kiln preset add NAME`: the document templates
//! Kiln ships (DESIGN.md, "Presets"), copied into a project with the types
//! and export target they need. `kiln init --preset NAME` is `init` then
//! `add`.
//!
//! Never overwrites: a file the preset would write that already exists
//! stops it before anything is written, and so does a result that `kiln
//! check` would find a new error in (an export or a template that can't
//! work). The merge into `kiln.toml` is `kiln_parse::add_preset`; this
//! module only reads, checks and writes.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use kiln_index::{DiskSource, FileSource, Listing, Project};
use kiln_model::{Rule, Severity, PRESETS};

use crate::project::{root, today};
use crate::Fail;

pub fn list() -> Result<u8, Fail> {
    let width = PRESETS.iter().map(|p| p.name.len()).max().unwrap_or(0);
    for p in PRESETS {
        println!("{:width$}  {}", p.name, p.summary);
    }
    Ok(0)
}

pub fn add(project: Option<&Path>, name: &str, dry_run: bool) -> Result<u8, Fail> {
    let root = root(project)?;
    add_at(&root, name, dry_run)
}

/// The preset called `name`, or an error listing the ones there are.
pub fn find(name: &str) -> Result<&'static kiln_model::Preset, Fail> {
    kiln_model::preset(name).ok_or_else(|| {
        let names: Vec<&str> = PRESETS.iter().map(|p| p.name).collect();
        Fail::usage(format!(
            "no preset `{name}`; the presets are {} (`kiln preset list`)",
            names.join(", ")
        ))
    })
}

/// The project's files with some replaced or added, for checking what
/// adding a preset would give before writing it.
struct Overlay<'a> {
    disk: &'a DiskSource,
    files: BTreeMap<String, Vec<u8>>,
}

impl FileSource for Overlay<'_> {
    fn list(&self, dirs: &[String], exclude: &[String], ext: Option<&str>) -> io::Result<Listing> {
        // The preset's files are templates without front matter: never
        // items, so listing the disk is enough.
        self.disk.list(dirs, exclude, ext)
    }

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        match self.files.get(path) {
            Some(bytes) => Ok(bytes.clone()),
            None => self.disk.read(path),
        }
    }

    fn exists(&self, path: &str) -> bool {
        self.files.contains_key(path) || self.disk.exists(path)
    }
}

/// The errors `kiln check` reports about exports and templates, without
/// positions, to compare before and after.
fn export_errors(files: &dyn FileSource) -> Vec<String> {
    let Ok(project) = Project::load(files) else {
        return Vec::new();
    };
    kiln_check::check(&project, files, today())
        .into_iter()
        .filter(|f| f.severity == Severity::Error && matches!(f.rule, Rule::K005 | Rule::K112))
        .map(|f| format!("{}: {} {}", f.path, f.rule, f.message))
        .collect()
}

pub fn add_at(root: &Path, name: &str, dry_run: bool) -> Result<u8, Fail> {
    let preset = find(name)?;
    let config_path = root.join(kiln_parse::CONFIG_FILE);
    let read = kiln_index::read_for_edit(&config_path)
        .map_err(|e| Fail::usage(format!("kiln.toml: {e}")))?;
    let text =
        kiln_parse::normalize(&read.bytes).ok_or_else(|| Fail::usage("kiln.toml isn't UTF-8"))?;
    let plan = kiln_parse::add_preset(&text, preset).map_err(|e| Fail::usage(e.to_string()))?;
    for (path, _) in &plan.files {
        if root.join(path).exists() {
            return Err(Fail::usage(format!(
                "{path} already exists; kiln preset add never overwrites (move it away, or rename it and add the preset again)"
            )));
        }
    }

    let disk = DiskSource::new(root);
    let mut files: BTreeMap<String, Vec<u8>> = plan
        .files
        .iter()
        .map(|(p, t)| (p.clone(), t.clone().into_bytes()))
        .collect();
    files.insert(
        kiln_parse::CONFIG_FILE.to_owned(),
        plan.config.clone().into_bytes(),
    );
    let before = export_errors(&disk);
    let after = export_errors(&Overlay { disk: &disk, files });
    if let Some(new) = after.iter().find(|e| !before.contains(e)) {
        return Err(Fail::usage(format!(
            "adding the {name} preset would fail `kiln check` ({new}); nothing was written"
        )));
    }

    let mut summary = Vec::new();
    if !plan.added_types.is_empty() {
        summary.push(format!("types {}", plan.added_types.join(", ")));
    }
    if !plan.kept_types.is_empty() {
        summary.push(format!(
            "kept kiln.toml's own {}",
            plan.kept_types.join(", ")
        ));
    }
    if !plan.added_links.is_empty() {
        summary.push(format!("roles {}", plan.added_links.join(", ")));
    }
    for (role, gained) in &plan.extended_links {
        summary.push(format!("{role} gains {gained}"));
    }
    summary.push(format!("export `{}`", plan.export));

    if dry_run {
        println!("kiln.toml: {}", summary.join("; "));
        for (path, _) in &plan.files {
            println!("{path}: new");
        }
        eprintln!("kiln: dry run; nothing was written");
        return Ok(0);
    }

    let mut written = Vec::new();
    let undo = |written: &[std::path::PathBuf]| {
        // Files this run created, so nobody else's.
        for path in written {
            let _ = std::fs::remove_file(path);
        }
    };
    for (path, text) in &plan.files {
        let full = root.join(path);
        let result = full
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| {
                fenix_storage::atomic_write_new(&full, |w| w.write_all(text.as_bytes()))
            });
        if let Err(e) = result {
            undo(&written);
            return Err(Fail::usage(format!("{path}: can't write it: {e}")));
        }
        written.push(full);
    }
    if let Err(e) = kiln_index::write_back(
        &config_path,
        kiln_parse::CONFIG_FILE,
        &read.bytes,
        read.stamp,
        &plan.config,
        true,
    ) {
        undo(&written);
        return Err(crate::project::edit_fail(e.into()));
    }
    println!("{}", kiln_parse::CONFIG_FILE);
    for (path, _) in &plan.files {
        println!("{path}");
    }
    eprintln!(
        "kiln: added the {name} preset ({}); `kiln export {}` writes it",
        summary.join("; "),
        plan.export
    );
    Ok(0)
}
