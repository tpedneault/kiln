//! Finding and loading the project, and how a refused edit exits: the
//! I/O every command shares (the writing itself is `kiln-index`'s).

use std::path::{Path, PathBuf};

use kiln_index::{find_root, DiskSource, LoadError, Project};
use kiln_model::{Date, Severity};

use crate::{Fail, EXIT_FINDINGS};

/// The project root: `--project` or the current directory, then up to
/// the nearest `kiln.toml`.
pub fn root(project: Option<&Path>) -> Result<PathBuf, Fail> {
    let start = match project {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|e| Fail::usage(format!("can't read the current directory: {e}")))?,
    };
    find_root(&start).ok_or_else(|| {
        Fail::usage(format!(
            "no kiln.toml in {} or any folder above it (run `kiln init` to start a project)",
            start.display()
        ))
    })
}

/// A loaded project and the source it was read through.
pub struct Loaded {
    pub source: DiskSource,
    pub project: Project,
}

/// The JUnit reports given with `--results`, for every command that loads
/// the project: one global, set once from the command line, so each
/// command applies them the same way without passing them down.
static RESULTS: std::sync::OnceLock<Vec<PathBuf>> = std::sync::OnceLock::new();

/// Records `--results` for [`load`] and [`apply_results`].
pub fn set_results(files: Vec<PathBuf>) {
    let _ = RESULTS.set(files);
}

/// Applies the `--results` reports to `project`, when there are any: a
/// report that can't be read or parsed is a usage error naming it; the
/// verdicts are summed up on stderr (K18, ADR-021).
// implements: REQ-023
pub fn apply_results(project: &mut Project, source: &DiskSource) -> Result<(), Fail> {
    let Some(files) = RESULTS.get().filter(|f| !f.is_empty()) else {
        return Ok(());
    };
    let mut cases = Vec::new();
    for file in files {
        let text = std::fs::read(file).map_err(|e| {
            Fail::usage(format!("--results {}: can't read it: {e}", file.display()))
        })?;
        let text = String::from_utf8_lossy(&text);
        cases.extend(
            kiln_parse::parse_junit(&text)
                .map_err(|e| Fail::usage(format!("--results {}: {e}", file.display())))?,
        );
    }
    let summary = project.apply_results(source, &cases);
    eprintln!(
        "kiln: test results ({} cases): {} verifying references passed, {} failed, {} didn't run",
        cases.len(),
        summary.passed,
        summary.failed,
        summary.not_run
    );
    Ok(())
}

/// Loads the project for a command other than `check`. A configuration
/// Kiln can't use is a usage error (exit 2); lesser configuration
/// findings are summed up in one line on stderr and the command goes on.
pub fn load(project: Option<&Path>) -> Result<Loaded, Fail> {
    let root = root(project)?;
    let source = DiskSource::new(&root);
    let project = match Project::load(&source) {
        Ok(p) => p,
        Err(LoadError::Read(e)) => {
            return Err(Fail::usage(format!("kiln.toml: can't read it: {e}")))
        }
        Err(e @ LoadError::Config(_)) => return Err(Fail::usage(e.to_string())),
    };
    let config_errors = project
        .findings
        .iter()
        .filter(|f| f.path == kiln_parse::CONFIG_FILE && f.severity == Severity::Error)
        .count();
    if config_errors > 0 {
        eprintln!(
            "kiln: kiln.toml has {config_errors} problem{}; `kiln check` lists them",
            if config_errors == 1 { "" } else { "s" }
        );
    }
    let mut project = project;
    apply_results(&mut project, &source)?;
    Ok(Loaded { source, project })
}

/// The project as it was at a git revision, and that revision's commit:
/// its `kiln.toml` and Markdown files read from git, not the disk. Code
/// isn't read (the item diff doesn't compare code references). A
/// revision from before the project had a `kiln.toml` reads with today's
/// configuration and no items, so everything shows as added.
pub fn at_revision(root: &Path, rev: &str) -> Result<(Project, String), Fail> {
    let commit = kiln_git::resolve(root, rev).map_err(|e| Fail::usage(format!("{rev}: {e}")))?;
    let files = kiln_git::files_at(root, &commit, &|p| {
        p == kiln_parse::CONFIG_FILE || p.ends_with(".md")
    })
    .map_err(|e| Fail::usage(format!("{rev}: {e}")))?;
    let mut source = kiln_index::MemorySource::new();
    let has_config = files.iter().any(|(p, _)| p == kiln_parse::CONFIG_FILE);
    for (path, bytes) in &files {
        source.insert(path, bytes);
    }
    if !has_config {
        let today = std::fs::read(root.join(kiln_parse::CONFIG_FILE))
            .map_err(|e| Fail::usage(format!("kiln.toml: {e}")))?;
        source = kiln_index::MemorySource::new().with(kiln_parse::CONFIG_FILE, today);
    }
    let project = Project::load(&source).map_err(|e| Fail::usage(format!("{rev}: its {e}")))?;
    Ok((project, commit))
}

/// Kiln's own folders: `KILN_HOME`, else the platform's.
pub fn folders() -> Option<kiln_model::Folders> {
    let home = std::env::var_os("KILN_HOME").map(std::path::PathBuf::from);
    let platform = kiln_model::Platform {
        config_dir: dirs::config_dir(),
        state_dir: dirs::state_dir(),
        data_local_dir: dirs::data_local_dir(),
    };
    kiln_model::Folders::resolve(home.as_deref(), &platform)
}

/// Today in UTC, for the rules that compare dates.
pub fn today() -> Date {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Date::from_unix_days((secs / 86_400) as i64)
}

/// An edit `kiln-index` refused: a file changed on disk exits 1
/// (FORMAT.md, "Edits Kiln makes"), anything else is a usage error.
pub fn edit_fail(e: kiln_index::EditError) -> Fail {
    match e {
        kiln_index::EditError::Changed(m) => Fail::new(EXIT_FINDINGS, m),
        kiln_index::EditError::Invalid(m) | kiln_index::EditError::Io(m) => Fail::usage(m),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // verifies: REQ-013
    fn a_file_changed_since_it_was_read_is_not_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.md");
        std::fs::write(&path, "+++\r\nid = 1\r\n+++\r\n").unwrap();
        let read = kiln_index::read_for_edit(&path).unwrap();
        // An editor saves in between (a different length, so the stamp
        // differs even on file systems with coarse modification times).
        std::fs::write(&path, "+++\r\nid = 22\r\n+++\r\n").unwrap();
        let err = kiln_index::write_back(
            &path,
            "a.md",
            &read.bytes,
            read.stamp,
            "+++\nid = 3\n+++\n",
            true,
        )
        .map_err(|e| edit_fail(e.into()))
        .unwrap_err();
        assert_eq!(err.code, EXIT_FINDINGS);
        assert!(err.message.contains("changed on disk"), "{}", err.message);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "+++\r\nid = 22\r\n+++\r\n"
        );

        let read = kiln_index::read_for_edit(&path).unwrap();
        kiln_index::write_back(
            &path,
            "a.md",
            &read.bytes,
            read.stamp,
            "+++\nid = 3\n+++\n",
            true,
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "+++\r\nid = 3\r\n+++\r\n",
            "CRLF kept"
        );
    }
}
