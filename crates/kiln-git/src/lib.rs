//! What Kiln needs from git: the current commit and its date, whether the
//! tree is dirty, the last commit that changed each file (one
//! `git log` pass, for exports' repository links), whether the clone is
//! shallow, and whether a path is ignored; and (K6) a revision resolved
//! to its commit, the files at a revision and the tags, for `kiln diff`,
//! `kiln export --since` and the GUI's baseline; and (K18) the lines
//! changed since a revision, for `kiln affected`.
//!
//! Shells out to `git`, as `fenix-git` does, rather than linking a git
//! library: the user's own git already honours their credentials,
//! configuration and repository format, and CI images have it. The cost
//! is a process per query, which is why later queries are batched.
//!
//! I/O, read-only: it never commits, checks out or changes a ref. Git
//! being absent, or the project not being a repository, is an ordinary
//! answer, not a failure: Kiln works without git and says so.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Output};

/// Why there is no commit to name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitError {
    /// `git` isn't on the PATH.
    NotInstalled,
    /// The directory isn't in a repository, or it has no commits yet.
    NoCommit,
    /// git ran and failed otherwise; its first line of stderr.
    Failed(String),
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::NotInstalled => write!(f, "git isn't installed"),
            GitError::NoCommit => write!(f, "not a git repository with commits"),
            GitError::Failed(e) => write!(f, "git failed: {e}"),
        }
    }
}

impl std::error::Error for GitError {}

/// The full id of `HEAD` for the repository containing `dir`.
pub fn head_commit(dir: &Path) -> Result<String, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--verify", "--quiet", "HEAD"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => GitError::NotInstalled,
            _ => GitError::Failed(e.to_string()),
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if output.status.success()
        && stdout.len() >= 40
        && stdout.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Ok(stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let first = stderr.lines().next().unwrap_or("").trim();
    if first.is_empty() || first.contains("not a git repository") {
        Err(GitError::NoCommit)
    } else {
        Err(GitError::Failed(first.to_owned()))
    }
}

fn git(dir: &Path, args: &[&str]) -> Result<Output, GitError> {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "core.quotepath=off"])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => GitError::NotInstalled,
            _ => GitError::Failed(e.to_string()),
        })
}

fn failed(output: &Output) -> GitError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let first = stderr.lines().next().unwrap_or("").trim();
    if first.is_empty() || first.contains("not a git repository") {
        GitError::NoCommit
    } else {
        GitError::Failed(first.to_owned())
    }
}

/// The committer date of `commit`, `YYYY-MM-DD` in the committer's own
/// time zone, so every machine prints the same.
pub fn commit_date(dir: &Path, commit: &str) -> Result<String, GitError> {
    let output = git(dir, &["show", "-s", "--format=%cs", commit])?;
    if !output.status.success() {
        return Err(failed(&output));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// The last commit that changed each file under `dir`, by path relative
/// to `dir` with `/`, in one pass over the history. Files never committed
/// are absent.
pub fn last_commits(dir: &Path) -> Result<HashMap<String, String>, GitError> {
    let output = git(
        dir,
        &[
            "log",
            "--format=%x01%H",
            "--name-only",
            "--no-renames",
            "--relative",
            "--",
            ".",
        ],
    )?;
    if !output.status.success() {
        return Err(failed(&output));
    }
    let mut out = HashMap::new();
    let mut commit = String::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some(hash) = line.strip_prefix('\u{1}') {
            commit = hash.trim().to_owned();
        } else if !line.is_empty() && !commit.is_empty() {
            out.entry(line.to_owned()).or_insert_with(|| commit.clone());
        }
    }
    Ok(out)
}

/// Whether the clone is shallow, so older files' last commits are
/// unknown (EXPORT.md, "Which commit a link names").
pub fn is_shallow(dir: &Path) -> Result<bool, GitError> {
    let output = git(dir, &["rev-parse", "--is-shallow-repository"])?;
    if !output.status.success() {
        return Err(failed(&output));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim() == "true")
}

/// Whether anything under `dir` is modified, staged or untracked.
pub fn is_dirty(dir: &Path) -> Result<bool, GitError> {
    let output = git(dir, &["status", "--porcelain", "--", "."])?;
    if !output.status.success() {
        return Err(failed(&output));
    }
    Ok(!output.stdout.is_empty())
}

/// Whether git ignores `path` (relative to `dir`).
pub fn is_ignored(dir: &Path, path: &str) -> Result<bool, GitError> {
    let output = git(dir, &["check-ignore", "-q", "--no-index", "--", path])?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(failed(&output)),
    }
}

/// The commit `rev` names (a tag, a branch, `HEAD~3`, an id). A revision
/// starting with `-` is refused rather than handed to git, where it would
/// read as an option.
pub fn resolve(dir: &Path, rev: &str) -> Result<String, GitError> {
    if rev.starts_with('-') || rev.trim().is_empty() {
        return Err(GitError::Failed(format!("`{rev}` isn't a revision")));
    }
    let spec = format!("{rev}^{{commit}}");
    let output = git(
        dir,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &spec,
        ],
    )?;
    let id = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if output.status.success() && id.len() >= 40 {
        return Ok(id);
    }
    match failed(&output) {
        GitError::NoCommit if head_commit(dir).is_ok() => Err(GitError::Failed(format!(
            "`{rev}` isn't a revision in this repository"
        ))),
        other => Err(other),
    }
}

/// The most recent tag reachable from `HEAD`, for a default baseline;
/// `None` when there is none.
pub fn latest_tag(dir: &Path) -> Option<String> {
    let output = git(dir, &["describe", "--tags", "--abbrev=0", "HEAD"]).ok()?;
    let tag = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (output.status.success() && !tag.is_empty()).then_some(tag)
}

/// The repository's tags, the most recently created first: what the GUI
/// offers as a baseline. No tags (or no repository) is an empty list.
pub fn tags(dir: &Path) -> Result<Vec<String>, GitError> {
    let output = git(dir, &["tag", "--list", "--sort=-creatordate"])?;
    if !output.status.success() {
        return Err(failed(&output));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect())
}

/// The files under `dir` at `commit` that `keep` accepts, by path relative
/// to `dir` with `/`, with their contents: one `git ls-tree` and one
/// `git cat-file --batch` for all of them, however many there are.
pub fn files_at(
    dir: &Path,
    commit: &str,
    keep: &dyn Fn(&str) -> bool,
) -> Result<Vec<(String, Vec<u8>)>, GitError> {
    let output = git(
        dir,
        &["ls-tree", "-r", "-z", "--name-only", commit, "--", "."],
    )?;
    if !output.status.success() {
        return Err(failed(&output));
    }
    let paths: Vec<String> = output
        .stdout
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .filter(|p| keep(p))
        .collect();
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut child = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["cat-file", "--batch"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| GitError::Failed(e.to_string()))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| GitError::Failed("cat-file has no input".into()))?;
    // Fed from a thread: git answers while it reads, and a full pipe
    // either way would otherwise stall both sides.
    let requests: String = paths.iter().map(|p| format!("{commit}:./{p}\n")).collect();
    let feeder = std::thread::spawn(move || {
        use std::io::Write;
        let _ = stdin.write_all(requests.as_bytes());
    });
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| GitError::Failed("cat-file has no output".into()))?;
    let mut all = Vec::new();
    std::io::Read::read_to_end(&mut stdout, &mut all)
        .map_err(|e| GitError::Failed(e.to_string()))?;
    let _ = feeder.join();
    let _ = child.wait();
    let mut out = Vec::with_capacity(paths.len());
    let mut at = 0;
    for path in paths {
        let Some(nl) = all[at..].iter().position(|b| *b == b'\n') else {
            return Err(GitError::Failed(format!("cat-file stopped before {path}")));
        };
        let header = String::from_utf8_lossy(&all[at..at + nl]).into_owned();
        at += nl + 1;
        let mut fields = header.split(' ');
        let (_, kind, size) = (fields.next(), fields.next(), fields.next());
        let Some(size) = size.and_then(|s| s.parse::<usize>().ok()) else {
            // `missing` (a submodule, a race): nothing to read.
            continue;
        };
        if at + size > all.len() {
            return Err(GitError::Failed(format!("cat-file cut {path} short")));
        }
        if kind == Some("blob") {
            out.push((path, all[at..at + size].to_vec()));
        }
        at += size + 1;
    }
    Ok(out)
}

/// The lines changed since `commit`, in the working tree (tracked files,
/// committed or not), by path relative to `dir` with `/`: each a list of
/// `(first, last)` 1-based line ranges in the file as it is now; a pure
/// deletion is the line it was deleted before, `(n, n)`. Deleted files
/// are left out (nothing of theirs is there to touch). For `kiln affected`
/// (K18); untracked files aren't seen, as `git diff` doesn't.
pub fn changed_lines(
    dir: &Path,
    commit: &str,
) -> Result<HashMap<String, Vec<(u32, u32)>>, GitError> {
    let output = git(
        dir,
        &[
            "diff",
            "--relative",
            "--unified=0",
            "--no-color",
            "--no-ext-diff",
            "--no-renames",
            commit,
            "--",
            ".",
        ],
    )?;
    if !output.status.success() {
        return Err(failed(&output));
    }
    Ok(parse_changed_lines(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

/// [`changed_lines`] from `git diff --unified=0` output.
pub fn parse_changed_lines(diff: &str) -> HashMap<String, Vec<(u32, u32)>> {
    let mut out: HashMap<String, Vec<(u32, u32)>> = HashMap::new();
    let mut current: Option<String> = None;
    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ ") {
            current = path
                .strip_prefix("b/")
                .filter(|_| path != "/dev/null")
                .map(str::to_owned);
            continue;
        }
        let Some(hunk) = line.strip_prefix("@@ ") else {
            continue;
        };
        let Some(path) = &current else { continue };
        // `@@ -a,b +c,d @@`: the new side is `+c,d`, `d` defaulting to 1.
        let Some(new) = hunk.split_whitespace().find(|w| w.starts_with('+')) else {
            continue;
        };
        let mut parts = new[1..].splitn(2, ',');
        let start: u32 = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0);
        let count: u32 = parts.next().and_then(|n| n.parse().ok()).unwrap_or(1);
        let range = if count == 0 {
            (start.max(1), start.max(1))
        } else {
            (start, start + count - 1)
        };
        out.entry(path.clone()).or_default().push(range);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=Kiln",
                "-c",
                "user.email=kiln@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stderr)
        );
    }

    #[test]
    fn head_of_a_repository_with_a_commit() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(head_commit(dir.path()), Err(GitError::NoCommit));
        git(dir.path(), &["init", "-q"]);
        assert_eq!(
            head_commit(dir.path()),
            Err(GitError::NoCommit),
            "no commits yet"
        );
        std::fs::write(dir.path().join("a"), "a").unwrap();
        git(dir.path(), &["add", "a"]);
        git(dir.path(), &["commit", "-q", "-m", "a"]);
        let id = head_commit(dir.path()).unwrap();
        assert_eq!(id.len(), 40);
    }

    #[test]
    fn last_commits_dates_dirt_and_ignores() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        git(d, &["init", "-q"]);
        std::fs::create_dir_all(d.join("sub/design")).unwrap();
        std::fs::write(d.join("sub/design/a.md"), "a").unwrap();
        std::fs::write(d.join("sub/b.md"), "b").unwrap();
        std::fs::write(d.join(".gitignore"), "target/\n").unwrap();
        git(d, &["add", "."]);
        git(
            d,
            &[
                "commit",
                "-q",
                "-m",
                "one",
                "--date=2026-01-02T10:00:00+00:00",
            ],
        );
        let first = head_commit(d).unwrap();
        std::fs::write(d.join("sub/b.md"), "b2").unwrap();
        assert!(is_dirty(&d.join("sub")).unwrap());
        git(d, &["commit", "-q", "-am", "two"]);
        let second = head_commit(d).unwrap();
        let last = last_commits(&d.join("sub")).unwrap();
        assert_eq!(last.get("design/a.md"), Some(&first));
        assert_eq!(last.get("b.md"), Some(&second));
        assert_eq!(last.len(), 2, "{last:?}");
        assert!(!is_dirty(&d.join("sub")).unwrap());
        assert!(!is_shallow(d).unwrap());
        assert_eq!(commit_date(d, &first).unwrap().len(), 10);
        assert!(is_ignored(d, "target/x.html").unwrap());
        assert!(!is_ignored(d, "docs/x.md").unwrap());
    }

    #[test]
    fn files_at_a_revision_from_a_subfolder() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        git(d, &["init", "-q"]);
        std::fs::create_dir_all(d.join("sub/design")).unwrap();
        std::fs::write(d.join("sub/design/a.md"), "old a").unwrap();
        std::fs::write(d.join("sub/design/b.md"), "b").unwrap();
        std::fs::write(d.join("sub/x.rs"), "x").unwrap();
        std::fs::write(d.join("top.md"), "top").unwrap();
        git(d, &["add", "."]);
        git(d, &["commit", "-q", "-m", "one"]);
        git(d, &["tag", "v1"]);
        std::fs::write(d.join("sub/design/a.md"), "new a").unwrap();
        git(d, &["commit", "-q", "-am", "two"]);
        assert_eq!(latest_tag(d).as_deref(), Some("v1"));
        assert_eq!(tags(d).unwrap(), ["v1"]);
        let v1 = resolve(&d.join("sub"), "v1").unwrap();
        assert_eq!(resolve(d, "HEAD~1").unwrap(), v1);
        assert!(
            resolve(d, "--output=x").is_err(),
            "an option isn't a revision"
        );
        assert!(resolve(d, "nope")
            .unwrap_err()
            .to_string()
            .contains("`nope` isn't a revision"));
        let files = files_at(&d.join("sub"), &v1, &|p| p.ends_with(".md")).unwrap();
        assert_eq!(
            files,
            vec![
                ("design/a.md".to_owned(), b"old a".to_vec()),
                ("design/b.md".to_owned(), b"b".to_vec())
            ]
        );
    }

    #[test]
    fn unified_zero_diffs_give_the_new_sides_lines() {
        let diff = "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -3 +3 @@ fn x\n-a\n+b\n@@ -10,2 +11,4 @@\n+c\n@@ -20,3 +23,0 @@\n-d\ndiff --git a/old.rs b/old.rs\n--- a/old.rs\n+++ /dev/null\n@@ -1,5 +0,0 @@\n";
        let changed = parse_changed_lines(diff);
        assert_eq!(changed["src/a.rs"], [(3, 3), (11, 14), (23, 23)]);
        assert!(!changed.contains_key("old.rs"));
    }
}
