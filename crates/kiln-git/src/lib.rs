//! What Kiln needs from git: the current commit and its date, whether the
//! tree is dirty, the last commit that changed each file (one
//! `git log` pass, for exports' repository links), whether the clone is
//! shallow, and whether a path is ignored. K6 adds revisions and tags.
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
}
