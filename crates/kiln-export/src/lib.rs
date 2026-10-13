//! Export targets (docs/EXPORT.md): selection, templates and their
//! `<!-- kiln:… -->` directives, assembly into one numbered document with
//! an anchor per item, the three layouts (`document`, `sections`, `tree`)
//! as splits of that one document with links resolved across pages,
//! diagram rasterising through `fenix-diagram` and its cache, and the
//! Markdown and HTML back ends with their file destination.
//!
//! Every target runs the same pipeline (select, assemble, split, back end,
//! destination), so a layout or a format is added once and works with
//! every other choice. `document` is built first and is the default; the
//! other layouts are splits of it, never separate code paths.
//!
//! Pure except for writing files, which happens only when bytes change
//! and atomically. It deliberately knows nothing about Confluence: that
//! back end and its destination live in `kiln-confluence`, which takes the
//! split pages from here.
//!
//! Arrives in K2; a shell until then.
