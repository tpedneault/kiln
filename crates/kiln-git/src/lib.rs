//! What Kiln needs from git. K1: the current commit, which `kiln trace`
//! names so a coverage report says what it describes. K2 adds the last
//! commit per file and shallow-clone detection, K6 revisions and tags.
//!
//! Shells out to `git`, as `fenix-git` does, rather than linking a git
//! library: the user's own git already honours their credentials,
//! configuration and repository format, and CI images have it. The cost
//! is a process per query, which is why later queries are batched.
//!
//! I/O, read-only: it never commits, checks out or changes a ref. Git
//! being absent, or the project not being a repository, is an ordinary
//! answer, not a failure: Kiln works without git and says so.

use std::path::Path;
use std::process::Command;

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
}
