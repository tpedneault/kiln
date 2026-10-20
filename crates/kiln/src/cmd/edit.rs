//! `kiln set`, `link` and `unlink`: comment-preserving front-matter edits,
//! validated against the configuration before anything is written
//! (`kiln_index::apply_set` and `apply_link`, which the GUI shares).

use std::path::Path;

use super::parse_id;
use crate::project::{edit_fail, load};
use crate::Fail;

pub fn set(project: Option<&Path>, id: &str, assignments: &[String]) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let id = parse_id(id)?;
    let mut pairs = Vec::new();
    for assignment in assignments {
        let (key, value) = assignment.split_once('=').ok_or_else(|| {
            Fail::usage(format!(
                "`{assignment}`: write KEY=VALUE ({})",
                kiln_index::SETTABLE
            ))
        })?;
        pairs.push((key.trim().to_owned(), value.trim().to_owned()));
    }
    let changes =
        kiln_index::apply_set(&loaded.project, &loaded.source, &id, &pairs).map_err(edit_fail)?;
    for change in changes {
        println!("{}", change.line());
    }
    Ok(0)
}

pub fn link(
    project: Option<&Path>,
    id: &str,
    role: &str,
    target: &str,
    add: bool,
) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let id = parse_id(id)?;
    let target = parse_id(target)?;
    let line = kiln_index::apply_link(&loaded.project, &loaded.source, &id, role, &target, add)
        .map_err(edit_fail)?;
    println!("{line}");
    Ok(0)
}
