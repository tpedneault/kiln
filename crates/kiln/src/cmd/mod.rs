//! One module per command, and the helpers they share.

pub mod auth;
pub mod check;
pub mod edit;
pub mod export;
pub mod impact;
pub mod init;
pub mod list;
pub mod new;
pub mod renumber;
pub mod show;
pub mod trace;

use kiln_index::Project;
use kiln_model::{Item, ItemId};

use crate::Fail;

/// An ID from the command line.
pub fn parse_id(arg: &str) -> Result<ItemId, Fail> {
    arg.parse()
        .map_err(|e| Fail::usage(format!("`{arg}`: {e}")))
}

/// The item with `id`, which must exist.
pub fn existing<'p>(project: &'p Project, id: &ItemId) -> Result<&'p Item, Fail> {
    project
        .item(id)
        .ok_or_else(|| Fail::usage(format!("{id} doesn't exist")))
}

/// The item with `id`, which must exist once: edits refuse a duplicated
/// ID, since they couldn't say which file they changed.
pub fn unique<'p>(project: &'p Project, id: &ItemId) -> Result<&'p Item, Fail> {
    let item = existing(project, id)?;
    let copies = project.indices(id);
    if copies.len() > 1 {
        let paths: Vec<&str> = copies
            .iter()
            .map(|&i| project.items[i].path.as_str())
            .collect();
        return Err(Fail::usage(format!(
            "{id} is in {} files ({}); `kiln renumber {id} --to {} --file …` first",
            paths.len(),
            paths.join(", "),
            project.next_id(id.prefix())
        )));
    }
    Ok(item)
}

pub use kiln_index::labelled;

/// Pretty JSON with a final newline.
pub fn json_out(value: &serde_json::Value) -> String {
    let mut s = serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".to_owned());
    s.push('\n');
    s
}
