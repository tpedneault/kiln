//! `kiln lsp`: a language server over stdio for item files, which is how
//! Fenix (or any LSP editor) gets completion, hover, definition,
//! references, rename, diagnostics, code actions and formatting
//! (docs/INTEGRATION.md, "Editing items").
//!
//! Synchronous, on `lsp-server` and `lsp-types`, as rust-analyzer is: one
//! thread owning the index is simpler to reason about than an async
//! runtime, and re-reading only the changed file
//! (`kiln_index::Project::update_file`) keeps it within the 50 ms budget.
//! A second thread watches the disk for changes made outside the editor.
//!
//! It deliberately serves Markdown only; code files need Fenix G3. It
//! never writes a file itself: every change is a workspace edit the
//! editor applies (and can undo), computed by the same functions as the
//! command line (`set_field`, `add_link`, `plan_renumber`,
//! `plan_new_item`), so the editor and `kiln set` never disagree. What a
//! client can't apply (file creation and renames, in Fenix until G7) is
//! offered disabled, naming the command that does it.

mod features;
mod server;
mod text;
mod uri;
mod watch;
mod world;

pub use server::capabilities;

use kiln_model::Date;
use lsp_server::Connection;

/// Serves the language server protocol on stdin and stdout until the
/// client exits. `today` is the date the rules compare against (K201's
/// overdue items); the server asks for it at every check, so a session
/// open past midnight stays right.
pub fn serve_stdio(today: fn() -> Date) -> Result<(), String> {
    let (connection, io) = Connection::stdio();
    serve(connection, today)?;
    io.join().map_err(|e| format!("closing stdio: {e}"))
}

/// Serves one connection; tests use `Connection::memory`.
pub fn serve(connection: Connection, today: fn() -> Date) -> Result<(), String> {
    server::serve(connection, today)
}

#[cfg(test)]
mod tests;
