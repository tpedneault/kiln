//! What a frame shows, as data: lines of styled spans for the top bar,
//! the page and the footer, and the which-key popup. The application
//! builds it; the renderer draws it and knows nothing about pages. Tests
//! compare screens as text.

/// What a span means, which the theme turns into a colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Normal,
    /// Secondary text: types, paths, counts.
    Muted,
    /// IDs, the current page, keys in the popup.
    Accent,
    /// A heading row.
    Heading,
    /// Gaps, errors.
    Error,
    /// Done, covered, added.
    Good,
    /// Changed, in progress.
    Warn,
    /// A done status.
    Done,
    /// A status between the first and done (approved, doing, active).
    Active,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub role: Role,
    /// Drawn on a rounded band of its own colour (a status), so it stands
    /// out from the text around it.
    pub pill: bool,
}

/// One line of spans.
pub type Line = Vec<Span>;

pub fn span(text: impl Into<String>, role: Role) -> Span {
    Span {
        text: text.into(),
        role,
        pill: false,
    }
}

/// A span drawn as a pill: ` text ` on a rounded band of its role's
/// colour. The spaces are part of the text, so columns still line up and
/// tests read it as text.
pub fn pill(text: impl Into<String>, role: Role) -> Span {
    Span {
        text: format!(" {} ", text.into()),
        role,
        pill: true,
    }
}

/// The line as plain text, for tests and filtering.
pub fn plain(line: &[Span]) -> String {
    line.iter().map(|s| s.text.as_str()).collect()
}

/// A whole frame.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Screen {
    /// The page names, the current one marked, and the project.
    pub top: Line,
    /// The rows that fit, from `first`.
    pub rows: Vec<Line>,
    /// Which of `rows` the cursor is on.
    pub cursor: Option<usize>,
    /// Which of `rows` are headings (drawn with a band behind them).
    pub headings: Vec<usize>,
    /// The filter, a message, or the keys that work here.
    pub footer: Line,
    /// The footer is an error message.
    pub footer_error: bool,
    /// The which-key popup: one line per key.
    pub popup: Vec<Line>,
}

impl Screen {
    /// The whole screen as text, one line per line, the cursor's row
    /// marked with `>`: what tests compare.
    pub fn dump(&self) -> String {
        let mut out = format!("{}\n", plain(&self.top));
        for (i, row) in self.rows.iter().enumerate() {
            let mark = if self.cursor == Some(i) { '>' } else { ' ' };
            out.push_str(&format!("{mark}{}\n", plain(row).trim_end()));
        }
        out.push_str(&format!("-- {}\n", plain(&self.footer)));
        for line in &self.popup {
            out.push_str(&format!("| {}\n", plain(line)));
        }
        out
    }
}
