//! `kiln new`: the next free ID, a file from the type's template (or the
//! body given), its links and tags, its path on stdout
//! (`kiln_index::create_item`, which the GUI shares). Everything an item
//! starts with goes in one command and one write, so an agent never edits
//! the front matter it just created (REQ-020).

use std::io::Read;
use std::path::Path;

use serde_json::json;

use super::{json_out, parse_id};
use crate::project::{edit_fail, load};
use crate::{Fail, TextOrJson};

/// What `kiln new` was given.
pub struct Args<'a> {
    pub type_name: &'a str,
    pub parent: Option<String>,
    pub title: Option<String>,
    /// `ROLE=ID`, as typed.
    pub links: &'a [String],
    pub tags: Vec<String>,
    /// A file, or `-` for stdin.
    pub body_file: Option<&'a Path>,
    pub open: bool,
    pub format: TextOrJson,
}

// implements: REQ-020
pub fn run(project: Option<&Path>, args: Args) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let parent = args.parent.as_deref().map(parse_id).transpose()?;
    let mut links = Vec::new();
    for link in args.links {
        let (role, target) = link
            .split_once('=')
            .ok_or_else(|| Fail::usage(format!("--link `{link}`: write ROLE=ID")))?;
        links.push((role.trim().to_owned(), parse_id(target.trim())?));
    }
    let body = match args.body_file {
        None => None,
        Some(path) if path == Path::new("-") => {
            let mut text = String::new();
            std::io::stdin()
                .read_to_string(&mut text)
                .map_err(|e| Fail::usage(format!("--body-file -: can't read stdin: {e}")))?;
            Some(text)
        }
        Some(path) => Some(std::fs::read_to_string(path).map_err(|e| {
            Fail::usage(format!(
                "--body-file {}: can't read it: {e}",
                path.display()
            ))
        })?),
    };
    let spec = kiln_index::NewItemSpec {
        type_name: args.type_name,
        title: args.title.as_deref(),
        parent: parent.as_ref(),
        links: &links,
        tags: &args.tags,
        body: body.as_deref(),
    };
    let new = kiln_index::create_item(&loaded.project, &loaded.source, &spec).map_err(edit_fail)?;
    match args.format {
        TextOrJson::Text => println!("{}", new.path),
        TextOrJson::Json => print!(
            "{}",
            json_out(&json!({ "id": new.id.to_string(), "path": new.path }))
        ),
    }
    if args.open {
        // The item exists whether or not this works.
        match kiln_fenix::open(&loaded.source.path(&new.path), new.title_line as usize) {
            Ok(how) => eprintln!("kiln: {} {}", new.id, how.describe()),
            Err(e) => eprintln!("kiln: created {} but couldn't open it: {e}", new.id),
        }
    }
    Ok(0)
}
