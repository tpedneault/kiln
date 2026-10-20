//! `kiln renumber`: applies `kiln-index`'s plan (`apply_renumber`, which
//! the GUI shares). Every file is checked unchanged since the index read
//! it before any is written, so a refusal leaves the project as it was;
//! the item's file is renamed last.

use std::path::Path;

use super::parse_id;
use crate::project::{edit_fail, load};
use crate::Fail;

pub fn run(project: Option<&Path>, id: &str, to: &str, file: Option<&str>) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let from = parse_id(id)?;
    let to = parse_id(to)?;
    let done = kiln_index::apply_renumber(&loaded.project, &loaded.source, &from, &to, file)
        .map_err(edit_fail)?;
    println!("{}", done.summary);
    for line in &done.left_alone {
        eprintln!("kiln: left alone: {line}");
    }
    Ok(0)
}
