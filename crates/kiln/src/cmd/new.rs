//! `kiln new`: the next free ID, a file from the type's template, its
//! path on stdout (`kiln_index::create_item`, which the GUI shares).

use std::path::Path;

use super::parse_id;
use crate::project::{edit_fail, load};
use crate::Fail;

pub fn run(
    project: Option<&Path>,
    type_name: &str,
    parent: Option<String>,
    title: Option<String>,
    open: bool,
) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let parent = parent.as_deref().map(parse_id).transpose()?;
    let spec = kiln_index::NewItemSpec {
        type_name,
        title: title.as_deref(),
        parent: parent.as_ref(),
        links: &[],
    };
    let new = kiln_index::create_item(&loaded.project, &loaded.source, &spec).map_err(edit_fail)?;
    println!("{}", new.path);
    if open {
        // The item exists whether or not this works.
        match kiln_fenix::open(&loaded.source.path(&new.path), new.title_line as usize) {
            Ok(how) => eprintln!("kiln: {} {}", new.id, how.describe()),
            Err(e) => eprintln!("kiln: created {} but couldn't open it: {e}", new.id),
        }
    }
    Ok(0)
}
