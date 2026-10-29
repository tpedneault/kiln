//! `kiln signoff [REV]`: every change to a sign-off status between `REV`
//! (default `HEAD`) and the working tree (K19, REQ-024, ADR-023): a
//! requirement approved, a decision accepted, a risk closed, a milestone
//! finished, or whatever `[signoff]` says, including an item created with
//! such a status. Each is a K218 finding on the status line naming the
//! commit and author that made it, so `--format github` or `gitlab` puts
//! it in front of the pull request's reviewers: the agent guide leaves
//! sign-off to people, and this shows when a change didn't.
//!
//! It reads git and changes nothing. Exit 0 unless `[check] K218` makes
//! it an error, for teams that want sign-off in a pull request of its own.

use std::path::Path;

use kiln_check::{render, summary, Format};
use kiln_index::{diff, ChangeKind, Detail};
use kiln_model::{Finding, Rule, Severity};

use crate::project::{at_revision, load};
use crate::{CheckFormat, Fail, EXIT_FINDINGS};

// implements: REQ-024
pub fn run(project: Option<&Path>, rev: &str, format: CheckFormat) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let root = loaded.source.path("");
    let (old, commit) = at_revision(&root, rev)?;
    let severity = p.config.severity(Rule::K218);
    let mut findings = Vec::new();
    for change in diff(&old, p) {
        let signoff = p.config.signoff_statuses(&change.type_name);
        let moved: Option<String> = match change.kind {
            ChangeKind::Changed => change.details.iter().find_map(|d| match d {
                Detail::Status { from, to } if signoff.contains(&to.as_str()) => {
                    Some(format!("status {from} → {to}"))
                }
                _ => None,
            }),
            ChangeKind::Added => p
                .item(&change.id)
                .filter(|i| signoff.contains(&i.status.as_str()))
                .map(|i| format!("created as {}", i.status)),
            ChangeKind::Removed => None,
        };
        let Some(moved) = moved else {
            continue;
        };
        let Some(item) = p.item(&change.id) else {
            continue;
        };
        let by = match kiln_git::last_change(&root, &commit, &item.path)
            .map_err(|e| Fail::usage(e.to_string()))?
        {
            Some(r) => format!(
                "in {} by {} ({})",
                &r.commit[..r.commit.len().min(7)],
                r.author,
                r.date
            ),
            None => "not committed yet".to_owned(),
        };
        let mut f = Finding::new(
            Rule::K218,
            &item.path,
            item.status_span,
            format!(
                "{} {}: {moved}, {by}; sign-off is a person's call, so check it was meant",
                item.id, item.title
            ),
        );
        f.severity = severity;
        findings.push(f);
    }
    findings.retain(|f| f.severity != Severity::Off);
    findings.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    let format = match format {
        CheckFormat::Text => Format::Text,
        CheckFormat::Json => Format::Json,
        CheckFormat::Gitlab => Format::Gitlab,
        CheckFormat::Github => Format::Github,
    };
    print!("{}", render(&findings, format));
    let short = &commit[..commit.len().min(7)];
    eprintln!(
        "kiln: since {rev} ({short}): {}",
        match findings.len() {
            0 => "no sign-off status changed".to_owned(),
            _ => summary(&findings),
        }
    );
    Ok(if findings.iter().any(|f| f.severity == Severity::Error) {
        EXIT_FINDINGS
    } else {
        0
    })
}
