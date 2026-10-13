//! `kiln new`: the next free ID, a file from the type's template, its
//! path on stdout.

use std::path::Path;

use kiln_model::slug;

use super::{existing, parse_id, toml_string};
use crate::project::{load, relative};
use crate::Fail;

/// File names carry at most this much of the title.
const SLUG_LEN: usize = 50;

pub fn run(
    project: Option<&Path>,
    type_name: &str,
    parent: Option<String>,
    title: Option<String>,
    open: bool,
) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let root = loaded.source.root();
    let ty = p.config.type_named(type_name).ok_or_else(|| {
        let names: Vec<&str> = p.config.types.iter().map(|t| t.name.as_str()).collect();
        Fail::usage(format!("`{type_name}` isn't a type ({})", names.join(", ")))
    })?;
    let parent = match parent {
        None => None,
        Some(arg) => {
            let id = parse_id(&arg)?;
            let target = existing(p, &id)?;
            if !ty.parents.contains(&target.type_name) {
                let allowed = if ty.parents.is_empty() {
                    "none".to_owned()
                } else {
                    ty.parents.join(", ")
                };
                return Err(Fail::usage(format!(
                    "a {} can't have a {} as parent (allowed: {allowed})",
                    ty.name, target.type_name
                )));
            }
            Some(id)
        }
    };
    let title = title.unwrap_or_else(|| format!("Untitled {}", ty.title.to_lowercase()));
    if title.contains('\n') || title.chars().count() > kiln_parse::MAX_TITLE {
        return Err(Fail::usage(format!(
            "a title is one line of at most {} characters",
            kiln_parse::MAX_TITLE
        )));
    }
    let body = match &ty.template {
        None => String::new(),
        Some(template) => {
            let bytes = std::fs::read(root.join(template))
                .map_err(|e| Fail::usage(format!("template {template} for {}: {e}", ty.name)))?;
            kiln_parse::normalize(&bytes)
                .ok_or_else(|| Fail::usage(format!("template {template} isn't UTF-8")))?
        }
    };
    let items_dir = p
        .config
        .paths
        .first()
        .ok_or_else(|| Fail::usage("kiln.toml has no `paths` to put items in"))?;
    let id = p.next_id(&ty.prefix);
    let name = match slug(&title, SLUG_LEN) {
        s if s.is_empty() => format!("{id}.md"),
        s => format!("{id}-{s}.md"),
    };
    let dir = root.join(items_dir).join(ty.folder());
    let path = dir.join(&name);

    let mut text = format!(
        "+++\nid = \"{id}\"\ntype = {}\ntitle = {}\nstatus = {}\n",
        toml_string(&ty.name),
        toml_string(&title),
        toml_string(&ty.initial)
    );
    if let Some(parent) = &parent {
        text.push_str(&format!("parent = \"{parent}\"\n"));
    }
    text.push_str("+++\n");
    if !body.is_empty() {
        text.push('\n');
        text.push_str(&body);
        if !body.ends_with('\n') {
            text.push('\n');
        }
    }
    std::fs::create_dir_all(&dir).map_err(|e| Fail::usage(format!("{}: {e}", dir.display())))?;
    fenix_storage::atomic_write_new(&path, |w| w.write_all(text.as_bytes()))
        .map_err(|e| Fail::usage(format!("{}: can't create it: {e}", relative(root, &path))))?;
    println!("{}", relative(root, &path));
    if open {
        // The title is on line 4; the item exists whether or not this works.
        match kiln_fenix::open(&path, 4) {
            Ok(how) => eprintln!("kiln: {id} {}", how.describe()),
            Err(e) => eprintln!("kiln: created {id} but couldn't open it: {e}"),
        }
    }
    Ok(0)
}
