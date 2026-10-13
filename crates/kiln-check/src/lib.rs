//! The rules of docs/FORMAT.md, "Check rules": each with a code (`K102`),
//! a default severity overridable in `kiln.toml`'s `[check]`, and a span.
//! Writes findings as text, JSON, GitLab Code Quality and GitHub workflow
//! annotations, so the same run fails a merge request and shows each
//! finding on the line that caused it.
//!
//! Pure: it reads a loaded project (and asks the file source whether
//! relative link targets exist) and returns findings; today's date comes
//! from the caller, and the binary decides the exit code.
//!
//! It deliberately doesn't fix anything. Every fix (`kiln renumber`,
//! `kiln set`) is a deliberate edit by its own command.
//!
//! K005, K111 and K112 (exports, Mermaid, templates) arrive with K2 and
//! K207 with the Jira mirror in K8; `[check]` may name them already.

mod output;
mod rules;

pub use output::{render, summary, Format};
pub use rules::resolve_relative;

use kiln_index::{FileSource, Project};
use kiln_model::{Date, Finding, Severity};

/// Every finding for `project`, at the project's severities, without the
/// ones turned off, sorted by file, line, column and rule.
pub fn check(project: &Project, files: &dyn FileSource, today: Date) -> Vec<Finding> {
    let mut findings: Vec<Finding> = rules::findings(project, files, today)
        .into_iter()
        .filter_map(|mut f| {
            f.severity = project.config.severity(f.rule);
            (f.severity != Severity::Off).then_some(f)
        })
        .collect();
    findings.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    findings.dedup();
    findings
}

/// Whether any finding is an error (`kiln check` exits 1).
pub fn has_errors(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.severity == Severity::Error)
}

#[cfg(test)]
mod tests;
