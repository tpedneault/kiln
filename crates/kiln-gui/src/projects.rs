//! The projects the window knows (GUI.md, "Home"): `projects.toml` in
//! Kiln's state folder, the list Home shows and opens from. A project
//! joins it when it is opened, from the command line or from Home, or
//! when it is added on Home; removing one takes it off the list and
//! leaves its folder alone.
//!
//! The state folder, not `settings.toml`: the paths are this machine's,
//! so the list must not roam with the settings on Windows. Kiln rewrites
//! the file whole, so it keeps no comments; a file Kiln can't read is an
//! empty list with a line in the footer, never an error, since losing a
//! list of shortcuts must not stop the window.
//!
//! Parsing, ordering and summarising are plain functions; reading and
//! writing the file, listing folders for completion and loading a
//! project for its summary are the I/O, kept small.

use std::path::{Path, PathBuf};

use kiln_index::{DiskSource, Project};
use kiln_model::{Date, Severity};

/// The file's name in the state folder.
pub const FILE: &str = "projects.toml";

/// One project on the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Known {
    /// The folder with `kiln.toml`.
    pub root: PathBuf,
    /// When it was last opened, in seconds since 1970; 0 for never.
    pub opened: u64,
}

/// The list, most recently opened first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Projects {
    pub list: Vec<Known>,
}

impl Projects {
    /// Reads the file's text; returns the list and what was wrong with
    /// it, one line each. Entries without a path are skipped; the same
    /// folder twice is kept once.
    pub fn parse(text: &str) -> (Projects, Vec<String>) {
        let doc: toml_edit::DocumentMut = match text.parse() {
            Ok(d) => d,
            Err(e) => {
                let first = e.to_string().lines().next().unwrap_or("").to_owned();
                return (
                    Projects::default(),
                    vec![format!(
                        "{FILE} isn't valid TOML ({first}); Home starts empty"
                    )],
                );
            }
        };
        let mut out = Projects::default();
        let mut problems = Vec::new();
        if let Some(tables) = doc.get("project").and_then(|p| p.as_array_of_tables()) {
            for (n, t) in tables.iter().enumerate() {
                let Some(path) = t.get("path").and_then(|p| p.as_str()) else {
                    problems.push(format!("{FILE}: project {} has no path", n + 1));
                    continue;
                };
                let opened = t
                    .get("opened")
                    .and_then(|o| o.as_integer())
                    .and_then(|o| u64::try_from(o).ok())
                    .unwrap_or(0);
                out.insert(Known {
                    root: PathBuf::from(path),
                    opened,
                });
            }
        }
        out.sort();
        (out, problems)
    }

    /// The file's text.
    pub fn render(&self) -> String {
        let mut doc = toml_edit::DocumentMut::new();
        let mut tables = toml_edit::ArrayOfTables::new();
        for k in &self.list {
            let mut t = toml_edit::Table::new();
            t.insert("path", toml_edit::value(k.root.to_string_lossy().as_ref()));
            t.insert(
                "opened",
                toml_edit::value(k.opened.min(i64::MAX as u64) as i64),
            );
            tables.push(t);
        }
        doc.insert("project", toml_edit::Item::ArrayOfTables(tables));
        format!(
            "# The projects Kiln's window knows, most recently opened first.\n\
             # Kiln rewrites this file; remove a project on Home with `d`.\n\n{doc}"
        )
    }

    /// Adds `k`, or keeps the later of the two opening times when its
    /// folder is already there.
    fn insert(&mut self, k: Known) {
        match self.list.iter_mut().find(|o| same(&o.root, &k.root)) {
            Some(o) => o.opened = o.opened.max(k.opened),
            None => self.list.push(k),
        }
    }

    /// Most recently opened first; never-opened ones by path.
    fn sort(&mut self) {
        self.list
            .sort_by(|a, b| b.opened.cmp(&a.opened).then_with(|| a.root.cmp(&b.root)));
    }

    /// `root` was opened at `now`: on the list, first.
    pub fn opened(&mut self, root: &Path, now: u64) {
        self.insert(Known {
            root: root.to_path_buf(),
            opened: now,
        });
        if let Some(k) = self.list.iter_mut().find(|k| same(&k.root, root)) {
            k.opened = now;
        }
        self.sort();
    }

    /// `root` was added without being opened: on the list, after the
    /// ones opened before it. `false` when it was there already.
    pub fn add(&mut self, root: &Path) -> bool {
        if self.contains(root) {
            return false;
        }
        self.list.push(Known {
            root: root.to_path_buf(),
            opened: 0,
        });
        self.sort();
        true
    }

    /// Takes `root` off the list; the folder is untouched.
    pub fn remove(&mut self, root: &Path) -> bool {
        let before = self.list.len();
        self.list.retain(|k| !same(&k.root, root));
        self.list.len() != before
    }

    pub fn contains(&self, root: &Path) -> bool {
        self.list.iter().any(|k| same(&k.root, root))
    }

    /// Reads the file at `path`: a missing file is an empty list.
    pub fn load(path: &Path) -> (Projects, Vec<String>) {
        match std::fs::read_to_string(path) {
            Ok(text) => Projects::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Projects::default(), Vec::new()),
            Err(e) => (
                Projects::default(),
                vec![format!("{}: can't read it: {e}", path.display())],
            ),
        }
    }

    /// Writes the file at `path`, through a temporary file beside it so
    /// a crash never leaves half a list.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: can't write it: {e}", path.display());
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(fail)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, self.render()).map_err(fail)?;
        std::fs::rename(&tmp, path).map_err(fail)
    }
}

/// The same folder: compared as written, then case-insensitively on
/// Windows, where `C:\Src` and `c:\src` are one folder.
fn same(a: &Path, b: &Path) -> bool {
    a == b
        || (cfg!(windows)
            && a.to_string_lossy()
                .eq_ignore_ascii_case(&b.to_string_lossy()))
}

/// Seconds since 1970, for `opened`.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// "now", "5 min", "3 h", "2 d", "6 w" since `then`: Fenix's ages.
pub fn age(then: u64, now: u64) -> String {
    if then == 0 {
        return "never opened".into();
    }
    let mins = now.saturating_sub(then) / 60;
    match mins {
        0 => "now".into(),
        1..=59 => format!("{mins} min"),
        60..=1439 => format!("{} h", mins / 60),
        1440..=20159 => format!("{} d", mins / 1440),
        _ => format!("{} w", mins / 10080),
    }
}

/// What Home's preview shows of a project, read from disk on a thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// `kiln.toml`'s `name`.
    pub name: String,
    pub items: usize,
    /// Items that aren't closed.
    pub open: usize,
    /// Item counts by type, in `kiln.toml`'s order, types with none left out.
    pub types: Vec<(String, usize)>,
    /// The first open milestone by due date.
    pub milestone: Option<Milestone>,
    /// `kiln check`'s errors and warnings.
    pub errors: usize,
    pub warnings: usize,
}

/// A project's next milestone, as Home's preview shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Milestone {
    /// `MS-003 Beta`.
    pub title: String,
    pub due: Option<Date>,
    /// Done and total of what is `part-of` it.
    pub progress: Option<(usize, usize)>,
}

/// How a project fared: what Home's dot shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Clean,
    Warnings,
    Errors,
    /// No `kiln.toml` there, or one Kiln can't use.
    Missing,
}

impl Summary {
    pub fn health(&self) -> Health {
        if self.errors > 0 {
            Health::Errors
        } else if self.warnings > 0 {
            Health::Warnings
        } else {
            Health::Clean
        }
    }

    /// Summarises a loaded project and its findings.
    pub fn of(p: &Project, findings: &[kiln_model::Finding]) -> Summary {
        let types = p
            .config
            .types
            .iter()
            .map(|t| {
                let n = p.items.iter().filter(|i| i.type_name == t.name).count();
                (t.name.clone(), n)
            })
            .filter(|(_, n)| *n > 0)
            .collect();
        let milestone = p.config.type_named("milestone").and_then(|ty| {
            p.items
                .iter()
                .filter(|i| i.type_name == ty.name && !p.is_closed(i) && !p.is_done(i))
                .min_by(|a, b| {
                    (a.due.is_none(), a.due, &a.id).cmp(&(b.due.is_none(), b.due, &b.id))
                })
                .map(|m| Milestone {
                    title: format!("{} {}", m.id, m.title),
                    due: m.due,
                    progress: p.progress(&m.id).map(|pr| (pr.done, pr.total)),
                })
        });
        Summary {
            name: p.config.name.clone(),
            items: p.items.len(),
            open: p.items.iter().filter(|i| !p.is_closed(i)).count(),
            types,
            milestone,
            errors: findings
                .iter()
                .filter(|f| f.severity == Severity::Error)
                .count(),
            warnings: findings
                .iter()
                .filter(|f| f.severity == Severity::Warning)
                .count(),
        }
    }
}

/// Reads the project at `root` and runs `kiln check` on it, for Home's
/// preview; the error says why there is nothing to show.
pub fn summarize(root: &Path, today: Date) -> Result<Summary, String> {
    if !root.join(kiln_parse::CONFIG_FILE).is_file() {
        return Err(format!("no kiln.toml in {}", root.display()));
    }
    let source = DiskSource::new(root);
    let project = Project::load(&source).map_err(|e| e.to_string())?;
    let findings = kiln_check::check(&project, &source, today);
    Ok(Summary::of(&project, &findings))
}

/// Completes the folder being typed: the longest name every subfolder
/// starting with what follows the last separator shares, with a
/// separator after it when only one matches. `None` when nothing matches
/// or there is nothing to add. `~` stands for the home folder, as in a
/// shell.
pub fn complete(input: &str, home: Option<&Path>) -> Option<String> {
    let expanded = expand(input, home);
    let sep = |c: char| c == '/' || (cfg!(windows) && c == '\\');
    let cut = expanded.rfind(sep).map_or(0, |i| i + 1);
    let (dir, stem) = expanded.split_at(cut);
    let listed = if dir.is_empty() { "." } else { dir };
    let mut names: Vec<String> = std::fs::read_dir(listed)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with(stem) && (!n.starts_with('.') || stem.starts_with('.')))
        .collect();
    names.sort();
    let first = names.first()?;
    let common = names.iter().fold(first.clone(), |acc, n| {
        acc.chars()
            .zip(n.chars())
            .take_while(|(a, b)| a == b)
            .map(|(a, _)| a)
            .collect()
    });
    let mut out = format!("{dir}{common}");
    if names.len() == 1 {
        out.push(std::path::MAIN_SEPARATOR);
    }
    (out != input).then_some(out)
}

/// `~` and `~/…` as the home folder.
pub fn expand(input: &str, home: Option<&Path>) -> String {
    match (input.strip_prefix('~'), home) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with(['/', '\\']) => {
            format!("{}{rest}", home.display())
        }
        _ => input.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_reads_back_what_it_writes_most_recent_first() {
        let mut p = Projects::default();
        p.opened(Path::new("/src/kiln"), 100);
        p.opened(Path::new("/src/flint"), 300);
        assert!(p.add(Path::new("/src/fenix")));
        assert!(!p.add(Path::new("/src/kiln")), "already there");
        p.opened(Path::new("/src/kiln"), 400);
        let roots: Vec<_> = p.list.iter().map(|k| k.root.clone()).collect();
        assert_eq!(
            roots,
            ["/src/kiln", "/src/flint", "/src/fenix"].map(PathBuf::from)
        );
        let (back, problems) = Projects::parse(&p.render());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(back, p);
        assert!(p.remove(Path::new("/src/flint")));
        assert!(!p.remove(Path::new("/src/flint")));
        assert_eq!(p.list.len(), 2);
    }

    #[test]
    fn a_bad_file_is_an_empty_list_with_a_reason() {
        let (p, problems) = Projects::parse("[[project]\n");
        assert!(p.list.is_empty());
        assert!(problems[0].starts_with("projects.toml isn't valid TOML"));
        let (p, problems) = Projects::parse(
            "[[project]]\nopened = 5\n[[project]]\npath = \"/a\"\nopened = -3\n[[project]]\npath = \"/a\"\nopened = 9\n",
        );
        assert_eq!(
            p.list,
            [Known {
                root: "/a".into(),
                opened: 9
            }]
        );
        assert_eq!(problems, ["projects.toml: project 1 has no path"]);
    }

    #[test]
    fn saving_and_loading_go_through_the_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state").join(FILE);
        assert_eq!(Projects::load(&path), (Projects::default(), Vec::new()));
        let mut p = Projects::default();
        p.opened(Path::new("/x"), 1);
        p.save(&path).unwrap();
        assert_eq!(Projects::load(&path).0, p);
    }

    #[test]
    fn ages_read_like_fenixs() {
        assert_eq!(age(0, 100), "never opened");
        assert_eq!(age(100, 130), "now");
        assert_eq!(age(1, 1 + 5 * 60), "5 min");
        assert_eq!(age(1, 1 + 3 * 3600), "3 h");
        assert_eq!(age(1, 1 + 2 * 86_400), "2 d");
        assert_eq!(age(1, 1 + 6 * 7 * 86_400), "6 w");
    }

    #[test]
    fn folders_complete_like_a_shell() {
        let dir = tempfile::tempdir().unwrap();
        for d in ["alpha", "alps", "beta", ".hidden"] {
            std::fs::create_dir(dir.path().join(d)).unwrap();
        }
        std::fs::write(dir.path().join("alfile"), "").unwrap();
        let base = format!("{}{}", dir.path().display(), std::path::MAIN_SEPARATOR);
        assert_eq!(
            complete(&format!("{base}a"), None),
            Some(format!("{base}alp"))
        );
        assert_eq!(
            complete(&format!("{base}b"), None),
            Some(format!("{base}beta{}", std::path::MAIN_SEPARATOR))
        );
        assert_eq!(
            complete(&format!("{base}alp"), None),
            None,
            "nothing to add"
        );
        assert_eq!(complete(&format!("{base}z"), None), None);
        assert_eq!(
            complete(&format!("{base}."), None),
            Some(format!("{base}.hidden{}", std::path::MAIN_SEPARATOR))
        );
        assert_eq!(expand("~/src", Some(Path::new("/home/t"))), "/home/t/src");
        assert_eq!(expand("~x", Some(Path::new("/home/t"))), "~x");
    }

    #[test]
    fn a_summary_counts_items_findings_and_the_next_milestone() {
        let source = kiln_index::MemorySource::new()
            .with("kiln.toml", kiln_model::default_config("Demo"))
            .with(
                "design/REQ-001.md",
                "+++\nid = \"REQ-001\"\ntype = \"requirement\"\ntitle = \"Fast\"\nstatus = \"approved\"\n+++\n",
            )
            .with(
                "design/MS-001.md",
                "+++\nid = \"MS-001\"\ntype = \"milestone\"\ntitle = \"Beta\"\nstatus = \"active\"\ndue = 2026-11-30\n+++\n",
            );
        let p = Project::load(&source).unwrap();
        let s = Summary::of(&p, &[]);
        assert_eq!(s.name, "Demo");
        assert_eq!((s.items, s.open), (2, 2));
        assert_eq!(
            s.types,
            [("requirement".to_owned(), 1), ("milestone".to_owned(), 1)]
        );
        let m = s.milestone.clone().unwrap();
        assert_eq!(m.title, "MS-001 Beta");
        assert_eq!(m.due.map(|d| d.to_string()).as_deref(), Some("2026-11-30"));
        assert_eq!(s.health(), Health::Clean);
    }
}
