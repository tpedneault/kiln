//! A loaded project: walks the configured item and code paths (the
//! `ignore` crate, so `.gitignore` is honoured), parses files in parallel,
//! and builds the graph: forward and inverse links, the parent tree,
//! mentions. Answers the queries everything above it asks: children,
//! ancestors, transitive impact with paths, coverage, milestone progress.
//!
//! Pure given a file-source trait: the walk goes through that trait, so
//! tests feed it an in-memory project and the language server (K4) feeds
//! it unsaved buffers. That indirection is the price of re-indexing one
//! file incrementally within the LSP's 50 ms budget (DESIGN.md,
//! "Performance") instead of reloading the project per keystroke.
//!
//! It deliberately doesn't judge the project (`kiln-check` turns the graph
//! into findings) or render it (`kiln-render`), and it never writes.
//!
//! Arrives in K1; a shell until then.
