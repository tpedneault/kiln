//! `kiln new`: the next free ID, a file from the type's template, its
//! path on stdout.

use std::path::Path;

use super::parse_id;
use crate::project::{load, relative};
use crate::Fail;

pub fn run(
    project: Option<&Path>,
    type_name: &str,
    parent: Option<String>,
    title: Option<String>,
    open: bool,
) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let root = loaded.source.root();
    let parent = parent.as_deref().map(parse_id).transpose()?;
    let spec = kiln_index::NewItemSpec {
        type_name,
        title: title.as_deref(),
        parent: parent.as_ref(),
        links: &[],
    };
    let new =
        kiln_index::plan_new_item(&loaded.project, &loaded.source, &spec).map_err(Fail::usage)?;
    let path = root.join(&new.path);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Fail::usage(format!("{}: {e}", dir.display())))?;
    }
    fenix_storage::atomic_write_new(&path, |w| w.write_all(new.text.as_bytes()))
        .map_err(|e| Fail::usage(format!("{}: can't create it: {e}", relative(root, &path))))?;
    println!("{}", relative(root, &path));
    if open {
        // The item exists whether or not this works.
        match kiln_fenix::open(&path, new.title_line as usize) {
            Ok(how) => eprintln!("kiln: {} {}", new.id, how.describe()),
            Err(e) => eprintln!("kiln: created {} but couldn't open it: {e}", new.id),
        }
    }
    Ok(0)
}
