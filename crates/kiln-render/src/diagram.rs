//! Mermaid validation for K111, through `fenix-diagram` (the renderer
//! Fenix and Kiln's exports draw with, so a diagram that passes here
//! draws there). It lays the diagram out but rasterises nothing, so it
//! needs no fonts.
//!
//! Results are remembered per source for the life of the process: the
//! language server checks the whole project on every keystroke, and laying
//! out every diagram each time would spend its 50 ms budget on diagrams
//! nobody touched. The memory is bounded; past the bound it starts over.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

type Outcome = Option<(u32, u32, String)>;

/// Distinct diagram sources remembered before the memory starts over.
const REMEMBERED: usize = 4096;

/// `None` when `source` draws; else the 0-based line within the source,
/// the 0-based column, and the renderer's message.
pub fn diagram_error(source: &str) -> Outcome {
    static SEEN: OnceLock<Mutex<HashMap<String, Outcome>>> = OnceLock::new();
    let seen = SEEN.get_or_init(Default::default);
    if let Some(known) = seen.lock().ok().and_then(|m| m.get(source).cloned()) {
        return known;
    }
    let outcome = layout_error(source);
    if let Ok(mut m) = seen.lock() {
        if m.len() >= REMEMBERED {
            m.clear();
        }
        m.insert(source.to_owned(), outcome.clone());
    }
    outcome
}

fn layout_error(source: &str) -> Outcome {
    let themes = fenix_diagram::Themes::default();
    match fenix_diagram::render(source, &themes, None) {
        Ok(_) => None,
        Err(d) => Some((d.line as u32, d.col as u32, d.message)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn good_and_bad_diagrams() {
        assert_eq!(diagram_error("flowchart TD\n  a --> b\n"), None);
        let (line, _, message) =
            diagram_error("flowchart TD\n  a --> \n").expect("a dangling edge is an error");
        assert_eq!(line, 1, "{message}");
        assert!(diagram_error("").is_some());
    }
}
