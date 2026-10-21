//! `kiln preset list` and `kiln preset add NAME`: the document templates
//! Kiln ships (DESIGN.md, "Presets"), copied into the project's
//! `templates/` folder. `kiln init --preset NAME` is `init` then `add`.
//!
//! Only the template is written. `kiln.toml` is left alone: the types the
//! template places and the export that uses it are the project's choice,
//! made from the options the wiki's "Presets" page lists (ADR-015). What
//! the template needs that the project doesn't declare yet is said on
//! stderr, so the next step is plain. Never overwrites.

use std::path::Path;

use kiln_index::{DiskSource, Project};
use kiln_model::PRESETS;

use crate::project::root;
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

pub fn add_at(root: &Path, name: &str, dry_run: bool) -> Result<u8, Fail> {
    let preset = find(name)?;
    let source = DiskSource::new(root);
    let project = Project::load(&source).map_err(|e| Fail::usage(e.to_string()))?;
    let dir = project
        .config
        .paths
        .first()
        .map(String::as_str)
        .unwrap_or("design");
    let path = preset.template_path(dir);
    let full = root.join(&path);
    if full.exists() {
        return Err(Fail::usage(format!(
            "{path} already exists; kiln preset add never overwrites (move it away, or rename it, and add the preset again)"
        )));
    }
    if dry_run {
        println!("{path}: new");
        eprintln!("kiln: dry run; nothing was written");
        return Ok(0);
    }
    full.parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| {
            fenix_storage::atomic_write_new(&full, |w| w.write_all(preset.template.as_bytes()))
        })
        .map_err(|e| Fail::usage(format!("{path}: can't write it: {e}")))?;
    println!("{path}");

    // The next step: what the template needs from kiln.toml.
    let mut missing: Vec<String> = Vec::new();
    for p in kiln_render::template_problems(preset.template, &project) {
        if !missing.contains(&p.message) {
            missing.push(p.message);
        }
    }
    let used = project
        .config
        .exports
        .iter()
        .any(|t| t.template.as_deref() == Some(path.as_str()));
    let page = "the wiki's \"Presets\" page has the configuration and its options";
    match (missing.is_empty(), used) {
        (true, true) => eprintln!("kiln: added the {name} template; `kiln export` uses it"),
        (true, false) => eprintln!(
            "kiln: added the {name} template; add an [[export]] with template = \"{path}\" to kiln.toml to use it ({page})"
        ),
        (false, _) => eprintln!(
            "kiln: added the {name} template; before an export can use it, kiln.toml needs what it places: {} ({page})",
            missing.join("; ")
        ),
    }
    Ok(0)
}
