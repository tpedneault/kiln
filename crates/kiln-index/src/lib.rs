//! A loaded project: walks the configured item and code paths (the
//! `ignore` crate, so `.gitignore` is honoured), parses files in parallel
//! (`rayon`), and builds the graph: forward and inverse links from items
//! and code locations, the parent tree, mentions. Answers the queries
//! everything above it asks: children, ancestors, transitive impact with
//! paths, coverage, milestone progress, the next free ID, and the edits a
//! renumbering needs.
//!
//! Pure given a file-source trait: the index reads only through
//! [`FileSource`], so tests feed it an in-memory project and the language
//! server (K4) will feed it unsaved buffers. [`DiskSource`] is the one
//! implementation that touches the disk. [`Project::update_file`]
//! re-reads one file and rebuilds the graph, which is how the language
//! server stays within its 50 ms budget.
//!
//! It deliberately doesn't judge the project (`kiln-check` turns it into
//! findings) or render it, and it never writes: a renumbering is a plan
//! the caller applies.

mod new_item;
mod project;
mod renumber;
mod source;

pub use new_item::{plan_new_item, toml_string, NewItem, NewItemSpec};
pub use project::{labelled, Impact, Inbound, LoadError, Progress, Project, Source, Step};
pub use renumber::{plan_renumber, FileEdits, RenumberPlan};
pub use source::{find_root, DiskSource, FileSource, Listing, MemorySource, Stamp};

#[cfg(test)]
mod tests;
