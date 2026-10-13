//! Notices files changed on disk by someone other than the editor: `kiln
//! new` in a terminal, a `git pull`, a branch switch. A client may send
//! `workspace/didChangeWatchedFiles`, but Fenix doesn't (G8), so the
//! server polls: item files and `kiln.toml` every two seconds (a few
//! thousand `stat` calls), code files every ten (they can be tens of
//! thousands). It runs on its own thread and only reports paths; the
//! main loop decides what to re-read.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use kiln_index::{DiskSource, FileSource, Stamp};

/// What to watch: the project's folders as configured.
#[derive(Debug, Clone)]
pub struct Watch {
    pub root: PathBuf,
    pub item_dirs: Vec<String>,
    pub code_dirs: Vec<String>,
    pub code_exclude: Vec<String>,
}

/// Paths that changed (added, modified or removed), and code files that
/// are new.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    pub paths: Vec<String>,
    pub new_code_files: Vec<String>,
}

const ITEMS_EVERY: Duration = Duration::from_secs(2);
const CODE_EVERY: Duration = Duration::from_secs(10);

/// Runs until `config` disconnects.
pub fn run(config: Receiver<Watch>, out: Sender<Changes>) {
    let mut watch: Option<Watch> = None;
    let mut items: Option<HashMap<String, Stamp>> = None;
    let mut code: Option<HashMap<String, Stamp>> = None;
    let mut code_at = Instant::now();
    loop {
        match config.recv_timeout(ITEMS_EVERY) {
            Ok(w) => {
                watch = Some(w);
                items = None;
                code = None;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let Some(w) = &watch else {
            continue;
        };
        let disk = DiskSource::new(&w.root);
        let mut changes = Changes::default();
        let mut now = stamps(&disk, &w.item_dirs, &[], Some("md"));
        if let Ok(s) = Stamp::of(&w.root.join(kiln_parse::CONFIG_FILE)) {
            now.insert(kiln_parse::CONFIG_FILE.to_owned(), s);
        }
        if let Some(before) = &items {
            changes.paths.extend(diff(before, &now));
        }
        items = Some(now);
        if code.is_none() || code_at.elapsed() >= CODE_EVERY {
            code_at = Instant::now();
            let now = stamps(&disk, &w.code_dirs, &w.code_exclude, None);
            if let Some(before) = &code {
                changes
                    .new_code_files
                    .extend(now.keys().filter(|p| !before.contains_key(*p)).cloned());
                changes.paths.extend(diff(before, &now));
            }
            code = Some(now);
        }
        changes.paths.sort();
        changes.paths.dedup();
        changes.new_code_files.sort();
        if !changes.paths.is_empty() && out.send(changes).is_err() {
            return;
        }
    }
}

fn stamps(
    disk: &DiskSource,
    dirs: &[String],
    exclude: &[String],
    ext: Option<&str>,
) -> HashMap<String, Stamp> {
    if dirs.is_empty() {
        return HashMap::new();
    }
    let Ok(listing) = disk.list(dirs, exclude, ext) else {
        return HashMap::new();
    };
    listing
        .files
        .into_iter()
        .filter_map(|p| Stamp::of(&disk.path(&p)).ok().map(|s| (p, s)))
        .collect()
}

fn diff(before: &HashMap<String, Stamp>, now: &HashMap<String, Stamp>) -> Vec<String> {
    let mut out: Vec<String> = now
        .iter()
        .filter(|(p, s)| before.get(*p) != Some(s))
        .map(|(p, _)| p.clone())
        .collect();
    out.extend(before.keys().filter(|p| !now.contains_key(*p)).cloned());
    out
}
