//! The colours Kiln draws with: Fenix's twelve themes, as Flint has them.
//!
//! Fenix's palettes live in `fenix-gui`, which Kiln mustn't depend on
//! (DESIGN.md, "Dependencies on Fenix and Flint crates"), so the handful
//! of colours each page needs are copied, from Flint's `flint-gui` theme
//! module at Flint `eb23547` (itself copied from Fenix's `theme.rs`), and
//! the four Catppuccin flavours from Fenix's `theme.rs` at Fenix `d821a0b`
//! (`accent` is Fenix's `caret_text` for those: Latte's caret is a light
//! rosewater that only reads as a block, and Kiln draws `accent` as text). Flint
//! records the shared `fenix-theme` crate that would replace both copies
//! as its F9. The same hex value looks the same in all three: colours are
//! plain `0..1` RGBA written to a non-sRGB surface.

use crate::screen::Role;

/// What each part of the window is painted with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Fenix's name for the theme.
    pub name: &'static str,
    /// The theme's own font, which `font` in settings overrides.
    pub font_family: Option<&'static str>,
    /// The window background (Fenix's `bg`).
    pub background: [f32; 4],
    /// The top bar and footer (Fenix's `bg_modeline`).
    pub surface: [f32; 4],
    /// Body text (Fenix's `fg`).
    pub foreground: [f32; 4],
    /// Secondary text (Fenix's `gutter_fg`).
    pub muted: [f32; 4],
    /// IDs and the current page (Fenix's `caret`).
    pub accent: [f32; 4],
    /// The cursor's row (Fenix's `selection`).
    pub selection: [f32; 4],
    /// Heading bands (Fenix's `hl_line`).
    pub border: [f32; 4],
    /// Gaps and errors (Fenix's `git_conflicted`).
    pub error: [f32; 4],
    /// Done and covered (Fenix's `mode_insert`).
    pub mode_insert: [f32; 4],
    /// Changed and in progress (Fenix's `mode_normal`).
    pub mode_normal: [f32; 4],
    /// Text on a badge (Fenix's `mode_text_dark`).
    pub badge_text: [f32; 4],
    /// Headings (Fenix's `syntax_function`).
    pub chip: [f32; 4],
    /// A done status's pill (Fenix's `git_staged`; Catppuccin's green).
    pub done: [f32; 4],
    /// An in-progress status's pill (Fenix's `mode_picker`; Catppuccin's
    /// mauve): apart from the IDs' colour, which is often the same as
    /// `mode_normal`.
    pub active: [f32; 4],
}

const fn rgba(hex: u32) -> [f32; 4] {
    rgba_alpha(hex, 1.0)
}

const fn rgba_alpha(hex: u32, alpha: f32) -> [f32; 4] {
    let r = ((hex >> 16) & 0xff) as f32 / 255.0;
    let g = ((hex >> 8) & 0xff) as f32 / 255.0;
    let b = (hex & 0xff) as f32 / 255.0;
    [r, g, b, alpha]
}

/// Orbit Dark, Fenix's default theme.
pub const ORBIT_DARK: Palette = Palette {
    name: "Orbit Dark",
    font_family: None,
    background: rgba(0x1a1b26),
    surface: rgba(0x24283b),
    foreground: rgba(0xc0caf5),
    muted: rgba(0x929bb9),
    accent: rgba(0xe0af68),
    selection: rgba_alpha(0x7aa2f7, 0.12),
    border: rgba(0x222534),
    error: rgba(0xf7768e),
    mode_insert: rgba(0x7dcfff),
    mode_normal: rgba(0xe0af68),
    badge_text: rgba(0x1a1b26),
    chip: rgba(0x7aa2f7),
    done: rgba(0x9ece6a),
    active: rgba(0xbb9af7),
};

/// TempleOS: white, with VGA blue.
pub const TEMPLEOS: Palette = Palette {
    name: "TempleOS",
    font_family: None,
    background: rgba(0xffffff),
    surface: rgba(0x0000aa),
    foreground: rgba(0x000000),
    muted: rgba(0x555555),
    accent: rgba(0x5555ff),
    selection: rgba_alpha(0x0000aa, 0.45),
    border: rgba_alpha(0x5555ff, 0.25),
    error: rgba(0xaa0000),
    mode_insert: rgba(0x55ffff),
    mode_normal: rgba(0x55ff55),
    badge_text: rgba(0x000000),
    chip: rgba(0xaa00aa),
    done: rgba(0x00aa00),
    active: rgba(0xff55ff),
};

/// Gruvbox Dark.
pub const GRUVBOX_DARK: Palette = Palette {
    name: "Gruvbox Dark",
    font_family: None,
    background: rgba(0x1d2021),
    surface: rgba(0x282828),
    foreground: rgba(0xebdbb2),
    muted: rgba(0xa89984),
    accent: rgba(0xfabd2f),
    selection: rgba_alpha(0x83a598, 0.12),
    border: rgba(0x282828),
    error: rgba(0xfb7965),
    mode_insert: rgba(0x83a598),
    mode_normal: rgba(0xfabd2f),
    badge_text: rgba(0x1d2021),
    chip: rgba(0x83a598),
    done: rgba(0xb8bb26),
    active: rgba(0xd3869b),
};

/// Nord.
pub const NORD: Palette = Palette {
    name: "Nord",
    font_family: None,
    background: rgba(0x2e3440),
    surface: rgba(0x343b49),
    foreground: rgba(0xd8dee9),
    muted: rgba(0x9aa6bc),
    accent: rgba(0x88c0d0),
    selection: rgba_alpha(0x91b1d1, 0.12),
    border: rgba(0x343b49),
    error: rgba(0xdf929c),
    mode_insert: rgba(0x88c0d0),
    mode_normal: rgba(0xa3be8c),
    badge_text: rgba(0x202530),
    chip: rgba(0x88c0d0),
    done: rgba(0xa3be8c),
    active: rgba(0xc49ebd),
};

/// Dracula.
pub const DRACULA: Palette = Palette {
    name: "Dracula",
    font_family: None,
    background: rgba(0x282a36),
    surface: rgba(0x303341),
    foreground: rgba(0xf8f8f2),
    muted: rgba(0x9badd4),
    accent: rgba(0xff79c6),
    selection: rgba_alpha(0xbd93f9, 0.12),
    border: rgba(0x303341),
    error: rgba(0xff7979),
    mode_insert: rgba(0x8be9fd),
    mode_normal: rgba(0x50fa7b),
    badge_text: rgba(0x282a36),
    chip: rgba(0x50fa7b),
    done: rgba(0x50fa7b),
    active: rgba(0xff79c6),
};

/// Solarized Dark.
pub const SOLARIZED_DARK: Palette = Palette {
    name: "Solarized Dark",
    font_family: None,
    background: rgba(0x002b36),
    surface: rgba(0x073642),
    foreground: rgba(0xb5c3be),
    muted: rgba(0x92a4a5),
    accent: rgba(0xd8ad44),
    selection: rgba_alpha(0x55a8df, 0.12),
    border: rgba(0x073642),
    error: rgba(0xed7770),
    mode_insert: rgba(0x52b7ad),
    mode_normal: rgba(0xd8ad44),
    badge_text: rgba(0x00212b),
    chip: rgba(0x55a8df),
    done: rgba(0xa6b64c),
    active: rgba(0xdf82b1),
};

/// One Dark.
pub const ONE_DARK: Palette = Palette {
    name: "One Dark",
    font_family: None,
    background: rgba(0x282c34),
    surface: rgba(0x21252b),
    foreground: rgba(0xc1c8d4),
    muted: rgba(0x9aa2b1),
    accent: rgba(0xe5c07b),
    selection: rgba_alpha(0x61afef, 0.12),
    border: rgba(0x30353f),
    error: rgba(0xed929b),
    mode_insert: rgba(0x56b6c2),
    mode_normal: rgba(0x98c379),
    badge_text: rgba(0x21252b),
    chip: rgba(0x61afef),
    done: rgba(0x98c379),
    active: rgba(0x56b6c2),
};

/// Visual Studio Dark, which brings its own font.
pub const VISUAL_STUDIO_DARK: Palette = Palette {
    name: "Visual Studio Dark",
    font_family: Some("Consolas"),
    background: rgba(0x1e1e1e),
    surface: rgba(0x2d2d30),
    foreground: rgba(0xd4d4d4),
    muted: rgba(0x9c9c9c),
    accent: rgba(0x4fc1ff),
    selection: rgba_alpha(0x264f78, 0.55),
    border: rgba(0x2a2d2e),
    error: rgba(0xf46a6a),
    mode_insert: rgba(0x4ec9b0),
    mode_normal: rgba(0x569cd6),
    badge_text: rgba(0x1e1e1e),
    chip: rgba(0xdcdcaa),
    done: rgba(0x89d185),
    active: rgba(0xc586c0),
};

/// Catppuccin Latte. Its accents darkened as Fenix's are, so they read on the light page.
pub const CATPPUCCIN_LATTE: Palette = Palette {
    name: "Catppuccin Latte",
    font_family: None,
    background: rgba(0xeff1f5),
    surface: rgba(0xe6e9ef),
    foreground: rgba(0x4c4f69),
    muted: rgba(0x5e606c),
    accent: rgba(0x865449),
    selection: rgba_alpha(0x7c7f93, 0.25),
    border: rgba(0xd9dce4),
    error: rgba(0xc10e34),
    mode_insert: rgba(0x327d22),
    mode_normal: rgba(0x1d63ee),
    badge_text: rgba(0xeff1f5),
    chip: rgba(0x1a58d3),
    done: rgba(0x40a02b),
    active: rgba(0x8839ef),
};

/// Catppuccin Frappé.
pub const CATPPUCCIN_FRAPPE: Palette = Palette {
    name: "Catppuccin Frappé",
    font_family: None,
    background: rgba(0x303446),
    surface: rgba(0x292c3c),
    foreground: rgba(0xc6d0f5),
    muted: rgba(0xa6abc0),
    accent: rgba(0xf2d5cf),
    selection: rgba_alpha(0x949cbb, 0.25),
    border: rgba(0x3b3f52),
    error: rgba(0xea9395),
    mode_insert: rgba(0xa6d189),
    mode_normal: rgba(0x8caaee),
    badge_text: rgba(0x232634),
    chip: rgba(0x8caaee),
    done: rgba(0xa6d189),
    active: rgba(0xca9ee6),
};

/// Catppuccin Macchiato.
pub const CATPPUCCIN_MACCHIATO: Palette = Palette {
    name: "Catppuccin Macchiato",
    font_family: None,
    background: rgba(0x24273a),
    surface: rgba(0x1e2030),
    foreground: rgba(0xcad3f5),
    muted: rgba(0x979db3),
    accent: rgba(0xf4dbd6),
    selection: rgba_alpha(0x939ab7, 0.25),
    border: rgba(0x303347),
    error: rgba(0xed8796),
    mode_insert: rgba(0xa6da95),
    mode_normal: rgba(0x8aadf4),
    badge_text: rgba(0x181926),
    chip: rgba(0x8aadf4),
    done: rgba(0xa6da95),
    active: rgba(0xc6a0f6),
};

/// Catppuccin Mocha.
pub const CATPPUCCIN_MOCHA: Palette = Palette {
    name: "Catppuccin Mocha",
    font_family: None,
    background: rgba(0x1e1e2e),
    surface: rgba(0x181825),
    foreground: rgba(0xcdd6f4),
    muted: rgba(0x9195aa),
    accent: rgba(0xf5e0dc),
    selection: rgba_alpha(0x9399b2, 0.25),
    border: rgba(0x2a2b3c),
    error: rgba(0xf38ba8),
    mode_insert: rgba(0xa6e3a1),
    mode_normal: rgba(0x89b4fa),
    badge_text: rgba(0x11111b),
    chip: rgba(0x89b4fa),
    done: rgba(0xa6e3a1),
    active: rgba(0xcba6f7),
};
/// Every theme, in Fenix's `theme::ALL` order.
pub const ALL: [&Palette; 12] = [
    &ORBIT_DARK,
    &TEMPLEOS,
    &GRUVBOX_DARK,
    &NORD,
    &DRACULA,
    &SOLARIZED_DARK,
    &ONE_DARK,
    &VISUAL_STUDIO_DARK,
    &CATPPUCCIN_LATTE,
    &CATPPUCCIN_FRAPPE,
    &CATPPUCCIN_MACCHIATO,
    &CATPPUCCIN_MOCHA,
];

/// The theme called `name`, in any case, as Fenix looks themes up.
pub fn by_name(name: &str) -> Option<&'static Palette> {
    ALL.iter()
        .find(|p| p.name.eq_ignore_ascii_case(name.trim()))
        .copied()
}

impl Palette {
    /// The colour of a span with `role`.
    pub fn of(&self, role: Role) -> [f32; 4] {
        match role {
            Role::Normal => self.foreground,
            Role::Muted => self.muted,
            Role::Accent => self.accent,
            Role::Heading => self.chip,
            Role::Error => self.error,
            Role::Good => self.mode_insert,
            Role::Warn => self.mode_normal,
            Role::Done => self.done,
            Role::Active => self.active,
        }
    }
}

/// Converts a palette colour for glyphon.
pub fn text_color(c: [f32; 4]) -> glyphon::Color {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    glyphon::Color::rgba(byte(c[0]), byte(c[1]), byte(c[2]), byte(c[3]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twelve_themes_by_any_case() {
        assert_eq!(ALL.len(), 12);
        assert_eq!(by_name(" nord ").map(|p| p.name), Some("Nord"));
        assert!(by_name("Solarized Light").is_none());
        let c = text_color(ORBIT_DARK.foreground);
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (0xc0, 0xca, 0xf5, 0xff));
    }
}
