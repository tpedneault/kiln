//! `kiln lsp`: a language server over stdio for item files, which is how
//! Fenix (or any LSP editor) gets completion, hover, definition,
//! references, rename, diagnostics and code actions (docs/INTEGRATION.md,
//! "Editing items").
//!
//! Synchronous, on `lsp-server` and `lsp-types`, as rust-analyzer is: one
//! thread owning the index is simpler to reason about than an async
//! runtime, and incremental re-indexing of the changed file keeps it
//! within the 50 ms budget.
//!
//! It deliberately serves Markdown only; code files need Fenix G3. Its
//! edits go through the same comment-preserving front-matter writer as
//! the command line, so the editor and `kiln set` never disagree.
//!
//! Arrives in K4; a shell until then.
