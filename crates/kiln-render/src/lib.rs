//! Markdown bodies to a small intermediate document tree
//! (`pulldown-cmark`, with GitHub tables, task lists and strikethrough),
//! where item references, callouts, tables, code blocks and Mermaid
//! diagrams are distinct nodes. The report builders (traceability,
//! findings, roadmap, risks, changes) produce the same tree.
//!
//! One tree for every output is the point: each back end (Confluence
//! storage, Markdown, HTML, later Word) renders nodes it understands
//! instead of re-parsing Markdown its own way, so a construct is handled
//! once and looks the same everywhere. Raw HTML in a body becomes a text
//! node; it is never passed through (DESIGN.md, "Security").
//!
//! Pure and deterministic: no clock, no environment, sorted wherever there
//! is no inherent order, so golden files change only deliberately. It
//! deliberately doesn't assemble documents, split them into pages or
//! rasterise diagrams; that is `kiln-export`.
//!
//! Arrives in K2; a shell until then.
