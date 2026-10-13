//! The vocabulary every other crate speaks: the `kiln.toml` schema (types,
//! statuses, link roles, paths, code-reference patterns, export targets),
//! `ItemId`, `Item`, `Link` and `Finding`, exactly as docs/FORMAT.md
//! defines them. Also Kiln's own folders (settings, state, logs,
//! `KILN_HOME`), mirroring `fenix_storage::paths`' rules because that
//! module hard-codes `fenix` (DESIGN.md, "Kiln's own folders").
//!
//! Pure: no file system, no clock, no environment beyond what a caller
//! passes in, so every type can be built and checked in a unit test.
//! It deliberately doesn't parse text (`kiln-parse` turns bytes into these
//! types, keeping spans), hold a project in memory (`kiln-index`), or
//! decide what is wrong with one (`kiln-check`). FORMAT.md is the
//! contract: change it and this crate together, with `kiln-parse`.
//!
//! Arrives in K1; a shell until then.
