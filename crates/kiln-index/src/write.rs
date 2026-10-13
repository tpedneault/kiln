//! Writing an edited file back (FORMAT.md, "Edits Kiln makes"): read it
//! with its stamp, compute the new text, and write it atomically with the
//! file's own line ending and byte-order mark, but only if it hasn't
//! changed on disk in between. Shared by the command line and the Flint
//! provider, so both refuse the same way when an editor saved first.
//!
//! The one place in this crate besides [`crate::DiskSource`] that touches
//! the disk; kept here so every writer gets the stamp check, not just the
//! binary.

use std::path::Path;

use crate::source::Stamp;

/// A file read for editing: its bytes and stamp at the time.
pub struct ReadFile {
    pub bytes: Vec<u8>,
    pub stamp: Stamp,
}

/// Why a write didn't happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteError {
    /// The file changed since it was read; nothing was written.
    Changed(String),
    /// Reading or writing failed.
    Io(String),
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WriteError::Changed(m) | WriteError::Io(m) => f.write_str(m),
        }
    }
}

pub fn read_for_edit(path: &Path) -> Result<ReadFile, WriteError> {
    let io = |e: std::io::Error| WriteError::Io(format!("{}: {e}", path.display()));
    let stamp = Stamp::of(path).map_err(io)?;
    let bytes = std::fs::read(path).map_err(io)?;
    Ok(ReadFile { bytes, stamp })
}

/// Writes `text` (normalised `\n` text for item files, or raw text for
/// code files) over `path` atomically, with the file's own line ending
/// and byte-order mark, refusing when the file changed since `stamp`.
// implements: REQ-013
pub fn write_back(
    path: &Path,
    rel: &str,
    original: &[u8],
    stamp: Stamp,
    text: &str,
    normalised: bool,
) -> Result<(), WriteError> {
    let now = Stamp::of(path).map_err(|e| WriteError::Io(format!("{rel}: {e}")))?;
    if now != stamp {
        return Err(WriteError::Changed(format!(
            "{rel} changed on disk since Kiln read it (an editor's save?); nothing was written to it, run the command again"
        )));
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
    fenix_storage::write(path, &out)
        .map_err(|e| WriteError::Io(format!("{rel}: can't write it: {e}")))
}
