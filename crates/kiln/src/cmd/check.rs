//! `kiln check`: the findings in the asked format on stdout, a summary on
//! stderr, exit 1 on any error.

use std::path::Path;

use kiln_check::{check, has_errors, render, summary, Format};
use kiln_index::{DiskSource, LoadError, Project};

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
        Ok(project) => check(&project, &source, today()),
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
