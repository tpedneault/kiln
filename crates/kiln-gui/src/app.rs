//! The application without a window: which page, where each page's
//! cursor is, the filter, pending keys and the leader; key presses in,
//! a [`Screen`] and an [`Effect`] out (GUI.md, "Keys"). Everything here is
//! tested by feeding keys and reading the screen as text.

use std::path::PathBuf;

use fenix_keymap::{KeyCode, KeyPress, KeyTrie, NamedKey, Step};
use kiln_index::Project;
use kiln_model::ItemId;

use crate::pages::{self, Changes, Options, Page, Row};
use crate::screen::{plain, span, Line, Role, Screen};

/// What the window should do after a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    /// Open this file (absolute) at this line in Fenix.
    Open {
        path: PathBuf,
        line: u32,
    },
    Reload,
    Quit,
}

/// What a leader binding does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Page(Page),
    Reload,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    Filter,
}

#[derive(Debug, Clone, Default)]
struct PageState {
    cursor: usize,
    top: usize,
}

/// The baseline: what Changes compares against.
pub struct Baseline {
    /// `v1.0 (1a2b3c4)`.
    pub label: String,
    pub project: Project,
}

pub struct App {
    root: PathBuf,
    project: Project,
    baseline: Option<Baseline>,
    changes: Option<Changes>,
    page: Page,
    states: [PageState; 7],
    options: Options,
    filter: String,
    mode: Mode,
    /// `g` was pressed; a second `g` goes to the top.
    pending_g: bool,
    leader: KeyTrie<Action>,
    /// Keys after `SPC`, while the leader is open.
    leader_path: Option<Vec<KeyPress>>,
    message: Option<(String, bool)>,
    /// How many rows fit; the window says.
    visible: usize,
    rows: Vec<Row>,
}

fn leader() -> KeyTrie<Action> {
    let mut t = KeyTrie::new();
    for (key, label, action) in [
        ('t', "Tree", Action::Page(Page::Tree)),
        ('x', "Trace", Action::Page(Page::Trace)),
        ('i', "Impact", Action::Page(Page::Impact)),
        ('b', "Board", Action::Page(Page::Board)),
        ('r', "Roadmap", Action::Page(Page::Roadmap)),
        ('d', "Decisions", Action::Page(Page::Decisions)),
        ('c', "Changes", Action::Page(Page::Changes)),
        ('R', "Reload", Action::Reload),
        ('q', "Quit", Action::Quit),
    ] {
        t.insert(&[KeyPress::char(key)], label, action);
    }
    t
}

impl App {
    pub fn new(root: PathBuf, project: Project, baseline: Option<Baseline>) -> App {
        let mut app = App {
            root,
            project,
            baseline,
            changes: None,
            page: Page::Tree,
            states: Default::default(),
            options: Options::default(),
            filter: String::new(),
            mode: Mode::Normal,
            pending_g: false,
            leader: leader(),
            leader_path: None,
            message: None,
            visible: 30,
            rows: Vec::new(),
        };
        app.diff();
        app.refresh();
        app.first_selectable();
        app
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn page(&self) -> Page {
        self.page
    }

    /// Puts the cursor on `id` in the Tree, for `kiln gui --item`.
    pub fn select(&mut self, id: &ItemId) -> bool {
        self.switch(Page::Tree);
        match self.rows.iter().position(|r| r.item.as_ref() == Some(id)) {
            Some(i) => {
                self.state().cursor = i;
                self.scroll();
                true
            }
            None => false,
        }
    }

    /// A message for the footer.
    pub fn say(&mut self, text: impl Into<String>, error: bool) {
        self.message = Some((text.into(), error));
    }

    /// How many rows the window has room for.
    pub fn set_visible(&mut self, rows: usize) {
        self.visible = rows.max(1);
        self.scroll();
    }

    /// The project changed on disk: the new one, keeping each page's
    /// cursor on the same item where it can.
    pub fn reload(&mut self, project: Project) {
        let current = self
            .rows
            .get(self.states[self.page.index()].cursor)
            .and_then(|r| r.item.clone());
        self.project = project;
        self.diff();
        self.refresh();
        if let Some(id) = current {
            if let Some(i) = self.rows.iter().position(|r| r.item.as_ref() == Some(&id)) {
                self.state().cursor = i;
            }
        }
        self.clamp();
    }

    fn diff(&mut self) {
        self.changes = self.baseline.as_ref().map(|b| Changes {
            label: b.label.clone(),
            changes: kiln_index::diff(&b.project, &self.project),
        });
    }

    fn state(&mut self) -> &mut PageState {
        &mut self.states[self.page.index()]
    }

    fn refresh(&mut self) {
        let rows = pages::rows(
            self.page,
            &self.project,
            self.changes.as_ref(),
            &self.options,
        );
        self.rows = pages::filter(rows, &self.filter);
        self.clamp();
    }

    fn clamp(&mut self) {
        let len = self.rows.len();
        let s = &mut self.states[self.page.index()];
        if s.cursor >= len {
            s.cursor = len.saturating_sub(1);
        }
        let at = self.states[self.page.index()].cursor;
        if !self.selectable(at) {
            self.step(1);
            if !self.selectable(self.states[self.page.index()].cursor) {
                self.step(-1);
            }
        }
        self.scroll();
    }

    fn first_selectable(&mut self) {
        self.state().cursor = 0;
        self.clamp();
    }

    /// A row the cursor can be on: an item or a location, not a heading
    /// or a note.
    fn selectable(&self, i: usize) -> bool {
        self.rows
            .get(i)
            .is_some_and(|r| !r.heading && (r.item.is_some() || r.open.is_some()))
    }

    /// Moves the cursor by `by` selectable rows, stopping at the ends.
    fn step(&mut self, by: isize) {
        let mut at = self.states[self.page.index()].cursor as isize;
        let len = self.rows.len() as isize;
        let mut left = by.abs();
        let dir = by.signum();
        while left > 0 {
            let mut next = at + dir;
            while next >= 0 && next < len && !self.selectable(next as usize) {
                next += dir;
            }
            if next < 0 || next >= len {
                break;
            }
            at = next;
            left -= 1;
        }
        self.state().cursor = at.max(0) as usize;
        self.scroll();
    }

    fn scroll(&mut self) {
        let visible = self.visible;
        let s = &mut self.states[self.page.index()];
        if s.cursor < s.top {
            s.top = s.cursor;
            // Show the heading over the first item.
            if s.top > 0 && self.rows.get(s.top - 1).is_some_and(|r| r.heading) {
                s.top -= 1;
            }
        } else if s.cursor >= s.top + visible {
            s.top = s.cursor + 1 - visible;
        }
    }

    fn switch(&mut self, page: Page) {
        self.page = page;
        self.filter.clear();
        self.mode = Mode::Normal;
        self.refresh();
    }

    fn current(&self) -> Option<&Row> {
        self.rows
            .get(self.states[self.page.index()].cursor)
            .filter(|r| !r.heading)
    }

    /// One key press.
    pub fn key(&mut self, k: KeyPress) -> Effect {
        self.message = None;
        if self.mode == Mode::Filter {
            return self.filter_key(k);
        }
        if let Some(path) = self.leader_path.take() {
            return self.leader_key(path, k);
        }
        let ctrl = k.mods.ctrl;
        if self.pending_g {
            self.pending_g = false;
            if k == KeyPress::char('g') {
                self.first_selectable();
                return Effect::None;
            }
        }
        let half = (self.visible / 2).max(1) as isize;
        match k.code {
            KeyCode::Char('q') if ctrl => return Effect::Quit,
            KeyCode::Char('d') if ctrl => self.step(half),
            KeyCode::Char('u') if ctrl => self.step(-half),
            _ if ctrl => {}
            KeyCode::Char('j') | KeyCode::Named(NamedKey::Down) => self.step(1),
            KeyCode::Char('k') | KeyCode::Named(NamedKey::Up) => self.step(-1),
            KeyCode::Named(NamedKey::PageDown) => self.step(half),
            KeyCode::Named(NamedKey::PageUp) => self.step(-half),
            KeyCode::Char('g') => self.pending_g = true,
            KeyCode::Named(NamedKey::Home) => self.first_selectable(),
            KeyCode::Char('G') | KeyCode::Named(NamedKey::End) => {
                self.state().cursor = self.rows.len().saturating_sub(1);
                self.clamp();
            }
            KeyCode::Named(NamedKey::Enter) => return self.open(),
            KeyCode::Char('i') => match self.current().and_then(|r| r.item.clone()) {
                Some(id) => {
                    self.options.impact = Some(id);
                    self.states[Page::Impact.index()] = PageState::default();
                    self.switch(Page::Impact);
                    self.first_selectable();
                }
                None => self.say("No item under the cursor", true),
            },
            KeyCode::Char('/') => {
                self.mode = Mode::Filter;
            }
            KeyCode::Char('a') if matches!(self.page, Page::Tree | Page::Board) => {
                self.options.closed = !self.options.closed;
                self.refresh();
            }
            KeyCode::Char('m') if self.page == Page::Trace => {
                self.options.gaps_only = !self.options.gaps_only;
                self.refresh();
            }
            KeyCode::Char('T') if self.page == Page::Trace => {
                let types = pages::traceable(&self.project);
                if !types.is_empty() {
                    let now = self
                        .options
                        .trace_type
                        .clone()
                        .filter(|t| types.contains(t))
                        .unwrap_or_else(|| types[0].clone());
                    let next = types
                        .iter()
                        .position(|t| *t == now)
                        .map_or(0, |i| (i + 1) % types.len());
                    self.options.trace_type = Some(types[next].clone());
                    self.refresh();
                }
            }
            KeyCode::Char('R') => return Effect::Reload,
            KeyCode::Char('q') => return Effect::Quit,
            KeyCode::Char(' ') => self.leader_path = Some(Vec::new()),
            KeyCode::Named(NamedKey::Tab) | KeyCode::Char(']') => {
                self.switch(Page::ALL[(self.page.index() + 1) % Page::ALL.len()]);
            }
            KeyCode::Char('[') => {
                self.switch(Page::ALL[(self.page.index() + Page::ALL.len() - 1) % Page::ALL.len()]);
            }
            KeyCode::Char(c @ '1'..='7') => {
                self.switch(Page::ALL[c as usize - '1' as usize]);
            }
            KeyCode::Named(NamedKey::Escape) if !self.filter.is_empty() => {
                self.filter.clear();
                self.refresh();
            }
            _ => {}
        }
        Effect::None
    }

    fn filter_key(&mut self, k: KeyPress) -> Effect {
        match k.code {
            KeyCode::Named(NamedKey::Enter) => self.mode = Mode::Normal,
            KeyCode::Named(NamedKey::Escape) => {
                self.mode = Mode::Normal;
                self.filter.clear();
                self.refresh();
            }
            KeyCode::Named(NamedKey::Backspace) => {
                self.filter.pop();
                self.refresh();
            }
            KeyCode::Char(c) if !k.mods.ctrl && !k.mods.alt => {
                self.filter.push(c);
                self.refresh();
                self.first_selectable();
            }
            _ => {}
        }
        Effect::None
    }

    fn leader_key(&mut self, mut path: Vec<KeyPress>, k: KeyPress) -> Effect {
        if k.code == KeyCode::Named(NamedKey::Escape) {
            return Effect::None;
        }
        path.push(k);
        let mut matcher = self.leader.matcher();
        let mut found = None;
        for key in &path {
            match matcher.feed(*key) {
                Step::Matched(a) => found = Some(*a),
                Step::Pending(_) => {}
                Step::NoMatch => {
                    self.say("SPC: no such key", true);
                    return Effect::None;
                }
            }
        }
        match found {
            Some(Action::Page(p)) => {
                self.switch(p);
                Effect::None
            }
            Some(Action::Reload) => Effect::Reload,
            Some(Action::Quit) => Effect::Quit,
            None => {
                self.leader_path = Some(path);
                Effect::None
            }
        }
    }

    fn open(&mut self) -> Effect {
        match self.current().and_then(|r| r.open.clone()) {
            Some(o) => Effect::Open {
                path: self
                    .root
                    .join(o.path.replace('/', std::path::MAIN_SEPARATOR_STR)),
                line: o.line,
            },
            None => {
                self.say("Nothing to open here", true);
                Effect::None
            }
        }
    }

    /// A click on visible row `n`: moves the cursor there; `open` (a
    /// double click) also opens it.
    pub fn click(&mut self, n: usize, open: bool) -> Effect {
        let i = self.states[self.page.index()].top + n;
        if !self.selectable(i) {
            return Effect::None;
        }
        self.state().cursor = i;
        if open {
            self.open()
        } else {
            Effect::None
        }
    }

    /// The wheel: `lines` down (negative: up), moving the view, not the
    /// cursor, unless the cursor would leave it.
    pub fn wheel(&mut self, lines: isize) {
        let max_top = self.rows.len().saturating_sub(self.visible);
        let s = &mut self.states[self.page.index()];
        s.top = (s.top as isize + lines).clamp(0, max_top as isize) as usize;
        if s.cursor < s.top {
            s.cursor = s.top;
        } else if s.cursor >= s.top + self.visible {
            s.cursor = s.top + self.visible - 1;
        }
        if !self.selectable(self.states[self.page.index()].cursor) {
            self.step(1);
        }
    }

    /// What to draw.
    pub fn screen(&self) -> Screen {
        let s = &self.states[self.page.index()];
        let mut top: Line = Vec::new();
        for (n, p) in Page::ALL.iter().enumerate() {
            let role = if *p == self.page {
                Role::Accent
            } else {
                Role::Muted
            };
            let mark = if *p == self.page {
                format!("[{}]", p.name())
            } else {
                p.name().to_owned()
            };
            top.push(span(format!("{} {mark}  ", n + 1), role));
        }
        top.push(span(
            format!("  {}", self.project.config.name),
            Role::Heading,
        ));
        let end = (s.top + self.visible).min(self.rows.len());
        let rows: Vec<Line> = self.rows[s.top.min(end)..end]
            .iter()
            .map(|r| r.line.clone())
            .collect();
        let headings = (s.top..end)
            .filter(|i| self.rows[*i].heading)
            .map(|i| i - s.top)
            .collect();
        let cursor = (!self.rows.is_empty()
            && s.cursor >= s.top
            && s.cursor < end
            && self.selectable(s.cursor))
        .then(|| s.cursor - s.top);
        let (footer, footer_error) = self.footer();
        let popup = self.popup();
        Screen {
            top,
            rows,
            cursor,
            headings,
            footer,
            footer_error,
            popup,
        }
    }

    fn footer(&self) -> (Line, bool) {
        if self.mode == Mode::Filter {
            return (
                vec![
                    span("/", Role::Accent),
                    span(self.filter.clone(), Role::Normal),
                    span("▏", Role::Accent),
                ],
                false,
            );
        }
        if let Some((m, error)) = &self.message {
            return (
                vec![span(
                    m.clone(),
                    if *error { Role::Error } else { Role::Normal },
                )],
                *error,
            );
        }
        let mut line = Vec::new();
        if !self.filter.is_empty() {
            line.push(span(format!("/{}  ", self.filter), Role::Accent));
        }
        let count = (0..self.rows.len()).filter(|i| self.selectable(*i)).count();
        let noun = if count == 1 { "row" } else { "rows" };
        line.push(span(format!("{count} {noun}  "), Role::Muted));
        let mut hints = String::from("Enter open · i impact · / filter · SPC pages · q quit");
        match self.page {
            Page::Tree | Page::Board => hints.push_str(if self.options.closed {
                " · a hide closed"
            } else {
                " · a show closed"
            }),
            Page::Trace => hints.push_str(if self.options.gaps_only {
                " · m all · T type"
            } else {
                " · m gaps only · T type"
            }),
            _ => {}
        }
        line.push(span(hints, Role::Muted));
        (line, false)
    }

    fn popup(&self) -> Vec<Line> {
        let Some(path) = &self.leader_path else {
            return Vec::new();
        };
        let mut matcher = self.leader.matcher();
        for k in path {
            let _ = matcher.feed(*k);
        }
        let mut hints = matcher.pending_hints();
        hints.sort_by_key(|h| match h.key.code {
            KeyCode::Char(c) => (c.is_uppercase(), c),
            KeyCode::Named(_) => (true, '~'),
        });
        hints
            .into_iter()
            .map(|h| {
                let key = match h.key.code {
                    KeyCode::Char(c) => c.to_string(),
                    KeyCode::Named(n) => format!("{n:?}"),
                };
                vec![
                    span(format!("{key}  "), Role::Accent),
                    span(h.label, Role::Normal),
                ]
            })
            .collect()
    }

    /// The rows of the current page as text, for tests.
    pub fn text(&self) -> Vec<String> {
        self.rows
            .iter()
            .map(|r| plain(&r.line).trim_end().to_owned())
            .collect()
    }
}
