//! The vocabulary every other crate speaks: the `kiln.toml` schema (types,
//! statuses, link roles, paths, code-reference patterns), `ItemId`,
//! `Item`, `Ref`, `CodeRef`, `Finding` and the check rules, exactly as
//! docs/FORMAT.md defines them.
//!
//! Pure: no file system, no clock, no environment beyond what a caller
//! passes in (today's date arrives as a `Date`), so every type can be
//! built and checked in a unit test. It deliberately doesn't parse text
//! (`kiln-parse` turns bytes into these types, keeping spans), hold a
//! project in memory (`kiln-index`), or decide what is wrong with one
//! (`kiln-check`). FORMAT.md is the contract: change it and this crate
//! together, with `kiln-parse`.
//!
//! Kiln's own folders (settings, state, logs, `KILN_HOME`) join this crate
//! with the first thing that keeps state (the Confluence page cache, K3);
//! K1 keeps none.

mod config;
mod finding;
mod id;
mod item;

pub use config::{default_config, slug, CodeConfig, Config, LinkRole, TypeDef, CODE};
pub use finding::{Finding, Rule, Severity, Span};
pub use id::{is_prefix, IdError, ItemId};
pub use item::{CodeRef, Date, Item, LinkEntry, Mention, Ref, RelativeLink};
