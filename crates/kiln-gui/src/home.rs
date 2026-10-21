//! Home, the projects dashboard (GUI.md, "Home"): what the window shows
//! when Kiln starts outside a project, and what `SPC h` goes back to.
//! Fenix's Home, cut to what Kiln has: the lockup and the date, a find
//! field, then two columns, the known projects (numbered, with when each
//! was last opened and a dot for how `kiln check` finds it) and a preview
//! of the one under the cursor (its items, types, next milestone and
//! findings). The columns stack into one when the window narrows.
//!
//! Like [`crate::app`], it is keys in, a [`Screen`] and an [`Effect`] out,
//! laid out in cells so tests read it as text. It keeps the list
//! ([`Projects`]) and asks the window to save it; the summaries are read
//! by the window on a thread ([`Home::wanted`], [`Home::summarized`]), so
//! a slow disk never holds up the window. Adding a project looks at the
//! disk (completing a folder, finding its `kiln.toml`), which is quick.
//!
//! It deliberately doesn't create projects (`kiln init` does, on the
//! command line) or delete folders: removing takes a project off the list.

use std::collections::BTreeMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

use fenix_keymap::{KeyCode, KeyPress, NamedKey};
use kiln_model::Date;

use crate::pages::Remote;
use crate::projects::{self, Health, Projects, Summary};
use crate::prompt::{Kind, Outcome, Prompt, Then};
use crate::screen::{span, Band, Fill, Line, Role, Screen};

/// What the window should do after a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    /// Open this project.
    Open(PathBuf),
    /// The list changed: write `projects.toml`.
    Save,
    Quit,
}

/// Content is centred and no wider than this, in cells (Fenix's width).
const MAX_WIDTH: usize = 112;
/// Between columns.
const GAP: usize = 4;
/// Below this width the preview goes under the list.
const TWO_COLUMNS: usize = 76;
/// Labels in the preview are this wide.
const LABEL: usize = 10;

/// Where something can be clicked: `rows` rows from `row`, cells `cols`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Slot {
    row: usize,
    rows: usize,
    cols: Range<usize>,
    /// The entry: a position in `shown()`, or `shown().len()` for "add".
    entry: usize,
}

/// Text placed in cells, turned into lines of spans at the end.
#[derive(Default)]
struct Grid {
    lines: Vec<Vec<(usize, String, Role)>>,
    bands: Vec<Band>,
    rules: Vec<(usize, Range<usize>)>,
}

impl Grid {
    fn put(&mut self, line: usize, col: usize, text: &str, role: Role) {
        if text.is_empty() {
            return;
        }
        if self.lines.len() <= line {
            self.lines.resize_with(line + 1, Vec::new);
        }
        self.lines[line].push((col, text.to_owned(), role));
    }

    fn band(&mut self, row: usize, rows: usize, cols: Range<usize>, fill: Fill) {
        self.bands.push(Band {
            row,
            rows,
            cols,
            fill,
        });
    }

    /// A section's title, its count, a rule, and its key at the right.
    fn header(
        &mut self,
        line: usize,
        x: usize,
        width: usize,
        name: &str,
        count: Option<usize>,
        key: &str,
    ) {
        self.put(line, x, name, Role::Heading);
        let mut at = x + name.chars().count() + 1;
        if let Some(n) = count {
            let n = n.to_string();
            self.put(line, at, &n, Role::Muted);
            at += n.len() + 1;
        }
        let key_at = (x + width).saturating_sub(key.chars().count());
        self.put(line, key_at, key, Role::Muted);
        if key_at > at + 2 {
            self.rules.push((line, at..key_at - 1));
        }
    }

    /// One line of the preview: a muted label, then the value.
    fn field(&mut self, line: usize, x: usize, width: usize, label: &str, value: &str, role: Role) {
        self.put(line, x, label, Role::Muted);
        self.put(
            line,
            x + LABEL,
            &fit(value, width.saturating_sub(LABEL)),
            role,
        );
    }

    /// The lines as spans: gaps padded with spaces, so columns line up in
    /// the monospace font; text running into the next piece is cut.
    fn into_lines(self) -> Vec<Line> {
        self.lines
            .into_iter()
            .map(|mut pieces| {
                pieces.sort_by_key(|p| p.0);
                let mut line = Vec::new();
                let mut at = 0;
                for (col, text, role) in pieces {
                    if col > at {
                        line.push(span(" ".repeat(col - at), Role::Normal));
                        at = col;
                    }
                    let text: String = text.chars().skip(at - col).collect();
                    at += text.chars().count();
                    line.push(span(text, role));
                }
                line
            })
            .collect()
    }
}

/// `s` cut to `max` cells, with `…` when cut.
fn fit(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    if max == 0 {
        return String::new();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

/// "Wednesday 21 October 2026".
pub fn long_date(d: Date) -> String {
    const DAYS: [&str; 7] = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    // Days since 1970-01-01, a Thursday (Howard Hinnant's algorithm).
    let (y, m) = (d.year as i64, d.month as i64);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d.day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let weekday = (days + 3).rem_euclid(7) as usize;
    format!(
        "{} {} {} {}",
        DAYS[weekday],
        d.day,
        MONTHS[(d.month as usize).saturating_sub(1).min(11)],
        d.year
    )
}

// implements: REQ-017
pub struct Home {
    projects: Projects,
    summaries: BTreeMap<PathBuf, Remote<Summary>>,
    /// A position in `shown()`; `shown().len()` is "add a project".
    cursor: usize,
    /// The first project the list shows, when they don't all fit.
    first: usize,
    filter: String,
    /// Typing goes to the find field.
    finding: bool,
    prompt: Option<Prompt>,
    message: Option<(String, bool)>,
    /// The project being opened, by name.
    opening: Option<String>,
    rows: usize,
    cols: usize,
    today: Date,
    /// Seconds since 1970, for the ages.
    now: u64,
    /// For `~` in paths, typed and shown.
    home_dir: Option<PathBuf>,
    pending_g: bool,
}

impl Home {
    pub fn new(projects: Projects, today: Date, now: u64, home_dir: Option<PathBuf>) -> Home {
        Home {
            projects,
            summaries: BTreeMap::new(),
            cursor: 0,
            first: 0,
            filter: String::new(),
            finding: false,
            prompt: None,
            message: None,
            opening: None,
            rows: 30,
            cols: 100,
            today,
            now,
            home_dir,
            pending_g: false,
        }
    }

    pub fn projects(&self) -> &Projects {
        &self.projects
    }

    /// The window's size, in rows and cells.
    pub fn set_size(&mut self, rows: usize, cols: usize) {
        self.rows = rows.max(1);
        self.cols = cols.max(1);
        self.scroll();
    }

    pub fn say(&mut self, text: impl Into<String>, error: bool) {
        self.message = Some((text.into(), error));
    }

    /// A project is being opened (`Some(name)`), or no longer is.
    pub fn set_opening(&mut self, name: Option<String>) {
        self.opening = name;
    }

    /// Puts the cursor on `root`'s entry, if it is shown.
    pub fn select(&mut self, root: &Path) -> bool {
        match self
            .shown()
            .iter()
            .position(|&i| self.projects.list[i].root == root)
        {
            Some(at) => {
                self.cursor = at;
                self.scroll();
                true
            }
            None => false,
        }
    }

    /// The projects whose summary hasn't been asked for, now marked as
    /// being read: the window reads them on a thread.
    pub fn wanted(&mut self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for k in &self.projects.list {
            let s = self
                .summaries
                .entry(k.root.clone())
                .or_insert(Remote::NotAsked);
            if *s == Remote::NotAsked {
                *s = Remote::Loading;
                out.push(k.root.clone());
            }
        }
        out
    }

    /// A summary arrived. One for a project no longer on the list is
    /// dropped.
    pub fn summarized(&mut self, root: &Path, summary: Result<Summary, String>) {
        if !self.projects.contains(root) {
            return;
        }
        self.summaries.insert(
            root.to_path_buf(),
            match summary {
                Ok(s) => Remote::Ready(s),
                Err(e) => Remote::Failed(e),
            },
        );
    }

    /// The list's entries that match the find field, as positions in it.
    fn shown(&self) -> Vec<usize> {
        let needle = self.filter.trim().to_lowercase();
        (0..self.projects.list.len())
            .filter(|&i| {
                needle.is_empty() || {
                    let root = &self.projects.list[i].root;
                    self.name(root).to_lowercase().contains(&needle)
                        || root.to_string_lossy().to_lowercase().contains(&needle)
                }
            })
            .collect()
    }

    /// The project's name: `kiln.toml`'s once read, else its folder's.
    fn name(&self, root: &Path) -> String {
        match self.summaries.get(root) {
            Some(Remote::Ready(s)) => s.name.clone(),
            _ => root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| root.display().to_string()),
        }
    }

    /// The path as shown: the home folder as `~`.
    fn show_path(&self, root: &Path) -> String {
        match self
            .home_dir
            .as_deref()
            .and_then(|h| root.strip_prefix(h).ok())
        {
            Some(rest) if rest.as_os_str().is_empty() => "~".into(),
            Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
            None => root.display().to_string(),
        }
    }

    /// The project under the cursor, if it is on one.
    fn current(&self) -> Option<PathBuf> {
        let shown = self.shown();
        shown
            .get(self.cursor)
            .map(|&i| self.projects.list[i].root.clone())
    }

    /// One key press.
    pub fn key(&mut self, k: KeyPress) -> Effect {
        self.message = None;
        if self.prompt.is_some() {
            return self.prompt_key(k);
        }
        if self.finding {
            return self.find_key(k);
        }
        let ctrl = k.mods.ctrl;
        if std::mem::take(&mut self.pending_g) && k == KeyPress::char('g') {
            self.move_to(0);
            return Effect::None;
        }
        let last = self.shown().len();
        match k.code {
            KeyCode::Char('q') if ctrl => return Effect::Quit,
            KeyCode::Char('n') if ctrl => self.step(1),
            KeyCode::Char('p') if ctrl => self.step(-1),
            _ if ctrl => {}
            KeyCode::Char('j') | KeyCode::Named(NamedKey::Down) => self.step(1),
            KeyCode::Char('k') | KeyCode::Named(NamedKey::Up) => self.step(-1),
            KeyCode::Char('g') => self.pending_g = true,
            KeyCode::Named(NamedKey::Home) => self.move_to(0),
            KeyCode::Char('G') | KeyCode::Named(NamedKey::End) => self.move_to(last),
            KeyCode::Named(NamedKey::Enter) => return self.activate(),
            KeyCode::Char(c @ '1'..='9') => {
                let n = c as usize - '1' as usize;
                if n < last {
                    self.move_to(n);
                    return self.activate();
                }
            }
            KeyCode::Char('a') => self.ask_add(),
            KeyCode::Char('d') => self.ask_remove(),
            KeyCode::Char('/') => self.finding = true,
            KeyCode::Char('R') => {
                self.summaries.clear();
                self.say("Reading the projects again", false);
            }
            KeyCode::Char('q') => return Effect::Quit,
            KeyCode::Named(NamedKey::Escape) if !self.filter.is_empty() => {
                self.filter.clear();
                self.move_to(0);
            }
            _ => {}
        }
        Effect::None
    }

    fn find_key(&mut self, k: KeyPress) -> Effect {
        let ctrl = k.mods.ctrl;
        match k.code {
            KeyCode::Named(NamedKey::Escape) => {
                self.finding = false;
                self.filter.clear();
                self.move_to(0);
            }
            KeyCode::Named(NamedKey::Enter) => {
                self.finding = false;
                if self.current().is_some() {
                    return self.activate();
                }
            }
            KeyCode::Named(NamedKey::Down) => self.step(1),
            KeyCode::Char('n' | 'j') if ctrl => self.step(1),
            KeyCode::Named(NamedKey::Up) => self.step(-1),
            KeyCode::Char('p' | 'k') if ctrl => self.step(-1),
            KeyCode::Named(NamedKey::Backspace) => {
                self.filter.pop();
                self.move_to(0);
            }
            KeyCode::Char('u') if ctrl => {
                self.filter.clear();
                self.move_to(0);
            }
            KeyCode::Char(c) if !ctrl && !k.mods.alt => {
                self.filter.push(c);
                self.move_to(0);
            }
            _ => {}
        }
        Effect::None
    }

    fn prompt_key(&mut self, k: KeyPress) -> Effect {
        let Some(p) = self.prompt.as_mut() else {
            return Effect::None;
        };
        if p.then == Then::AddProject && k.code == KeyCode::Named(NamedKey::Tab) {
            match projects::complete(&p.input, self.home_dir.as_deref()) {
                Some(done) => {
                    p.input = done;
                    p.error = None;
                }
                None => p.error = Some("no folder to complete".into()),
            }
            return Effect::None;
        }
        match p.key(k) {
            Outcome::Stay => Effect::None,
            Outcome::Cancel => {
                self.prompt = None;
                Effect::None
            }
            Outcome::Answer(answer) => {
                let then = p.then.clone();
                self.answer(then, answer)
            }
        }
    }

    fn answer(&mut self, then: Then, answer: String) -> Effect {
        match then {
            Then::AddProject => {
                if answer.is_empty() {
                    self.prompt = None;
                    return Effect::None;
                }
                let typed = PathBuf::from(projects::expand(&answer, self.home_dir.as_deref()));
                let start = std::path::absolute(&typed).unwrap_or(typed);
                let Some(root) = kiln_index::find_root(&start) else {
                    if let Some(p) = &mut self.prompt {
                        p.error = Some(format!(
                            "no kiln.toml in {} or any folder above it; `kiln init` starts one",
                            start.display()
                        ));
                    }
                    return Effect::None;
                };
                self.prompt = None;
                if !self.projects.add(&root) {
                    self.filter.clear();
                    self.select(&root);
                    self.say(
                        format!("{} is on the list already", self.name(&root)),
                        false,
                    );
                    return Effect::None;
                }
                self.filter.clear();
                self.select(&root);
                self.say(format!("Added {}", self.show_path(&root)), false);
                Effect::Save
            }
            Then::RemoveProject { root } => {
                self.prompt = None;
                let name = self.name(&root);
                if !self.projects.remove(&root) {
                    return Effect::None;
                }
                self.summaries.remove(&root);
                self.clamp();
                self.say(
                    format!("Removed {name} from the list; its folder is untouched"),
                    false,
                );
                Effect::Save
            }
            // A project's questions; never asked on Home.
            _ => {
                self.prompt = None;
                Effect::None
            }
        }
    }

    /// `a`: asks for a folder, starting from the one the project under
    /// the cursor is in, where its neighbours most likely are.
    fn ask_add(&mut self) {
        let start = self
            .current()
            .and_then(|r| r.parent().map(Path::to_path_buf))
            .map(|d| format!("{}{}", d.display(), std::path::MAIN_SEPARATOR))
            .unwrap_or_default();
        self.prompt = Some(Prompt::text(
            "Add the project in (Tab completes)",
            start,
            Then::AddProject,
        ));
    }

    /// `d`: asks before taking the project under the cursor off the list.
    fn ask_remove(&mut self) {
        let Some(root) = self.current() else {
            self.say("No project under the cursor", true);
            return;
        };
        self.prompt = Some(Prompt::confirm(
            format!(
                "Take {} ({}) off the list? Its folder stays. y/n",
                self.name(&root),
                self.show_path(&root)
            ),
            Then::RemoveProject { root },
        ));
    }

    /// Enter: open the project, or add one.
    fn activate(&mut self) -> Effect {
        if let Some(name) = &self.opening {
            let name = name.clone();
            self.say(format!("Opening {name}…"), false);
            return Effect::None;
        }
        match self.current() {
            Some(root) => Effect::Open(root),
            None => {
                self.ask_add();
                Effect::None
            }
        }
    }

    fn step(&mut self, by: isize) {
        let at = (self.cursor as isize + by).max(0) as usize;
        self.move_to(at);
    }

    fn move_to(&mut self, at: usize) {
        self.cursor = at;
        self.clamp();
    }

    fn clamp(&mut self) {
        self.cursor = self.cursor.min(self.shown().len());
        self.scroll();
    }

    /// Keeps the cursor's project among those the list has room for.
    fn scroll(&mut self) {
        let fit = self.layout_frame().fit;
        if self.cursor < self.first {
            self.first = self.cursor;
        } else if self.cursor < self.shown().len() && self.cursor >= self.first + fit {
            self.first = self.cursor + 1 - fit;
        }
        self.first = self.first.min(self.shown().len().saturating_sub(fit));
    }

    /// A click on row `row`, cell `col`; `open` for a double click.
    pub fn click(&mut self, row: usize, col: usize, open: bool) -> Effect {
        if self.prompt.is_some() {
            return Effect::None;
        }
        let slots = self.layout().1;
        let Some(slot) = slots
            .iter()
            .find(|s| (s.row..s.row + s.rows).contains(&row) && s.cols.contains(&col))
            .or_else(|| {
                slots
                    .iter()
                    .find(|s| (s.row..s.row + s.rows).contains(&row))
            })
        else {
            return Effect::None;
        };
        self.cursor = slot.entry;
        self.clamp();
        if open {
            self.activate()
        } else {
            Effect::None
        }
    }

    /// The wheel moves the cursor, a project at a time.
    pub fn wheel(&mut self, lines: isize) {
        self.step(lines.signum());
    }

    /// Where things go at this size.
    fn layout_frame(&self) -> Frame {
        let cols = self.cols.max(24);
        let width = cols.saturating_sub(4).clamp(20, MAX_WIDTH).min(cols);
        let left = (cols - width) / 2;
        let top = usize::from(self.rows >= 30);
        let find = top + 4;
        let body = find + 5;
        let two = width >= TWO_COLUMNS;
        let list_width = if two { (width - GAP) / 2 } else { width };
        // The header, then two rows a project, then "add a project"; in
        // one column, room is left under them for the preview.
        let room = self.rows.saturating_sub(body + 2 + if two { 0 } else { 9 });
        Frame {
            width,
            left,
            top,
            find,
            body,
            two,
            list_width,
            fit: (room / 2).max(1),
        }
    }

    /// The page's lines, and where its entries can be clicked.
    fn layout(&self) -> (Grid, Vec<Slot>) {
        let f = self.layout_frame();
        let mut g = Grid::default();
        let mut slots = Vec::new();
        let (left, width) = (f.left, f.width);

        // The lockup, and the date beside it.
        let date = long_date(self.today);
        if width > date.chars().count() + 30 {
            g.put(
                f.top + 1,
                left + width - date.chars().count(),
                &date,
                Role::Muted,
            );
        }

        // The find field: a three-row panel.
        g.band(f.find, 3, left..left + width, Fill::Panel);
        g.put(f.find + 1, left + 2, "›", Role::Accent);
        let key = " / ";
        let key_at = left + width - 2 - key.len();
        if self.filter.is_empty() && !self.finding {
            g.put(f.find + 1, left + 5, "Find a project", Role::Muted);
        } else {
            let text = fit(&self.filter, key_at.saturating_sub(left + 7));
            g.put(f.find + 1, left + 5, &text, Role::Normal);
            if self.finding {
                g.put(
                    f.find + 1,
                    left + 5 + text.chars().count(),
                    "▏",
                    Role::Accent,
                );
            }
        }
        g.put(f.find + 1, key_at, key, Role::Normal);
        g.band(f.find + 1, 1, key_at..key_at + key.len(), Fill::Key);

        // The projects.
        let shown = self.shown();
        let (x, w) = (left, f.list_width);
        let mut y = f.body;
        g.header(y, x, w, "projects", Some(shown.len()), "a add");
        y += 1;
        if shown.is_empty() {
            let why = if self.projects.list.is_empty() {
                "none yet: add a folder with a kiln.toml"
            } else {
                "nothing matches"
            };
            g.put(y, x + 4, &fit(why, w.saturating_sub(4)), Role::Muted);
            y += 1;
        }
        let end = (self.first + f.fit).min(shown.len());
        if self.first > 0 {
            g.put(f.body, x + w / 2, "↑", Role::Muted);
        }
        for (n, &i) in shown.iter().enumerate().take(end).skip(self.first) {
            let root = &self.projects.list[i].root;
            if n < 9 {
                g.put(y, x + 1, &(n + 1).to_string(), Role::Muted);
            }
            let (dot, role) = match self.summaries.get(root) {
                Some(Remote::Ready(s)) => (
                    "●",
                    match s.health() {
                        Health::Clean => Role::Good,
                        Health::Warnings => Role::Warn,
                        Health::Errors | Health::Missing => Role::Error,
                    },
                ),
                Some(Remote::Failed(_)) => ("●", Role::Error),
                _ => ("○", Role::Muted),
            };
            let age = projects::age(self.projects.list[i].opened, self.now);
            let age_at = (x + w).saturating_sub(age.chars().count() + 2);
            g.put(y, x + w - 1, dot, role);
            g.put(y, age_at, &age, Role::Muted);
            let name = fit(&self.name(root), age_at.saturating_sub(x + 6));
            g.put(y, x + 4, &name, Role::Normal);
            g.put(
                y + 1,
                x + 4,
                &fit(&self.show_path(root), w.saturating_sub(4)),
                Role::Muted,
            );
            if n == self.cursor {
                g.band(y, 2, x..x + w, Fill::Cursor);
            }
            slots.push(Slot {
                row: y,
                rows: 2,
                cols: x..x + w,
                entry: n,
            });
            y += 2;
        }
        if end < shown.len() {
            g.put(y, x + w / 2, "↓", Role::Muted);
            y += 1;
        }
        g.put(y, x + 1, "+", Role::Accent);
        g.put(y, x + 4, "add a project", Role::Normal);
        g.put(y, x + w - 1, "a", Role::Muted);
        if self.cursor == shown.len() {
            g.band(y, 1, x..x + w, Fill::Cursor);
        }
        slots.push(Slot {
            row: y,
            rows: 1,
            cols: x..x + w,
            entry: shown.len(),
        });
        y += 1;

        // The preview of the entry under the cursor.
        let (px, pw, py) = if f.two {
            (left + w + GAP, width - w - GAP, f.body)
        } else {
            (left, width, y + 1)
        };
        self.preview(&mut g, px, pw, py);
        (g, slots)
    }

    fn preview(&self, g: &mut Grid, x: usize, w: usize, mut y: usize) {
        let Some(root) = self.current() else {
            g.header(y, x, w, "add a project", None, "a");
            for (n, line) in [
                "Type the folder of a project with a kiln.toml;",
                "Tab completes folders, ~ is your home folder.",
                "",
                "No kiln.toml yet? `kiln init` in the folder",
                "starts one.",
            ]
            .iter()
            .enumerate()
            {
                g.put(y + 2 + n, x, &fit(line, w), Role::Muted);
            }
            return;
        };
        g.header(
            y,
            x,
            w,
            &fit(&self.name(&root), w.saturating_sub(12)),
            None,
            "Enter open",
        );
        y += 1;
        g.put(y, x, &fit(&self.show_path(&root), w), Role::Muted);
        y += 2;
        let opened = self
            .projects
            .list
            .iter()
            .find(|k| k.root == root)
            .map_or(0, |k| k.opened);
        match self.summaries.get(&root) {
            Some(Remote::Ready(s)) => {
                let open = if s.open == s.items {
                    String::new()
                } else {
                    format!(" · {} open", s.open)
                };
                g.field(
                    y,
                    x,
                    w,
                    "items",
                    &format!("{}{open}", s.items),
                    Role::Normal,
                );
                y += 1;
                let types = s
                    .types
                    .iter()
                    .map(|(t, n)| format!("{t} {n}"))
                    .collect::<Vec<_>>()
                    .join(" · ");
                if !types.is_empty() {
                    g.field(y, x, w, "types", &types, Role::Normal);
                    y += 1;
                }
                match &s.milestone {
                    Some(m) => {
                        let mut text = m.title.clone();
                        if let Some(d) = m.due {
                            text.push_str(&format!(" · due {d}"));
                        }
                        if let Some((done, total)) = m.progress {
                            text.push_str(&format!(" · {done}/{total}"));
                        }
                        g.field(y, x, w, "next", &text, Role::Normal);
                    }
                    None => g.field(y, x, w, "next", "no open milestone", Role::Muted),
                }
                y += 1;
                let plural =
                    |n: usize, one: &str| format!("{n} {one}{}", if n == 1 { "" } else { "s" });
                let (check, role) = match (s.errors, s.warnings) {
                    (0, 0) => ("clean".to_owned(), Role::Good),
                    (0, w) => (plural(w, "warning"), Role::Warn),
                    (e, 0) => (plural(e, "error"), Role::Error),
                    (e, w) => (
                        format!("{} · {}", plural(e, "error"), plural(w, "warning")),
                        Role::Error,
                    ),
                };
                g.field(y, x, w, "check", &check, role);
                y += 1;
            }
            Some(Remote::Failed(e)) => {
                g.field(y, x, w, "error", e, Role::Error);
                g.put(
                    y + 1,
                    x + LABEL,
                    &fit("d takes it off the list", w.saturating_sub(LABEL)),
                    Role::Muted,
                );
                y += 2;
            }
            _ => {
                g.field(y, x, w, "items", "reading…", Role::Muted);
                y += 1;
            }
        }
        let age = projects::age(opened, self.now);
        let age = if opened == 0 || age == "now" {
            age
        } else {
            format!("{age} ago")
        };
        g.field(y, x, w, "opened", &age, Role::Muted);
    }

    /// What to draw.
    pub fn screen(&self) -> Screen {
        let f = self.layout_frame();
        let (g, _) = self.layout();
        let count = self.projects.list.len();
        let top = vec![
            span("[Home]", Role::Accent),
            span(
                format!("  {count} project{}", if count == 1 { "" } else { "s" }),
                Role::Muted,
            ),
        ];
        let (footer, footer_error) = self.footer();
        let rows = self.rows;
        let mut lines = g.lines;
        lines.truncate(rows);
        let grid = Grid {
            lines,
            ..Grid::default()
        };
        Screen {
            top,
            rows: grid.into_lines(),
            cursor: None,
            headings: Vec::new(),
            footer,
            footer_error,
            popup: Vec::new(),
            bands: g.bands.into_iter().filter(|b| b.row < rows).collect(),
            rules: g.rules.into_iter().filter(|r| r.0 < rows).collect(),
            logo: (f.top + 3 <= rows).then_some((f.top, f.left)),
        }
    }

    fn footer(&self) -> (Line, bool) {
        if let Some(p) = &self.prompt {
            let mut line = vec![span(format!("{}: ", p.label), Role::Accent)];
            if p.kind != Kind::Confirm {
                line.push(span(p.input.clone(), Role::Normal));
                line.push(span("▏", Role::Accent));
            }
            if let Some(e) = &p.error {
                line.push(span(format!("  {e}"), Role::Error));
                return (line, true);
            }
            return (line, false);
        }
        if self.finding {
            return (
                vec![span(
                    "Type to find a project · ↓ ↑ move · Enter open · Esc clear",
                    Role::Muted,
                )],
                false,
            );
        }
        if let Some((m, error)) = &self.message {
            let role = if *error { Role::Error } else { Role::Normal };
            return (vec![span(m.clone(), role)], *error);
        }
        let mut line = Vec::new();
        if let Some(name) = &self.opening {
            line.push(span(format!("Opening {name}…  "), Role::Warn));
        }
        line.push(span(
            "Enter open · 1–9 open · j k move · a add · d remove · / find · R refresh · q quit",
            Role::Muted,
        ));
        (line, false)
    }
}

/// Where things go at one size.
struct Frame {
    width: usize,
    left: usize,
    top: usize,
    find: usize,
    body: usize,
    two: bool,
    list_width: usize,
    /// How many projects the list has room for.
    fit: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::{Known, Milestone};

    fn home(roots: &[&str]) -> Home {
        let projects = Projects {
            list: roots
                .iter()
                .enumerate()
                .map(|(i, r)| Known {
                    root: PathBuf::from(r),
                    opened: 100_000 - i as u64 * 3600,
                })
                .collect(),
        };
        let today = Date {
            year: 2026,
            month: 10,
            day: 21,
        };
        let mut h = Home::new(projects, today, 100_000 + 7200, Some("/home/t".into()));
        h.set_size(30, 100);
        h
    }

    fn summary(name: &str, errors: usize) -> Summary {
        Summary {
            name: name.into(),
            items: 42,
            open: 30,
            types: vec![("requirement".into(), 12), ("decision".into(), 14)],
            milestone: Some(Milestone {
                title: "MS-003 Beta".into(),
                due: Some(Date {
                    year: 2026,
                    month: 11,
                    day: 30,
                }),
                progress: Some((3, 5)),
            }),
            errors,
            warnings: 1,
        }
    }

    fn keys(h: &mut Home, text: &str) -> Effect {
        let mut last = Effect::None;
        for c in text.chars() {
            last = h.key(KeyPress::char(c));
        }
        last
    }

    #[test]
    fn dates_name_their_weekday() {
        let d = |year, month, day| long_date(Date { year, month, day });
        assert_eq!(d(2026, 10, 21), "Wednesday 21 October 2026");
        assert_eq!(d(1970, 1, 1), "Thursday 1 January 1970");
        assert_eq!(d(2024, 2, 29), "Thursday 29 February 2024");
    }

    #[test]
    // verifies: REQ-017
    fn the_dashboard_shows_the_projects_and_a_preview_of_the_selected_one() {
        let mut h = home(&["/home/t/src/kiln", "/home/t/src/flint"]);
        assert_eq!(h.wanted().len(), 2);
        assert!(h.wanted().is_empty(), "asked for once");
        h.summarized(Path::new("/home/t/src/kiln"), Ok(summary("Kiln", 2)));
        let dump = h.screen().dump();
        let expected = "\
[Home]  2 projects
\u{20}
\u{20}
\u{20}                                                                         Wednesday 21 October 2026
\u{20}
\u{20}
\u{20}
\u{20}    ›  Find a project                                                                         /
\u{20}
\u{20}
\u{20}
\u{20}  projects 2                               a add    Kiln                                Enter open
>   1  Kiln                                 2 h ●    ~/src/kiln
>      ~/src/kiln
\u{20}   2  flint                                3 h ○    items     42 · 30 open
\u{20}      ~/src/flint                                   types     requirement 12 · decision 14
\u{20}   +  add a project                            a    next      MS-003 Beta · due 2026-11-30 · 3/5
\u{20}                                                    check     2 errors · 1 warning
\u{20}                                                    opened    2 h ago
-- Enter open · 1–9 open · j k move · a add · d remove · / find · R refresh · q quit
";
        assert_eq!(dump, expected, "\n{dump}");
        let s = h.screen();
        assert_eq!(s.logo, Some((1, 2)));
        assert!(s.rules.iter().any(|r| r.0 == 10), "the section's rule");
    }

    #[test]
    fn keys_move_open_and_find() {
        let mut h = home(&["/a/kiln", "/a/flint", "/a/fenix"]);
        assert_eq!(
            h.key(KeyPress::named(NamedKey::Enter)),
            Effect::Open("/a/kiln".into())
        );
        assert_eq!(keys(&mut h, "3"), Effect::Open("/a/fenix".into()));
        assert_eq!(keys(&mut h, "4"), Effect::None, "no fourth project");
        keys(&mut h, "gg");
        keys(&mut h, "jj");
        assert_eq!(h.current(), Some("/a/fenix".into()));
        keys(&mut h, "j");
        assert_eq!(h.current(), None, "on `add a project`");
        keys(&mut h, "jjj");
        assert_eq!(h.cursor, 3, "stops at the end");
        keys(&mut h, "/fl");
        assert!(h.screen().dump().contains("fl▏"));
        assert_eq!(h.shown(), [1]);
        assert_eq!(
            h.key(KeyPress::named(NamedKey::Enter)),
            Effect::Open("/a/flint".into())
        );
        assert!(h.screen().dump().contains("projects 1"));
        h.key(KeyPress::named(NamedKey::Escape));
        assert_eq!(h.shown().len(), 3, "Esc clears the filter");
        assert_eq!(keys(&mut h, "q"), Effect::Quit);
    }

    #[test]
    fn removing_asks_first_and_keeps_the_folder() {
        let mut h = home(&["/a/kiln", "/a/flint"]);
        keys(&mut h, "jd");
        assert!(h
            .screen()
            .dump()
            .contains("Take flint (/a/flint) off the list? Its folder stays. y/n"));
        assert_eq!(keys(&mut h, "n"), Effect::None);
        assert_eq!(h.projects().list.len(), 2);
        keys(&mut h, "d");
        assert_eq!(keys(&mut h, "y"), Effect::Save);
        assert_eq!(h.projects().list.len(), 1);
        assert!(h
            .screen()
            .dump()
            .contains("Removed flint from the list; its folder is untouched"));
        // A late summary for it is dropped.
        h.summarized(Path::new("/a/flint"), Ok(summary("Flint", 0)));
        assert!(!h.summaries.contains_key(Path::new("/a/flint")));
    }

    #[test]
    // verifies: REQ-017
    fn adding_needs_a_kiln_toml_and_tab_completes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        std::fs::create_dir_all(root.join("design")).unwrap();
        std::fs::write(root.join("kiln.toml"), kiln_model::default_config("P")).unwrap();
        std::fs::create_dir(dir.path().join("empty")).unwrap();
        let mut h = home(&[]);
        assert!(h
            .screen()
            .dump()
            .contains("none yet: add a folder with a kiln.toml"));
        // Enter on `add a project`, the only entry.
        h.key(KeyPress::named(NamedKey::Enter));
        let base = format!("{}{}", dir.path().display(), std::path::MAIN_SEPARATOR);
        keys(&mut h, &format!("{base}em"));
        h.key(KeyPress::named(NamedKey::Tab));
        assert_eq!(
            h.prompt.as_ref().unwrap().input,
            format!("{base}empty{}", std::path::MAIN_SEPARATOR)
        );
        assert_eq!(h.key(KeyPress::named(NamedKey::Enter)), Effect::None);
        let error = h.prompt.as_ref().and_then(|p| p.error.clone()).unwrap();
        assert!(error.starts_with("no kiln.toml in "), "{error}");
        // A folder inside the project finds its root.
        h.key(KeyPress::char('u').with_ctrl());
        keys(
            &mut h,
            &format!("{base}project{}design", std::path::MAIN_SEPARATOR),
        );
        assert_eq!(h.key(KeyPress::named(NamedKey::Enter)), Effect::Save);
        assert!(h.prompt.is_none());
        let added = std::path::absolute(&root).unwrap();
        assert_eq!(h.projects().list[0].root, added);
        assert_eq!(h.current(), Some(added.clone()));
        // Again: already there, nothing to save.
        keys(&mut h, "a");
        h.key(KeyPress::char('u').with_ctrl());
        keys(&mut h, &added.display().to_string());
        assert_eq!(h.key(KeyPress::named(NamedKey::Enter)), Effect::None);
        assert!(h.screen().dump().contains("is on the list already"));
    }

    #[test]
    fn a_narrow_window_stacks_the_preview_and_the_list_scrolls() {
        let roots: Vec<String> = (0..12).map(|i| format!("/p/project{i:02}")).collect();
        let refs: Vec<&str> = roots.iter().map(String::as_str).collect();
        let mut h = home(&refs);
        h.set_size(24, 60);
        let dump = h.screen().dump();
        assert!(dump.contains("1  project00"), "{dump}");
        assert!(!dump.contains("project11"), "{dump}");
        assert!(dump.contains("↓"));
        keys(&mut h, "G");
        keys(&mut h, "k");
        let dump = h.screen().dump();
        assert!(dump.contains("project11"), "{dump}");
        assert!(dump.contains("↑"));
        // The preview is under the list.
        let lines: Vec<&str> = dump.lines().collect();
        let list = lines
            .iter()
            .position(|l| l.contains("add a project"))
            .unwrap();
        let preview = lines.iter().position(|l| l.contains("Enter open")).unwrap();
        assert!(preview > list, "{dump}");
    }

    #[test]
    fn clicks_select_and_double_clicks_open() {
        let mut h = home(&["/a/kiln", "/a/flint"]);
        // The second project's path row.
        assert_eq!(h.click(14, 10, false), Effect::None);
        assert_eq!(h.current(), Some("/a/flint".into()));
        assert_eq!(h.click(11, 10, true), Effect::Open("/a/kiln".into()));
        assert_eq!(h.click(0, 0, true), Effect::None);
    }

    #[test]
    fn a_project_that_cant_be_read_says_why() {
        let mut h = home(&["/gone"]);
        h.wanted();
        h.summarized(Path::new("/gone"), Err("no kiln.toml in /gone".into()));
        let dump = h.screen().dump();
        assert!(dump.contains("error     no kiln.toml in /gone"), "{dump}");
        assert!(dump.contains("d takes it off the list"));
    }
}
