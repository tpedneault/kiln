//! One module per command, and the helpers they share.

pub mod auth;
pub mod check;
pub mod confluence;
pub mod diff;
pub mod edit;
pub mod export;
pub mod gui;
pub mod impact;
pub mod init;
pub mod jira;
pub mod list;
pub mod new;
pub mod preset;
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

pub use kiln_index::labelled;

/// Pretty JSON with a final newline.
pub fn json_out(value: &serde_json::Value) -> String {
    let mut s = serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".to_owned());
    s.push('\n');
    s
}
