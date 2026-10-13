//! Mermaid validation for K111, through `fenix-diagram` (the renderer
//! Fenix and Kiln's exports draw with, so a diagram that passes here
//! draws there). It lays the diagram out but rasterises nothing, so it
//! needs no fonts.

/// `None` when `source` draws; else the 0-based line within the source,
/// the 0-based column, and the renderer's message.
pub fn diagram_error(source: &str) -> Option<(u32, u32, String)> {
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
