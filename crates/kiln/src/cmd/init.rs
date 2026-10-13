//! `kiln init`: a `kiln.toml` with FORMAT.md's default types and a
//! `design/` folder. Never overwrites anything.

use std::path::Path;

use crate::Fail;

const DESIGN_README: &str = "# Design\n\nKiln items live here: Markdown files that start with a `+++` front\nmatter block. `kiln new requirement --title \"…\"` creates one; `kiln check`\nchecks them. Files without front matter, like this one, are ignored.\n";

pub fn run(project: Option<&Path>, name: Option<String>) -> Result<u8, Fail> {
    let dir = match project {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|e| Fail::usage(format!("can't read the current directory: {e}")))?,
    };
    if !dir.is_dir() {
        return Err(Fail::usage(format!("{} isn't a folder", dir.display())));
    }
    let config = dir.join(kiln_parse::CONFIG_FILE);
    if config.exists() {
        return Err(Fail::usage(format!(
            "{} already exists; kiln init never overwrites",
            config.display()
        )));
    }
    let name = name
        .or_else(|| {
            dir.canonicalize()
                .ok()
                .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
        })
        .unwrap_or_else(|| "Project".to_owned());
    let text = kiln_model::default_config(&name);
    fenix_storage::atomic_write_new(&config, |w| w.write_all(text.as_bytes()))
        .map_err(|e| Fail::usage(format!("{}: can't write it: {e}", config.display())))?;
    let design = dir.join("design");
    let readme = design.join("README.md");
    std::fs::create_dir_all(&design)
        .map_err(|e| Fail::usage(format!("{}: {e}", design.display())))?;
    if !readme.exists() {
        fenix_storage::atomic_write_new(&readme, |w| w.write_all(DESIGN_README.as_bytes()))
            .map_err(|e| Fail::usage(format!("{}: can't write it: {e}", readme.display())))?;
    }
    println!("kiln.toml");
    println!("design/");
    eprintln!("kiln: started project \"{name}\"; `kiln new requirement --title \"…\"` adds the first item");
    Ok(0)
}
