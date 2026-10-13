//! `settings.toml` in Kiln's settings folder (CONFIGURATION.md): the
//! GUI's theme, font and size. Not the project's business, so not in
//! `kiln.toml`. Anything Kiln doesn't know is reported in the footer and
//! otherwise ignored: a typo in a theme name shouldn't stop the window.

use crate::theme::{self, Palette};

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub palette: &'static Palette,
    /// A family to use instead of the monospace one Fenix would pick.
    pub font: Option<String>,
    /// In points.
    pub font_size: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            palette: &theme::ORBIT_DARK,
            font: None,
            font_size: 15.0,
        }
    }
}

/// Reads settings text; returns them and what was wrong with them, one
/// line each.
pub fn parse(text: &str) -> (Settings, Vec<String>) {
    let mut s = Settings::default();
    let mut problems = Vec::new();
    let doc: toml_edit::DocumentMut = match text.parse() {
        Ok(d) => d,
        Err(e) => {
            let first = e
                .to_string()
                .lines()
                .next()
                .unwrap_or("not TOML")
                .to_owned();
            return (s, vec![format!("settings.toml isn't valid TOML: {first}")]);
        }
    };
    for (key, item) in doc.iter() {
        match key {
            "theme" => match item.as_str().and_then(theme::by_name) {
                Some(p) => s.palette = p,
                None => problems.push(format!(
                    "settings.toml: theme {} isn't one of Fenix's ({})",
                    item.to_string().trim(),
                    theme::ALL
                        .iter()
                        .map(|p| p.name)
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            },
            "font" => match item.as_str() {
                Some(f) if !f.trim().is_empty() => s.font = Some(f.trim().to_owned()),
                _ => problems.push("settings.toml: font is a font family's name".into()),
            },
            "font_size" => {
                let size = item
                    .as_integer()
                    .map(|i| i as f64)
                    .or_else(|| item.as_float());
                match size {
                    Some(v) if (8.0..=40.0).contains(&v) => s.font_size = v as f32,
                    _ => problems.push("settings.toml: font_size is a number from 8 to 40".into()),
                }
            }
            other => problems.push(format!(
                "settings.toml: unknown key `{other}` (theme, font, font_size)"
            )),
        }
    }
    (s, problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_keys_and_complaints() {
        let (s, p) = parse("theme = \"gruvbox dark\"\nfont = \"Cascadia Mono\"\nfont_size = 13\n");
        assert_eq!(
            (s.palette.name, s.font.as_deref(), s.font_size),
            ("Gruvbox Dark", Some("Cascadia Mono"), 13.0)
        );
        assert!(p.is_empty());
        let (s, p) = parse("theme = \"Pink\"\nfont_size = 99\ncolour = 1\n");
        assert_eq!(s, Settings::default());
        assert_eq!(p.len(), 3, "{p:?}");
        assert!(
            p[0].starts_with("settings.toml: theme \"Pink\" isn't one of Fenix's (Orbit Dark,"),
            "{}",
            p[0]
        );
        assert_eq!(parse("= nope").1.len(), 1);
    }
}
