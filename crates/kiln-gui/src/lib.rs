//! The GUI (docs/GUI.md): eleven pages onto the project (Tree, Trace,
//! Impact, Board, Roadmap, Decisions, Changes, Item, Check, Exports,
//! Comments), driven from the keyboard, opening items in Fenix, editing
//! front matter and running exports.
//!
//! Everything that can be tested without a display is in plain modules:
//! [`pages`] (each page as rows), [`app`] (keys in, a [`Screen`] and an
//! [`Effect`] out), [`prompt`], [`screen`], [`keys`], [`theme`] and
//! [`settings`]. The window, the GPU and the text only draw a `Screen`
//! and turn winit's events into key presses. Built on the same stack and versions as Fenix
//! and Flint (winit 0.30, wgpu 30, glyphon 0.12), so text and colours look
//! alike.
//!
//! It edits front matter, never prose (ADR-012): one-line values, links,
//! new items and renumbering, through the same `kiln-index` functions as
//! `kiln set` and friends ([`edit`]); bodies are edited in Fenix, and the
//! window follows the files by polling. What needs git, the export
//! pipeline or Confluence is the binary's, behind [`Host`], so the GUI
//! never handles a token.

pub mod app;
pub mod edit;
mod gpu;
pub mod host;
pub mod keys;
pub mod pages;
pub mod prompt;
mod rect;
mod render;
pub mod screen;
pub mod settings;
pub mod theme;
mod window;

pub use app::{App, Baseline, Effect};
pub use host::{Comment, CommentGroup, Done, ExportReport, Host, Job};
pub use screen::Screen;
pub use settings::{parse as parse_settings, Settings};
pub use window::{run, JiraLoad, Options};

#[cfg(test)]
mod tests;
