//! `kiln init`: a `kiln.toml` with FORMAT.md's default types and a
//! `design/` folder. Never overwrites anything. `--preset` then adds a
//! preset, as `kiln preset add` would: the defaults stay, since a preset
//! adds only the types it lacks.
//!
//! `--fenix` also registers `kiln lsp` as an additional language server in
//! the project's `.fenix/tools.json` (INTEGRATION.md, "Editing items"), for
//! Markdown and the languages of the code `[code] paths` scans, beside each
//! language's own server: merged into an existing file, never replacing
//! it. That file is the project's configuration,
//! written once on request; Kiln never writes Fenix's files at run time.

use std::path::Path;

use crate::Fail;

const DESIGN_README: &str = "# Design\n\nKiln items live here: Markdown files that start with a `+++` front\nmatter block. `kiln new requirement --title \"…\"` creates one; `kiln check`\nchecks them. Files without front matter, like this one, are ignored.\n";

pub fn run(
    project: Option<&Path>,
    name: Option<String>,
    fenix: bool,
    preset: Option<&str>,
) -> Result<u8, Fail> {
    // An unknown preset stops before anything is written.
    if let Some(p) = preset {
        super::preset::find(p)?;
    }
    let dir = match project {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|e| Fail::usage(format!("can't read the current directory: {e}")))?,
    };
    if !dir.is_dir() {
        return Err(Fail::usage(format!("{} isn't a folder", dir.display())));
    }
    let config = dir.join(kiln_parse::CONFIG_FILE);
    if config.exists() && fenix && preset.is_none() {
        // An existing project only gets the editor setup.
        return register_lsp(&dir).map(|()| 0);
    }
    if config.exists() {
        let hint = match preset {
            Some(p) => format!(" (`kiln preset add {p}` adds the preset to it)"),
            None => String::new(),
        };
        return Err(Fail::usage(format!(
            "{} already exists; kiln init never overwrites{hint}",
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
    if let Some(p) = preset {
        super::preset::add_at(&dir, p, false)?;
    }
    if fenix {
        register_lsp(&dir)?;
    }
    Ok(0)
}

/// Fenix's language name for a code file's extension, as Fenix detects
/// languages (`fenix_syntax::detect_language` and the names `tools.json`
/// uses). Extensions Fenix has no language for are left out: Fenix
/// couldn't attach a server to those files anyway.
fn fenix_language(path: &str) -> Option<&'static str> {
    let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "rs" => "rust",
        "py" | "pyi" => "python",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "c++" | "hpp" | "hh" | "hxx" | "h++" | "inl" | "ino" | "pde" => {
            "cpp"
        }
        "sh" | "bash" => "bash",
        "tcl" | "tm" => "tcl",
        "go" => "go",
        "java" => "java",
        "cs" | "csx" => "csharp",
        "lua" => "lua",
        "sql" => "sql",
        "bat" | "cmd" => "batch",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "json" => "json",
        "xml" => "xml",
        "html" | "htm" => "html",
        "css" => "css",
        _ => return None,
    })
}

/// The languages Kiln serves in this project: Markdown (its items), and
/// the language of every code file `[code] paths` scans, in name order.
fn languages(dir: &Path) -> Vec<String> {
    let mut out = std::collections::BTreeSet::from(["markdown".to_owned()]);
    if let Ok(project) = kiln_index::Project::load(&kiln_index::DiskSource::new(dir)) {
        out.extend(
            project
                .known_files()
                .1
                .iter()
                .filter_map(|f| fenix_language(f))
                .map(str::to_owned),
        );
    }
    out.into_iter().collect()
}

/// Registers `kiln lsp` in `.fenix/tools.json` as an additional language
/// server (Fenix G3): `"lsp": { "additional": [{ "name": "kiln",
/// "executable": "kiln", "args": ["lsp"], "languages": [...] }] }`, beside
/// whatever server each language already has, keeping everything else in
/// the file. Run again after adding `[code] paths`, it adds the new
/// languages. The pre-G3 form (`kiln lsp` as the Markdown server) is moved
/// over, so one Fenix doesn't start Kiln twice.
fn register_lsp(dir: &Path) -> Result<(), Fail> {
    let fenix_dir = dir.join(".fenix");
    let path = fenix_dir.join("tools.json");
    let shown = ".fenix/tools.json";
    let mut tools: serde_json::Value = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| {
            Fail::usage(format!(
                "{shown} isn't valid JSON ({e}); add Kiln's server by hand (the wiki's \"Editors and Flint\")"
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
    let mut notes = Vec::new();
    if lsp
        .get("markdown")
        .and_then(|m| m["executable"].as_str())
        .is_some_and(|e| e == "kiln")
    {
        lsp.remove("markdown");
        notes.push("moved `kiln lsp` from the Markdown server to an additional server".to_owned());
    }
    let wanted = languages(dir);
    let additional = lsp
        .entry("additional")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .ok_or_else(|| Fail::usage(format!("`lsp.additional` in {shown} isn't an array")))?;
    let changed = match additional.iter_mut().find(|s| s["name"] == "kiln") {
        Some(entry) => {
            // Keep what the user added, in their order; add what the
            // project now needs.
            let mut langs: Vec<serde_json::Value> =
                entry["languages"].as_array().cloned().unwrap_or_default();
            let missing: Vec<&String> = wanted
                .iter()
                .filter(|w| !langs.iter().any(|l| l == *w))
                .collect();
            let changed = !missing.is_empty();
            langs.extend(missing.into_iter().map(|w| serde_json::json!(w)));
            entry["languages"] = serde_json::json!(langs);
            changed
        }
        None => {
            additional.push(serde_json::json!({
                "name": "kiln",
                "executable": "kiln",
                "args": ["lsp"],
                "languages": wanted,
            }));
            true
        }
    };
    let languages: Vec<String> = additional
        .iter()
        .find(|s| s["name"] == "kiln")
        .and_then(|e| e["languages"].as_array())
        .map(|a| {
            a.iter()
                .filter_map(|l| l.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if !changed && notes.is_empty() {
        eprintln!(
            "kiln: {shown} already runs `kiln lsp` for {}",
            languages.join(", ")
        );
        return Ok(());
    }
    std::fs::create_dir_all(&fenix_dir)
        .map_err(|e| Fail::usage(format!("{}: {e}", fenix_dir.display())))?;
    let text = super::json_out(&tools);
    fenix_storage::write(&path, text.as_bytes())
        .map_err(|e| Fail::usage(format!("{shown}: can't write it: {e}")))?;
    println!("{shown}");
    for note in notes {
        eprintln!("kiln: {note}");
    }
    eprintln!(
        "kiln: Fenix now runs `kiln lsp` beside each language's own server for {} in this project (`:lsp-restart` to pick it up)",
        languages.join(", ")
    );
    Ok(())
}
