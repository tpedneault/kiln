//! Text to model: splits an item file's `+++` front matter from its body,
//! parses the front matter and `kiln.toml` with `toml_edit`, finds
//! `[[ID]]` and bare-ID references in bodies (outside code, through
//! `pulldown-cmark`), and matches code-reference patterns in source text
//! (docs/FORMAT.md). Also the comment-preserving front-matter edits, and
//! adding a preset to `kiln.toml` the same way.
//!
//! Every value keeps its byte span. That costs memory and some ceremony,
//! and buys the two things Kiln is for at the point of an error: a
//! finding that names file, line and column, and edits (`kiln set`,
//! `link`, `renumber`) that touch only the text they are about.
//! `toml_edit` rather than `toml` for the same reason.
//!
//! Pure: it takes text and returns model values, findings or new text; it
//! never opens, walks or writes a file. It deliberately checks only what
//! one file can tell; anything needing the configuration's types or the
//! rest of the project is `kiln-check`. It doesn't parse any programming
//! language: code references are regex matches over whole text, so a
//! match in a string literal counts, which keeps the scanner
//! language-agnostic.

mod code;
mod config;
mod edit;
mod exports;
mod item;
mod preset;
mod text;
mod toml_walk;

pub use code::{compile_pattern, is_scannable, scan_code, CodePattern, MAX_CODE_FILE};
pub use config::{confined, is_word, parse_config, CONFIG_FILE, FORMAT};
pub use edit::{
    add_link, format_front_matter, remove_link, set_config_key, set_field, splice, EditError,
    FieldValue,
};
pub use item::{parse_item, relative_destination, sections, ItemOptions, Sections, MAX_TITLE};
pub use preset::{add_preset, PresetPlan};
pub use text::{denormalize, normalize, uses_crlf, LineIndex};
