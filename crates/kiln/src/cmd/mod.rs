//! One module per command, and the helpers they share.

pub mod agents;
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
pub mod plan;
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

/// The shape of the JSON Kiln prints. Raised when an object's shape
/// changes incompatibly (a key removed, renamed or retyped); adding keys,
/// or fields to an array's elements, keeps it. 1 is the shape K15 set.
pub const KILN_JSON: u32 = 1;

/// Pretty JSON with a final newline. A top-level object carries
/// `kiln_json` ([`KILN_JSON`]) so a script can tell which shape it reads
/// (REQ-022); an array can't without wrapping it, which would itself
/// break readers, so arrays only ever gain fields.
pub fn json_out(value: &serde_json::Value) -> String {
    let mut value = value.clone();
    if let serde_json::Value::Object(map) = &mut value {
        map.insert("kiln_json".to_owned(), serde_json::json!(KILN_JSON));
    }
    let mut s = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "null".to_owned());
    s.push('\n');
    s
}
