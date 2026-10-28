//! `kiln agents`: writes the agent guide, `AGENTS.md` in the first item
//! folder, or brings its generated section up to date with `kiln.toml`
//! (`kiln_model::write_agents`, DESIGN.md "The agent guide"). `kiln init`
//! writes it and `kiln preset add` refreshes it, through [`refresh`].
//!
//! Only Kiln's section between its two marker lines is ever replaced;
//! the rest of the file is the project's, and a file without the
//! markers gains the section at its end. An existing file goes through
//! `write_back`, so its line endings survive and a file changed under
//! Kiln is refused, as for every other edit.

use std::path::Path;

use kiln_model::Config;

use crate::project::root;
use crate::Fail;

pub fn run(project: Option<&Path>) -> Result<u8, Fail> {
    let root = root(project)?;
    let source = kiln_index::DiskSource::new(&root);
    let project =
        kiln_index::Project::load(&source).map_err(|e| Fail::usage(format!("kiln.toml: {e}")))?;
    let existed = root.join(kiln_model::agents_path(&project.config)).exists();
    match refresh(&root, &project.config, true)? {
        Some((path, true)) if existed => {
            println!("{path}");
            eprintln!("kiln: brought Kiln's section of {path} up to date with kiln.toml");
        }
        Some((path, true)) => {
            println!("{path}");
            eprintln!(
                "kiln: wrote the agent guide; everything above Kiln's section is yours to edit"
            );
            eprintln!("{}", pointer_hint(&project.config));
        }
        Some((path, false)) => eprintln!("kiln: {path} is up to date"),
        None => {}
    }
    Ok(0)
}

/// What to tell a person who just got a guide: the line for the
/// repository's own guide, since agents working on the code may never
/// look in the design folder.
pub fn pointer_hint(config: &Config) -> String {
    format!(
        "kiln: so agents working on the code find it, add this line to the repository's AGENTS.md or CLAUDE.md:\n  {}",
        kiln_model::agents_pointer(config)
    )
}

/// Brings the guide up to date: `Some((path, changed))`, or `None` when
/// there is no guide and `create` is false. Kiln's own lines go nowhere
/// else: a guide whose markers are broken is an error naming the line.
pub fn refresh(root: &Path, config: &Config, create: bool) -> Result<Option<(String, bool)>, Fail> {
    let rel = kiln_model::agents_path(config);
    let full = root.join(&rel);
    if !full.exists() {
        if !create {
            return Ok(None);
        }
        let text = kiln_model::write_agents(config, None)
            .map_err(|e| Fail::usage(format!("{rel}: {e}")))?;
        full.parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| fenix_storage::atomic_write_new(&full, |w| w.write_all(text.as_bytes())))
            .map_err(|e| Fail::usage(format!("{rel}: can't write it: {e}")))?;
        return Ok(Some((rel, true)));
    }
    let read = kiln_index::read_for_edit(&full).map_err(|e| Fail::usage(e.to_string()))?;
    let text = kiln_parse::normalize(&read.bytes)
        .ok_or_else(|| Fail::usage(format!("{rel} isn't UTF-8")))?;
    let new = kiln_model::write_agents(config, Some(&text))
        .map_err(|e| Fail::usage(format!("{rel}: {e}")))?;
    if new == text {
        return Ok(Some((rel, false)));
    }
    kiln_index::write_back(&full, &rel, &read.bytes, read.stamp, &new, true)
        .map_err(|e| crate::project::edit_fail(e.into()))?;
    Ok(Some((rel, true)))
}
