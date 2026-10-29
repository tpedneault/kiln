//! `kiln check`: the findings in the asked format on stdout, a summary on
//! stderr, exit 1 on any error. K207 reads the Jira mirror when it is on.

use std::path::Path;

use kiln_check::{check, has_errors, render, summary, Format};
use kiln_index::{DiskSource, LoadError, Project};
use kiln_model::{Rule, Severity};

use crate::project::{root, today};
use crate::{CheckFormat, Fail, EXIT_FINDINGS};

pub fn run(project: Option<&Path>, format: CheckFormat) -> Result<u8, Fail> {
    let format = match format {
        CheckFormat::Text => Format::Text,
        CheckFormat::Json => Format::Json,
        CheckFormat::Gitlab => Format::Gitlab,
        CheckFormat::Github => Format::Github,
    };
    let root = root(project)?;
    let source = DiskSource::new(&root);
    let findings = match Project::load(&source) {
        Ok(mut project) => {
            crate::project::apply_results(&mut project, &source)?;
            let mut findings = check(&project, &source, today());
            jira_findings(&project, &mut findings);
            findings
        }
        // A kiln.toml Kiln can't read is a finding like any other here,
        // so a CI report shows it on the file.
        Err(LoadError::Config(findings)) => findings,
        Err(LoadError::Read(e)) => {
            return Err(Fail::usage(format!("kiln.toml: can't read it: {e}")))
        }
    };
    print!("{}", render(&findings, format));
    eprintln!("kiln: {}", summary(&findings));
    Ok(if has_errors(&findings) {
        EXIT_FINDINGS
    } else {
        0
    })
}

/// K207, when `[check]` turns it on: the Jira mirror's tickets against
/// the items' statuses. Jira being unreachable or unconfigured is a note
/// on stderr, never a failed check: Kiln works without Jira.
// implements: REQ-006
fn jira_findings(project: &Project, findings: &mut Vec<kiln_model::Finding>) {
    let severity = project.config.severity(Rule::K207);
    if severity == Severity::Off || kiln_jira::keys(project).is_empty() {
        return;
    }
    match super::jira::mirror(project) {
        Ok(mirror) => {
            for mut f in kiln_jira::k207(project, &mirror) {
                f.severity = severity;
                findings.push(f);
            }
            findings.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
        }
        Err(e) => eprintln!("kiln: K207 not checked: {e}"),
    }
}
