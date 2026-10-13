//! Where a project's files come from. The index reads only through
//! [`FileSource`], so tests load in-memory projects and the language
//! server (K4) can feed it unsaved buffers; [`DiskSource`] is the real
//! file system.

use std::collections::{BTreeMap, HashMap};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use ignore::overrides::{Override, OverrideBuilder};
use ignore::{WalkBuilder, WalkState};

/// A project's files, by path relative to its root with `/` separators.
pub trait FileSource: Sync {
    /// Files under each of `dirs` (relative; `.` is the root), honouring
    /// `.gitignore`, minus anything matching `exclude` (gitignore-style
    /// globs), with extension `ext` when given. Sorted. A directory that
    /// doesn't exist is reported in `missing`, not an error.
    fn list(&self, dirs: &[String], exclude: &[String], ext: Option<&str>) -> io::Result<Listing>;

    fn read(&self, path: &str) -> io::Result<Vec<u8>>;

    /// Whether a file or directory exists (for relative links, K110).
    fn exists(&self, path: &str) -> bool;
}

/// What [`FileSource::list`] found.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Listing {
    pub files: Vec<String>,
    /// Requested directories that don't exist.
    pub missing: Vec<String>,
}

/// Builds the exclusion matcher: each glob as an ignore rule.
fn excludes(root: &Path, globs: &[String]) -> io::Result<Override> {
    let mut builder = OverrideBuilder::new(root);
    for glob in globs {
        builder.add(&format!("!{glob}")).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("exclude `{glob}`: {e}"),
            )
        })?;
    }
    builder
        .build()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))
}

fn has_ext(path: &str, ext: Option<&str>) -> bool {
    match ext {
        None => true,
        Some(ext) => path
            .rsplit_once('.')
            .is_some_and(|(_, e)| e.eq_ignore_ascii_case(ext)),
    }
}

/// The file system under `root`.
pub struct DiskSource {
    root: PathBuf,
    /// Length and modification time of every file read, so a writer can
    /// refuse when a file changed since the index read it.
    stamps: Mutex<HashMap<String, Stamp>>,
}

/// A file's length and modification time when it was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub len: u64,
    pub modified: Option<SystemTime>,
}

impl Stamp {
    pub fn of(path: &Path) -> io::Result<Stamp> {
        let meta = std::fs::metadata(path)?;
        Ok(Stamp {
            len: meta.len(),
            modified: meta.modified().ok(),
        })
    }
}

impl DiskSource {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        DiskSource {
            root: root.into(),
            stamps: Mutex::new(HashMap::new()),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// The stamp recorded when `rel` was read, if it was.
    pub fn stamp(&self, rel: &str) -> Option<Stamp> {
        self.stamps.lock().ok()?.get(rel).copied()
    }
}

impl FileSource for DiskSource {
    fn list(&self, dirs: &[String], exclude: &[String], ext: Option<&str>) -> io::Result<Listing> {
        let overrides = excludes(&self.root, exclude)?;
        let mut listing = Listing::default();
        let found = Mutex::new(Vec::new());
        for dir in dirs {
            // `root/.` would reach the exclusion matcher as `root/./src`,
            // which no root-anchored glob matches.
            let start = if dir == "." {
                self.root.clone()
            } else {
                self.root.join(dir)
            };
            if !start.is_dir() {
                listing.missing.push(dir.clone());
                continue;
            }
            let walker = WalkBuilder::new(&start)
                // .gitignore applies whether or not this is a git checkout
                // (a CI job may export an archive).
                .require_git(false)
                .overrides(overrides.clone())
                .build_parallel();
            walker.run(|| {
                let found = &found;
                let root = &self.root;
                Box::new(move |entry| {
                    if let Ok(entry) = entry {
                        if entry.file_type().is_some_and(|t| t.is_file()) {
                            if let Ok(rel) = entry.path().strip_prefix(root) {
                                let rel = rel.to_string_lossy().replace('\\', "/");
                                if has_ext(&rel, ext) {
                                    if let Ok(mut found) = found.lock() {
                                        found.push(rel);
                                    }
                                }
                            }
                        }
                    }
                    WalkState::Continue
                })
            });
        }
        let mut files = found.into_inner().unwrap_or_default();
        files.sort();
        files.dedup();
        listing.files = files;
        Ok(listing)
    }

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        let full = self.root.join(path);
        let stamp = Stamp::of(&full)?;
        let bytes = std::fs::read(&full)?;
        if let Ok(mut stamps) = self.stamps.lock() {
            stamps.insert(path.to_owned(), stamp);
        }
        Ok(bytes)
    }

    fn exists(&self, path: &str) -> bool {
        self.root.join(path).exists()
    }
}

/// Files in memory, for tests (and, from K4, unsaved buffers layered over
/// the disk).
#[derive(Debug, Default, Clone)]
pub struct MemorySource {
    files: BTreeMap<String, Vec<u8>>,
}

impl MemorySource {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, path: &str, contents: impl AsRef<[u8]>) -> Self {
        self.insert(path, contents);
        self
    }

    pub fn insert(&mut self, path: &str, contents: impl AsRef<[u8]>) {
        self.files
            .insert(path.to_owned(), contents.as_ref().to_vec());
    }

    pub fn remove(&mut self, path: &str) {
        self.files.remove(path);
    }
}

impl FileSource for MemorySource {
    fn list(&self, dirs: &[String], exclude: &[String], ext: Option<&str>) -> io::Result<Listing> {
        let overrides = excludes(Path::new(""), exclude)?;
        let excluded = |path: &str| {
            let mut at = Path::new(path);
            let mut is_dir = false;
            loop {
                if overrides.matched(at, is_dir).is_ignore() {
                    return true;
                }
                match at.parent() {
                    Some(parent) if !parent.as_os_str().is_empty() => {
                        at = parent;
                        is_dir = true;
                    }
                    _ => return false,
                }
            }
        };
        let mut listing = Listing::default();
        for dir in dirs {
            let prefix = if dir == "." {
                String::new()
            } else {
                format!("{dir}/")
            };
            let matching: Vec<&String> = self
                .files
                .keys()
                .filter(|p| p.starts_with(&prefix))
                .collect();
            if matching.is_empty() && !prefix.is_empty() {
                listing.missing.push(dir.clone());
            }
            listing.files.extend(
                matching
                    .into_iter()
                    .filter(|p| has_ext(p, ext) && !excluded(p))
                    .cloned(),
            );
        }
        listing.files.sort();
        listing.files.dedup();
        Ok(listing)
    }

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{path} not found")))
    }

    fn exists(&self, path: &str) -> bool {
        let dir = format!("{}/", path.trim_end_matches('/'));
        self.files.contains_key(path) || self.files.keys().any(|p| p.starts_with(&dir))
    }
}

/// The nearest directory at or above `start` holding `kiln.toml`.
pub fn find_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(kiln_parse::CONFIG_FILE).is_file())
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_listing_filters_by_dir_extension_and_exclude() {
        let source = MemorySource::new()
            .with("design/a.md", "")
            .with("design/sub/b.md", "")
            .with("design/c.txt", "")
            .with("crates/x/src/lib.rs", "")
            .with("crates/x/tests/fixtures/f.rs", "");
        let md = source
            .list(&["design".into(), "nowhere".into()], &[], Some("md"))
            .unwrap();
        assert_eq!(md.files, ["design/a.md", "design/sub/b.md"]);
        assert_eq!(md.missing, ["nowhere"]);
        let code = source
            .list(
                &["crates".into()],
                &["crates/*/tests/fixtures".into()],
                None,
            )
            .unwrap();
        assert_eq!(code.files, ["crates/x/src/lib.rs"]);
        assert!(
            source.exists("crates/x")
                && source.exists("design/a.md")
                && !source.exists("design/z.md")
        );
    }

    #[test]
    fn disk_listing_honours_gitignore_and_excludes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for (path, text) in [
            (".gitignore", "target/\n"),
            ("design/a.md", "a"),
            ("design/deep/b.md", "b"),
            ("src/lib.rs", "x"),
            ("src/fixtures/f.rs", "x"),
            ("target/gen.rs", "x"),
        ] {
            let full = root.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, text).unwrap();
        }
        let source = DiskSource::new(root);
        let md = source.list(&["design".into()], &[], Some("md")).unwrap();
        assert_eq!(md.files, ["design/a.md", "design/deep/b.md"]);
        let code = source
            .list(&[".".into()], &["src/fixtures".into()], Some("rs"))
            .unwrap();
        assert_eq!(code.files, ["src/lib.rs"]);
        assert_eq!(source.read("design/a.md").unwrap(), b"a");
        assert_eq!(source.stamp("design/a.md").unwrap().len, 1);
        assert_eq!(find_root(&root.join("design/deep")), None);
        std::fs::write(root.join("kiln.toml"), "").unwrap();
        assert_eq!(find_root(&root.join("design/deep")).as_deref(), Some(root));
    }
}
