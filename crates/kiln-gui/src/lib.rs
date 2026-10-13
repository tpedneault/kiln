//! The GUI (docs/GUI.md): seven pages onto the project (Tree, Trace,
//! Impact, Board, Roadmap, Decisions, Changes), driven from the keyboard,
//! opening items in Fenix.
//!
//! Everything that can be tested without a display is in plain modules:
//! [`pages`] (each page as rows), [`app`] (keys in, a [`Screen`] and an
//! [`Effect`] out), [`screen`], [`keys`], [`theme`] and [`settings`]. The
//! window, the GPU and the text only draw a `Screen` and turn winit's
//! events into key presses. Built on the same stack and versions as Fenix
//! and Flint (winit 0.30, wgpu 30, glyphon 0.12), so text and colours look
//! alike.
//!
//! It deliberately edits nothing: the files are edited in Fenix (or with
//! `kiln set`), and the window follows them by polling.

pub mod app;
mod gpu;
pub mod keys;
pub mod pages;
mod rect;
mod render;
pub mod screen;
pub mod settings;
pub mod theme;
mod window;

pub use app::{App, Baseline, Effect};
pub use screen::Screen;
pub use settings::{parse as parse_settings, Settings};
pub use window::{run, JiraLoad, Options};

#[cfg(test)]
mod tests;
