//! Finding and loading the project, and writing files safely: the I/O
//! every command shares.

use std::path::{Path, PathBuf};

use kiln_index::{find_root, DiskSource, LoadError, Project, Stamp};
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
    Ok(Loaded { source, project })
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

/// A file read for editing: its bytes and stamp at the time.
pub struct ReadFile {
    pub bytes: Vec<u8>,
    pub stamp: Stamp,
}

pub fn read_for_edit(path: &Path) -> Result<ReadFile, Fail> {
    let stamp = Stamp::of(path).map_err(|e| Fail::usage(format!("{}: {e}", path.display())))?;
    let bytes = std::fs::read(path).map_err(|e| Fail::usage(format!("{}: {e}", path.display())))?;
    Ok(ReadFile { bytes, stamp })
}

/// Writes `text` (normalised `\n` text for item files, or raw text for
/// code files) over `path` atomically, with the file's own line ending
/// and byte-order mark, refusing when the file changed since `stamp`
/// (FORMAT.md, "Edits Kiln makes").
// implements: REQ-013
pub fn write_back(
    path: &Path,
    rel: &str,
    original: &[u8],
    stamp: Stamp,
    text: &str,
    normalised: bool,
) -> Result<(), Fail> {
    let now = Stamp::of(path).map_err(|e| Fail::usage(format!("{rel}: {e}")))?;
    if now != stamp {
        return Err(Fail::new(
            EXIT_FINDINGS,
            format!("{rel} changed on disk since Kiln read it (an editor's save?); nothing was written to it, run the command again"),
        ));
    }
    let mut out = Vec::new();
    if normalised {
        if original.starts_with(b"\xef\xbb\xbf") {
            out.extend_from_slice(b"\xef\xbb\xbf");
        }
        out.extend_from_slice(
            kiln_parse::denormalize(text, kiln_parse::uses_crlf(original)).as_bytes(),
        );
    } else {
        out.extend_from_slice(text.as_bytes());
    }
    fenix_storage::write(path, &out).map_err(|e| Fail::usage(format!("{rel}: can't write it: {e}")))
}

/// `path` relative to `root` with `/`, for messages.
pub fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
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
        let read = read_for_edit(&path).unwrap();
        // An editor saves in between (a different length, so the stamp
        // differs even on file systems with coarse modification times).
        std::fs::write(&path, "+++\r\nid = 22\r\n+++\r\n").unwrap();
        let err = write_back(
            &path,
            "a.md",
            &read.bytes,
            read.stamp,
            "+++\nid = 3\n+++\n",
            true,
        )
        .unwrap_err();
        assert_eq!(err.code, EXIT_FINDINGS);
        assert!(err.message.contains("changed on disk"), "{}", err.message);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "+++\r\nid = 22\r\n+++\r\n"
        );

        let read = read_for_edit(&path).unwrap();
        write_back(
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
