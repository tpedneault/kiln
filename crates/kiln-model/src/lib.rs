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
//! Also the export targets and Confluence connection (K2), and Kiln's own
//! folders (settings, state, logs, `KILN_HOME`), whose rules are pure
//! here and resolved against the platform by the binary. And the presets
//! (K11): the document templates Kiln ships, as data.

mod config;
mod export;
mod finding;
mod folders;
mod id;
mod item;
mod preset;

pub use config::{default_config, slug, CodeConfig, Config, JiraSync, LinkRole, TypeDef, CODE};
pub use export::{
    ChildList, ConfluenceConfig, ConfluenceFlavour, ConfluenceTarget, Diagrams, DocxOptions,
    ExportFormat, ExportTarget, Flavour, HtmlOptions, ItemStyle, Layout, MarkdownOptions, OnDrift,
    QualifyTitles, REPORTS,
};
pub use finding::{Finding, Rule, Severity, Span};
pub use folders::{Folders, Platform};
pub use id::{is_prefix, IdError, ItemId};
pub use item::{CodeRef, Date, Item, LinkEntry, Mention, Ref, RelativeLink};
pub use preset::{preset, Preset, PRESETS};
