//! Text to model: splits an item file's `+++` front matter from its body,
//! parses the front matter and `kiln.toml` with `toml_edit`, finds
//! `[[ID]]` and bare-ID references in bodies, and matches code-reference
//! patterns in source lines (docs/FORMAT.md).
//!
//! Every value keeps its byte span. That costs memory and some ceremony,
//! and buys the two things Kiln is for at the point of an error: a
//! finding that names file, line and column, and comment-preserving
//! front-matter edits (`kiln set`, `link`, `renumber`, LSP code actions)
//! that touch only the keys they are about. `toml_edit` rather than `toml`
//! for the same reason.
//!
//! Pure: it takes bytes and returns model values or spans; it never opens
//! a file, walks a directory or writes one. It deliberately doesn't parse
//! Markdown structure (that is `kiln-render`'s) or any programming
//! language: code references are matched on whole lines, so a match in a
//! string literal counts, which keeps the scanner language-agnostic.
//!
//! Arrives in K1; a shell until then.
