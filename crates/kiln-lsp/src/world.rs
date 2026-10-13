//! What the server knows: the project root, the documents the editor has
//! open (which win over the disk), and the project loaded through them.
//! Edits are applied lazily: a change marks the document, and the next
//! request or the next idle moment re-reads just that file
//! ([`Project::update_file`]), so a burst of keystrokes costs one update.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io;
use std::path::{Path, PathBuf};

use kiln_index::{DiskSource, FileSource, Listing, Project};
use kiln_model::{Date, Finding};
use lsp_types::Uri;

use crate::text::Doc;
use crate::uri;

/// An open document.
#[derive(Debug, Clone)]
pub struct Open {
    pub doc: Doc,
    /// Relative to the root; `None` outside the project.
    pub rel: Option<String>,
}

pub struct World {
    pub root: Option<PathBuf>,
    disk: Option<DiskSource>,
    /// By URI as the editor sent it.
    pub open: HashMap<String, Open>,
    /// Open documents by relative path, to their URI.
    by_rel: HashMap<String, String>,
    pub project: Option<Project>,
    /// Why there is no project, when there is a root.
    pub load_error: Option<String>,
    /// Relative paths whose new text the project hasn't read yet.
    stale: BTreeSet<String>,
    /// `kiln.toml` changed: load everything again.
    reload: bool,
}

/// The disk, with open documents layered over it.
pub struct Overlay<'a> {
    disk: &'a DiskSource,
    open: &'a HashMap<String, Open>,
    by_rel: &'a HashMap<String, String>,
}

impl FileSource for Overlay<'_> {
    fn list(&self, dirs: &[String], exclude: &[String], ext: Option<&str>) -> io::Result<Listing> {
        self.disk.list(dirs, exclude, ext)
    }

    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        match self.by_rel.get(path).and_then(|u| self.open.get(u)) {
            Some(open) => Ok(open.doc.text.clone().into_bytes()),
            None => self.disk.read(path),
        }
    }

    fn exists(&self, path: &str) -> bool {
        self.by_rel.contains_key(path) || self.disk.exists(path)
    }
}

impl World {
    /// A world for the project containing `start` (a folder), if any.
    pub fn new(start: Option<&Path>) -> World {
        let mut world = World {
            root: None,
            disk: None,
            open: HashMap::new(),
            by_rel: HashMap::new(),
            project: None,
            load_error: None,
            stale: BTreeSet::new(),
            reload: false,
        };
        if let Some(start) = start {
            world.find_root(start);
        }
        world
    }

    /// Looks for `kiln.toml` from `start` upwards; loads it when found
    /// and no project is loaded yet.
    pub fn find_root(&mut self, start: &Path) {
        if self.root.is_some() {
            return;
        }
        if let Some(root) = kiln_index::find_root(start) {
            self.disk = Some(DiskSource::new(&root));
            self.root = Some(root);
            self.reload = true;
        }
    }

    pub fn source(&self) -> Option<Overlay<'_>> {
        Some(Overlay {
            disk: self.disk.as_ref()?,
            open: &self.open,
            by_rel: &self.by_rel,
        })
    }

    /// The editor opened or changed a document.
    pub fn set(&mut self, uri: &Uri, text: &str) {
        let path = uri::to_path(uri);
        if self.root.is_none() {
            if let Some(dir) = path.as_deref().and_then(Path::parent) {
                self.find_root(dir);
            }
        }
        let rel = match (&self.root, &path) {
            (Some(root), Some(path)) => uri::relative(root, path),
            _ => None,
        };
        if let Some(rel) = &rel {
            self.by_rel.insert(rel.clone(), uri.as_str().to_owned());
            if rel == kiln_parse::CONFIG_FILE {
                self.reload = true;
            } else {
                self.stale.insert(rel.clone());
            }
        }
        self.open.insert(
            uri.as_str().to_owned(),
            Open {
                doc: Doc::new(text),
                rel,
            },
        );
    }

    /// The editor closed a document: the disk's copy counts again.
    pub fn close(&mut self, uri: &Uri) {
        if let Some(open) = self.open.remove(uri.as_str()) {
            if let Some(rel) = open.rel {
                self.by_rel.remove(&rel);
                if rel == kiln_parse::CONFIG_FILE {
                    self.reload = true;
                } else {
                    self.stale.insert(rel);
                }
            }
        }
    }

    /// Files changed on disk (from the watcher); open ones are the
    /// editor's and are left alone.
    pub fn changed_on_disk(&mut self, paths: &[String], new_code_files: &[String]) {
        for path in paths {
            if path == kiln_parse::CONFIG_FILE {
                self.reload = true;
            } else if !self.by_rel.contains_key(path) {
                self.stale.insert(path.clone());
            }
        }
        if let Some(project) = &mut self.project {
            for path in new_code_files {
                project.track_code_file(path, true);
            }
        }
    }

    /// Brings the project up to date with every change since the last
    /// call; returns whether anything changed.
    pub fn sync(&mut self) -> bool {
        if self.reload {
            self.reload = false;
            self.stale.clear();
            let Some(source) = self.source() else {
                return false;
            };
            match Project::load(&source) {
                Ok(mut project) => {
                    // Open documents not on disk yet (or outside the
                    // listing) are items all the same.
                    let extra: Vec<(String, String)> = self
                        .open
                        .values()
                        .filter_map(|o| o.rel.clone().map(|r| (r, o.doc.text.clone())))
                        .filter(|(r, _)| !project.items.iter().any(|i| &i.path == r))
                        .collect();
                    for (rel, text) in extra {
                        project.update_file(&rel, Some(text.as_bytes()));
                    }
                    self.project = Some(project);
                    self.load_error = None;
                }
                Err(e) => {
                    self.project = None;
                    self.load_error = Some(e.to_string());
                }
            }
            return true;
        }
        if self.stale.is_empty() {
            return false;
        }
        let stale = std::mem::take(&mut self.stale);
        let mut texts: Vec<(String, Option<Vec<u8>>)> = Vec::new();
        if let Some(source) = self.source() {
            for rel in stale {
                texts.push((rel.clone(), source.read(&rel).ok()));
            }
        }
        let Some(project) = &mut self.project else {
            return false;
        };
        for (rel, bytes) in &texts {
            project.update_file(rel, bytes.as_deref());
        }
        true
    }

    /// Every finding, through the open documents.
    pub fn findings(&self, today: Date) -> Vec<Finding> {
        match (&self.project, self.source()) {
            (Some(project), Some(source)) => kiln_check::check(project, &source, today),
            _ => Vec::new(),
        }
    }

    /// Findings grouped by relative path.
    pub fn findings_by_file(&self, today: Date) -> BTreeMap<String, Vec<Finding>> {
        let mut out: BTreeMap<String, Vec<Finding>> = BTreeMap::new();
        for f in self.findings(today) {
            out.entry(f.path.clone()).or_default().push(f);
        }
        out
    }

    /// The document at `rel`: the editor's copy, else the disk's.
    pub fn doc(&self, rel: &str) -> Option<Doc> {
        if let Some(open) = self.by_rel.get(rel).and_then(|u| self.open.get(u)) {
            return Some(open.doc.clone());
        }
        let bytes = self.disk.as_ref()?.read(rel).ok()?;
        Some(Doc::new(&String::from_utf8_lossy(&bytes)))
    }

    /// A code file's text as it is on disk, unnormalised: renumbering
    /// edits code files at raw byte offsets.
    pub fn raw(&self, rel: &str) -> Option<String> {
        if let Some(open) = self.by_rel.get(rel).and_then(|u| self.open.get(u)) {
            return Some(open.doc.text.clone());
        }
        let bytes = self.disk.as_ref()?.read(rel).ok()?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// The URI for a relative path: the editor's spelling when open.
    pub fn uri(&self, rel: &str) -> Option<Uri> {
        if let Some(u) = self.by_rel.get(rel) {
            return u.parse().ok();
        }
        uri::from_path(&self.root.as_ref()?.join(rel))
    }

    /// The URIs of open documents inside the project, by relative path.
    pub fn open_in_project(&self) -> &HashMap<String, String> {
        &self.by_rel
    }
}
