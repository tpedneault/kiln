//! The footer's prompt: how the GUI asks for the one-line values its
//! edits need (GUI.md, "Editing"). A pick narrows a list as you type; a
//! text prompt takes one line; a confirm takes `y` or `n`. It holds no
//! knowledge of what the answer is for: the application keeps that in
//! [`Then`] and checks the answer when it arrives, so an invalid value
//! keeps the prompt open with the reason under it.

use fenix_keymap::{KeyCode, KeyPress, NamedKey};
use kiln_model::ItemId;

/// What the answer is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Then {
    /// `kiln set ID KEY=answer`.
    Set {
        id: ItemId,
        key: String,
    },
    /// A role picked; the target comes next.
    LinkRole {
        id: ItemId,
    },
    LinkTarget {
        id: ItemId,
        role: String,
    },
    /// A link picked (`role TARGET`); a confirm comes next.
    Unlink {
        id: ItemId,
    },
    UnlinkConfirm {
        id: ItemId,
        role: String,
        target: ItemId,
    },
    /// A type picked; the title comes next.
    NewType {
        parent: Option<ItemId>,
    },
    NewTitle {
        type_name: String,
        parent: Option<ItemId>,
    },
    /// The new ID typed; a confirm comes next.
    Renumber {
        from: ItemId,
        file: Option<String>,
    },
    RenumberConfirm {
        from: ItemId,
        to: ItemId,
        file: Option<String>,
    },
    Publish {
        target: String,
        shown: String,
    },
    Baseline,
    /// Home: a project's folder, typed.
    AddProject,
    /// Home: taking this project off the list, confirmed.
    RemoveProject {
        root: std::path::PathBuf,
    },
}

/// One entry of a pick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// What the answer is.
    pub value: String,
    /// What is shown and matched.
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// `free`: an answer that matches no choice is taken as typed.
    Pick {
        choices: Vec<Choice>,
        free: bool,
    },
    Text,
    Confirm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    /// `Status of REQ-014`.
    pub label: String,
    pub input: String,
    pub kind: Kind,
    pub then: Then,
    /// Why the last answer wasn't taken.
    pub error: Option<String>,
    /// The highlighted choice, among those that match.
    pub selected: usize,
}

/// What a key did to the prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Stay,
    Cancel,
    Answer(String),
}

/// A pick shows at most this many choices.
pub const SHOWN: usize = 12;

impl Prompt {
    pub fn pick(label: impl Into<String>, choices: Vec<Choice>, then: Then) -> Prompt {
        Prompt::new(
            label,
            Kind::Pick {
                choices,
                free: false,
            },
            then,
        )
    }

    pub fn text(label: impl Into<String>, input: impl Into<String>, then: Then) -> Prompt {
        let mut p = Prompt::new(label, Kind::Text, then);
        p.input = input.into();
        p
    }

    pub fn confirm(label: impl Into<String>, then: Then) -> Prompt {
        Prompt::new(label, Kind::Confirm, then)
    }

    fn new(label: impl Into<String>, kind: Kind, then: Then) -> Prompt {
        Prompt {
            label: label.into(),
            input: String::new(),
            kind,
            then,
            error: None,
            selected: 0,
        }
    }

    /// Lets an answer outside the choices through (a revision, say).
    pub fn free(mut self) -> Prompt {
        if let Kind::Pick { free, .. } = &mut self.kind {
            *free = true;
        }
        self
    }

    /// The choices that match what was typed, in their order.
    pub fn matches(&self) -> Vec<&Choice> {
        let Kind::Pick { choices, .. } = &self.kind else {
            return Vec::new();
        };
        let needle = self.input.trim().to_lowercase();
        choices
            .iter()
            .filter(|c| needle.is_empty() || c.label.to_lowercase().contains(&needle))
            .collect()
    }

    pub fn key(&mut self, k: KeyPress) -> Outcome {
        let ctrl = k.mods.ctrl;
        if let Kind::Confirm = self.kind {
            return match k.code {
                KeyCode::Char('y' | 'Y') if !ctrl => Outcome::Answer("y".into()),
                KeyCode::Char('n' | 'N') | KeyCode::Named(NamedKey::Escape) => Outcome::Cancel,
                KeyCode::Char('q') if ctrl => Outcome::Cancel,
                _ => Outcome::Stay,
            };
        }
        let count = self.matches().len();
        match k.code {
            KeyCode::Named(NamedKey::Escape) => return Outcome::Cancel,
            KeyCode::Char('q') if ctrl => return Outcome::Cancel,
            KeyCode::Named(NamedKey::Enter) => return self.answer(),
            KeyCode::Named(NamedKey::Down | NamedKey::Tab) => {
                self.selected = (self.selected + 1).min(count.saturating_sub(1));
            }
            KeyCode::Char('n' | 'j') if ctrl => {
                self.selected = (self.selected + 1).min(count.saturating_sub(1));
            }
            KeyCode::Named(NamedKey::Up) => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char('p' | 'k') if ctrl => self.selected = self.selected.saturating_sub(1),
            KeyCode::Named(NamedKey::Backspace) => {
                self.input.pop();
                self.selected = 0;
                self.error = None;
            }
            KeyCode::Char('u') if ctrl => {
                self.input.clear();
                self.selected = 0;
                self.error = None;
            }
            KeyCode::Char(c) if !ctrl && !k.mods.alt => {
                self.input.push(c);
                self.selected = 0;
                self.error = None;
            }
            _ => {}
        }
        Outcome::Stay
    }

    fn answer(&mut self) -> Outcome {
        match &self.kind {
            Kind::Pick { free, .. } => {
                let free = *free;
                let matches = self.matches();
                if let Some(c) = matches.get(self.selected.min(matches.len().saturating_sub(1))) {
                    return Outcome::Answer(c.value.clone());
                }
                if free && !self.input.trim().is_empty() {
                    return Outcome::Answer(self.input.trim().to_owned());
                }
                self.error = Some("nothing matches".into());
                Outcome::Stay
            }
            Kind::Text => Outcome::Answer(self.input.trim().to_owned()),
            Kind::Confirm => Outcome::Stay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choices(names: &[&str]) -> Vec<Choice> {
        names
            .iter()
            .map(|n| Choice {
                value: n.to_string(),
                label: n.to_string(),
            })
            .collect()
    }

    #[test]
    fn a_pick_narrows_as_you_type_and_answers_the_highlighted_choice() {
        let mut p = Prompt::pick(
            "Status",
            choices(&["draft", "approved", "obsolete"]),
            Then::Baseline,
        );
        for c in "o".chars() {
            p.key(KeyPress::char(c));
        }
        assert_eq!(p.matches().len(), 2, "approved and obsolete");
        p.key(KeyPress::named(NamedKey::Down));
        assert_eq!(
            p.key(KeyPress::named(NamedKey::Enter)),
            Outcome::Answer("obsolete".into())
        );
        p.key(KeyPress::char('z'));
        assert_eq!(p.key(KeyPress::named(NamedKey::Enter)), Outcome::Stay);
        assert_eq!(p.error.as_deref(), Some("nothing matches"));
    }

    #[test]
    fn a_free_pick_takes_what_was_typed() {
        let mut p = Prompt::pick("Baseline", choices(&["v1.0"]), Then::Baseline).free();
        for c in "HEAD~3".chars() {
            p.key(KeyPress::char(c));
        }
        assert_eq!(
            p.key(KeyPress::named(NamedKey::Enter)),
            Outcome::Answer("HEAD~3".into())
        );
    }

    #[test]
    fn a_confirm_needs_y() {
        let mut p = Prompt::confirm("Publish?", Then::Baseline);
        assert_eq!(p.key(KeyPress::named(NamedKey::Enter)), Outcome::Stay);
        assert_eq!(p.key(KeyPress::char('x')), Outcome::Stay);
        assert_eq!(p.key(KeyPress::char('n')), Outcome::Cancel);
        assert_eq!(p.key(KeyPress::char('y')), Outcome::Answer("y".into()));
    }
}
