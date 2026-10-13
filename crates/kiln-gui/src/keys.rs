//! winit's key events as Fenix's [`KeyPress`]es. Pure, so the mapping and
//! its awkward corners (AltGr, shifted characters) are tested without a
//! window. Shift isn't a modifier here, as in Fenix: a shifted letter
//! arrives as itself (`G`).

use fenix_keymap::{KeyPress, NamedKey as Named};
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// The presses one key event produces: none, one, or several characters
/// an input method committed at once.
pub fn presses(key: &Key, mods: ModifiersState) -> Vec<KeyPress> {
    let ctrl = mods.control_key();
    let alt = mods.alt_key();
    match key {
        Key::Named(named) => named_press(*named, ctrl, alt).into_iter().collect(),
        Key::Character(text) => {
            // AltGr arrives as Ctrl+Alt on Windows and types `@`, `{`, `€`
            // on many layouts: text, not a shortcut.
            if ctrl && !alt {
                let mut chars = text.chars();
                return match (chars.next(), chars.next()) {
                    (Some(c), None) if c.is_ascii_alphabetic() => {
                        vec![KeyPress::char(c.to_ascii_lowercase()).with_ctrl()]
                    }
                    _ => Vec::new(),
                };
            }
            if (alt && !ctrl) || mods.super_key() {
                return Vec::new();
            }
            text.chars()
                .filter(|c| !c.is_control())
                .map(KeyPress::char)
                .collect()
        }
        _ => Vec::new(),
    }
}

fn named_press(key: NamedKey, ctrl: bool, alt: bool) -> Option<KeyPress> {
    let named = |n| Some(KeyPress::named(n));
    match key {
        NamedKey::Escape => named(Named::Escape),
        NamedKey::Enter => named(Named::Enter),
        NamedKey::Tab => named(Named::Tab),
        NamedKey::Backspace => named(Named::Backspace),
        NamedKey::ArrowUp => named(Named::Up),
        NamedKey::ArrowDown => named(Named::Down),
        NamedKey::ArrowLeft => named(Named::Left),
        NamedKey::ArrowRight => named(Named::Right),
        NamedKey::Home => named(Named::Home),
        NamedKey::End => named(Named::End),
        NamedKey::PageUp => named(Named::PageUp),
        NamedKey::PageDown => named(Named::PageDown),
        NamedKey::Space if !ctrl && !alt => Some(KeyPress::char(' ')),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(s: &str, mods: ModifiersState) -> Vec<KeyPress> {
        presses(&Key::Character(s.into()), mods)
    }

    #[test]
    fn text_control_letters_and_altgr() {
        assert_eq!(ch("j", ModifiersState::empty()), [KeyPress::char('j')]);
        assert_eq!(ch("G", ModifiersState::SHIFT), [KeyPress::char('G')]);
        assert_eq!(
            ch("D", ModifiersState::CONTROL | ModifiersState::SHIFT),
            [KeyPress::char('d').with_ctrl()]
        );
        assert_eq!(
            ch("@", ModifiersState::CONTROL | ModifiersState::ALT),
            [KeyPress::char('@')]
        );
        assert!(ch("x", ModifiersState::ALT).is_empty());
        assert_eq!(
            presses(&Key::Named(NamedKey::Space), ModifiersState::empty()),
            [KeyPress::char(' ')]
        );
        assert_eq!(
            presses(&Key::Named(NamedKey::PageDown), ModifiersState::empty()),
            [KeyPress::named(Named::PageDown)]
        );
        assert!(presses(&Key::Named(NamedKey::F1), ModifiersState::empty()).is_empty());
    }
}
