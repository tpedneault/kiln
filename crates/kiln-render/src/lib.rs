//! Markdown bodies to a small intermediate document tree
//! (`pulldown-cmark`, with GitHub tables, task lists, strikethrough and
//! callouts), where item references, callouts, tables, code blocks and
//! Mermaid diagrams are distinct nodes. Templates parse to the same tree
//! with their `<!-- kiln:… -->` directives as nodes, and the report
//! builders (traceability, findings, roadmap, risks) produce it too.
//!
//! One tree for every output is the point: each back end (Confluence
//! storage, Markdown, HTML, later Word) renders nodes it understands
//! instead of re-parsing Markdown its own way, so a construct is handled
//! once and looks the same everywhere. Raw HTML in a body becomes a text
//! node; it is never passed through (DESIGN.md, "Security").
//!
//! Pure and deterministic: no clock, no environment, sorted wherever there
//! is no inherent order, so golden files change only deliberately. It
//! deliberately doesn't assemble documents, split them into pages, resolve
//! references to pages or rasterise diagrams; that is `kiln-export`. It
//! does parse Mermaid (through `fenix-diagram`, drawing nothing) so
//! `kiln check` can report a broken diagram on its line (K111).

mod body;
mod diagram;
mod doc;
mod reports;
mod template;

pub use body::{render_markdown, resolve, split_references, BodyContext};
pub use diagram::diagram_error;
pub use doc::{
    plain, Align, Block, Callout, Directive, Inline, ListItem, PropEntry, Props, Table, Target,
    Tone,
};
pub use reports::{report, status_tone, ReportOptions, Since};
pub use template::{template_problems, TemplateProblem};

#[cfg(test)]
mod tests;
