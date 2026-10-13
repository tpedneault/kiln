//! `kiln init`: a `kiln.toml` with FORMAT.md's default types and a
//! `design/` folder. Never overwrites anything.
//!
//! `--fenix` also registers `kiln lsp` as the Markdown language server in
//! the project's `.fenix/tools.json` (INTEGRATION.md, "Editing items"):
//! merged into an existing file, never replacing it, and leaving another
//! Markdown server alone. That file is the project's configuration,
//! written once on request; Kiln never writes Fenix's files at run time.

use std::path::Path;

use crate::Fail;

const DESIGN_README: &str = "# Design\n\nKiln items live here: Markdown files that start with a `+++` front\nmatter block. `kiln new requirement --title \"…\"` creates one; `kiln check`\nchecks them. Files without front matter, like this one, are ignored.\n";

pub fn run(project: Option<&Path>, name: Option<String>, fenix: bool) -> Result<u8, Fail> {
    let dir = match project {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|e| Fail::usage(format!("can't read the current directory: {e}")))?,
    };
    if !dir.is_dir() {
        return Err(Fail::usage(format!("{} isn't a folder", dir.display())));
    }
    let config = dir.join(kiln_parse::CONFIG_FILE);
    if config.exists() && fenix {
        // An existing project only gets the editor setup.
        return register_lsp(&dir).map(|()| 0);
    }
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
    if fenix {
        register_lsp(&dir)?;
    }
    Ok(0)
}

/// Adds `"lsp": { "markdown": { "executable": "kiln", "args": ["lsp"] } }`
/// to `.fenix/tools.json`, keeping everything else in it.
fn register_lsp(dir: &Path) -> Result<(), Fail> {
    let fenix_dir = dir.join(".fenix");
    let path = fenix_dir.join("tools.json");
    let shown = ".fenix/tools.json";
    let mut tools: serde_json::Value = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| {
            Fail::usage(format!(
                "{shown} isn't valid JSON ({e}); add Kiln's server by hand (docs/INTEGRATION.md)"
            ))
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(e) => return Err(Fail::usage(format!("{shown}: {e}"))),
    };
    let lsp = tools
        .as_object_mut()
        .ok_or_else(|| Fail::usage(format!("{shown} isn't a JSON object")))?
        .entry("lsp")
        .or_insert_with(|| serde_json::json!({}));
    let lsp = lsp
        .as_object_mut()
        .ok_or_else(|| Fail::usage(format!("`lsp` in {shown} isn't an object")))?;
    if let Some(existing) = lsp.get("markdown") {
        let executable = existing["executable"].as_str().unwrap_or("?");
        if executable == "kiln" {
            eprintln!("kiln: {shown} already runs `kiln lsp` for Markdown");
        } else {
            eprintln!("kiln: {shown} already runs `{executable}` for Markdown; left it alone");
        }
        return Ok(());
    }
    lsp.insert(
        "markdown".into(),
        serde_json::json!({ "executable": "kiln", "args": ["lsp"] }),
    );
    std::fs::create_dir_all(&fenix_dir)
        .map_err(|e| Fail::usage(format!("{}: {e}", fenix_dir.display())))?;
    let text = super::json_out(&tools);
    fenix_storage::write(&path, text.as_bytes())
        .map_err(|e| Fail::usage(format!("{shown}: can't write it: {e}")))?;
    println!("{shown}");
    eprintln!("kiln: Fenix now runs `kiln lsp` for Markdown in this project (restart its language server to pick it up)");
    Ok(())
}
