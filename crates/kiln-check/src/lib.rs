//! The rules of docs/FORMAT.md, "Check rules": each with a code (`K102`),
//! a default severity overridable in `kiln.toml`'s `[check]`, and a span.
//! Writes findings as text, JSON, GitLab Code Quality and GitHub workflow
//! annotations, so the same run fails a merge request and shows each
//! finding on the line that caused it.
//!
//! Pure: it reads an index and returns findings; the binary decides the
//! exit code. One fixture project per rule keeps each rule's test
//! readable on its own, at the cost of many small fixtures.
//!
//! It deliberately doesn't fix anything. Every fix (`kiln renumber`, LSP
//! code actions) is a deliberate edit by its own command.
//!
//! Arrives in K1; a shell until then.
