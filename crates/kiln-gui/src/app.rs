//! The application without a window: which page, where each page's
//! cursor is, the filter, the prompt, pending keys and the leader; key
//! presses in, a [`Screen`] and an [`Effect`] out (GUI.md, "Keys").
//! Everything here is tested by feeding keys and reading the screen as
//! text.
//!
//! Edits are decided here and checked against the project shown (the
//! same checks `kiln set` and `kiln link` make), so a bad value keeps the
//! prompt open with the reason; the window makes them ([`crate::edit`]).
//! Work that needs the binary (exports, Confluence, git) is queued as a
//! [`Job`] the window runs on a thread and hands back with [`App::done`].

use std::path::PathBuf;

use fenix_keymap::{KeyCode, KeyPress, KeyTrie, NamedKey, Step};
use kiln_index::{labelled, Project};
use kiln_model::{Finding, Item, ItemId, Severity};

use crate::edit::{Applied, Edit};
use crate::host::{CommentGroup, Done, ExportReport, Job};
use crate::pages::{self, Changes, Data, Options, Page, Remote, Row};
use crate::prompt::{self, Choice, Kind, Outcome, Prompt, Then};
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
    /// Open this address in the browser.
    OpenUrl(String),
    /// Make this edit, then reload.
    Edit(Edit),
    Reload,
    /// Leave the project for Home, the projects dashboard.
    Home,
    Quit,
}

/// The edits the leader's `SPC e` offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditKey {
    Status,
    Field(&'static str),
    Link,
    Unlink,
    New,
    NewChild,
    Renumber,
}

/// What a leader binding does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Page(Page),
    Edit(EditKey),
    Reload,
    Home,
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
    states: [PageState; Page::ALL.len()],
    options: Options,
    filter: String,
    mode: Mode,
    /// The question in the footer, while one is asked.
    prompt: Option<Prompt>,
    /// `g` was pressed; a second `g` goes to the top.
    pending_g: bool,
    leader: KeyTrie<Action>,
    /// Keys after `SPC`, while the leader is open.
    leader_path: Option<Vec<KeyPress>>,
    message: Option<(String, bool)>,
    /// How many rows fit; the window says.
    visible: usize,
    rows: Vec<Row>,
    /// Linked tickets' status, once the mirror has arrived.
    jira: Option<kiln_jira::Mirror>,
    /// `kiln check`'s findings, which the window computes on a thread
    /// after each load; `checking` while that runs.
    findings: Vec<Finding>,
    checking: bool,
    exports: Remote<Vec<ExportReport>>,
    comments: Remote<Vec<CommentGroup>>,
    /// The job running on the window's thread, and the one asked for next.
    running: Option<Job>,
    queued: Option<Job>,
    /// Items viewed before (`C-o`) and after (`C-i`) the current one.
    back: Vec<ItemId>,
    forward: Vec<ItemId>,
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
        ('v', "Item", Action::Page(Page::Item)),
        ('k', "Check", Action::Page(Page::Check)),
        ('p', "Exports", Action::Page(Page::Exports)),
        ('m', "Comments", Action::Page(Page::Comments)),
        ('R', "Reload", Action::Reload),
        ('h', "Home: projects", Action::Home),
        ('q', "Quit", Action::Quit),
    ] {
        t.insert(&[KeyPress::char(key)], label, action);
    }
    let e = KeyPress::char('e');
    t.label_group(&[e], "Edit…");
    for (key, label, edit) in [
        ('s', "Status", EditKey::Status),
        ('t', "Title", EditKey::Field("title")),
        ('p', "Parent", EditKey::Field("parent")),
        ('o', "Owner", EditKey::Field("owner")),
        ('d', "Due date", EditKey::Field("due")),
        ('l', "Link", EditKey::Link),
        ('u', "Unlink", EditKey::Unlink),
        ('n', "New item", EditKey::New),
        ('N', "New child", EditKey::NewChild),
        ('r', "Renumber", EditKey::Renumber),
    ] {
        t.insert(&[e, KeyPress::char(key)], label, Action::Edit(edit));
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
            prompt: None,
            pending_g: false,
            leader: leader(),
            leader_path: None,
            message: None,
            visible: 30,
            rows: Vec::new(),
            jira: None,
            findings: Vec::new(),
            checking: false,
            exports: Remote::NotAsked,
            comments: Remote::NotAsked,
            running: None,
            queued: None,
            back: Vec::new(),
            forward: Vec::new(),
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

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    /// Puts the cursor on `id` in the Tree, for `kiln gui --item`.
    pub fn select(&mut self, id: &ItemId) -> bool {
        self.switch(Page::Tree);
        self.cursor_to(id)
    }

    /// Moves the cursor on the current page to `id`'s row, if it has one.
    fn cursor_to(&mut self, id: &ItemId) -> bool {
        match self.rows.iter().position(|r| r.item.as_ref() == Some(id)) {
            Some(i) => {
                self.state().cursor = i;
                self.scroll();
                true
            }
            None => false,
        }
    }

    /// The Jira mirror arrived: items show their tickets' status.
    pub fn set_jira(&mut self, mirror: kiln_jira::Mirror) {
        self.jira = Some(mirror);
        self.refresh();
    }

    pub fn jira(&self) -> Option<&kiln_jira::Mirror> {
        self.jira.as_ref()
    }

    /// `kiln check`'s findings for the project shown.
    pub fn set_findings(&mut self, findings: Vec<Finding>) {
        self.findings = findings;
        self.checking = false;
        self.refresh();
    }

    /// `kiln check` is running: until its findings arrive, the Check
    /// page shows the last ones, saying so.
    pub fn set_checking(&mut self) {
        self.checking = true;
        self.refresh();
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
            self.cursor_to(&id);
        }
        self.clamp();
    }

    /// An edit was made (the window has reloaded): its line in the
    /// footer, and the cursor on the item it was about.
    pub fn edited(&mut self, applied: &Applied) {
        // The item shown was renumbered: show it under its new ID.
        let gone = self
            .options
            .item
            .as_ref()
            .is_some_and(|i| self.project.item(i).is_none());
        if gone && applied.item.is_some() {
            self.options.item = applied.item.clone();
            self.refresh();
        }
        if let Some(id) = &applied.item {
            self.cursor_to(id);
        }
        self.say(applied.message.clone(), false);
    }

    /// The next job for the window's thread, if none is running: one
    /// asked for, or the data the page shown needs.
    pub fn next_job(&mut self) -> Option<Job> {
        if self.running.is_some() {
            return None;
        }
        let job = self.queued.take().or_else(|| match self.page {
            Page::Exports if self.exports == Remote::NotAsked => {
                self.exports = Remote::Loading;
                Some(Job::PlanExports)
            }
            Page::Comments if self.comments == Remote::NotAsked => {
                self.comments = Remote::Loading;
                Some(Job::Comments)
            }
            _ => None,
        })?;
        self.running = Some(job.clone());
        self.refresh();
        Some(job)
    }

    fn queue(&mut self, job: Job) {
        if let Some(busy) = self.running.as_ref().or(self.queued.as_ref()) {
            let busy = busy.describe();
            self.say(format!("Busy: {busy}"), true);
            return;
        }
        self.queued = Some(job);
    }

    /// A job finished.
    pub fn done(&mut self, done: Done) {
        self.running = None;
        match done {
            Done::Exports(reports) => self.exports = Remote::Ready(reports),
            Done::Wrote(result) | Done::Published(result) => {
                match result {
                    Ok(lines) => {
                        let last = lines
                            .iter()
                            .rev()
                            .find(|l| !l.trim().is_empty())
                            .cloned()
                            .unwrap_or_else(|| "Done".into());
                        self.say(last.trim().to_owned(), false);
                    }
                    Err(e) => self.say(e, true),
                }
                // What was written or published changes the plan.
                self.exports = Remote::NotAsked;
            }
            Done::Pulled(Ok(lines)) => {
                let last = lines.last().cloned().unwrap_or_else(|| "Done".into());
                self.say(last, false);
            }
            Done::Pulled(Err(e)) => self.say(e, true),
            Done::Comments(Ok(groups)) => self.comments = Remote::Ready(groups),
            Done::Comments(Err(e)) => self.comments = Remote::Failed(e),
            Done::Tags(tags) => {
                let tags = tags.unwrap_or_else(|e| {
                    self.say(e, true);
                    Vec::new()
                });
                let choices = tags
                    .into_iter()
                    .map(|t| Choice {
                        value: t.clone(),
                        label: t,
                    })
                    .collect();
                self.prompt = Some(
                    Prompt::pick("Baseline (a tag, or any revision)", choices, Then::Baseline)
                        .free(),
                );
            }
            Done::Baseline(Ok(b)) => {
                self.say(format!("Changes since {}", b.label), false);
                self.baseline = Some(*b);
                self.diff();
                self.states[Page::Changes.index()] = PageState::default();
            }
            Done::Baseline(Err(e)) => self.say(e, true),
        }
        self.refresh();
        self.clamp();
    }

    /// The running job ended without an answer.
    pub fn job_failed(&mut self, why: String) {
        self.running = None;
        if self.exports == Remote::Loading {
            self.exports = Remote::Failed(why.clone());
        }
        if self.comments == Remote::Loading {
            self.comments = Remote::Failed(why.clone());
        }
        self.say(why, true);
        self.refresh();
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
        let data = Data {
            changes: self.changes.as_ref(),
            jira: self.jira.as_ref(),
            findings: &self.findings,
            checking: self.checking,
            exports: Some(&self.exports),
            comments: Some(&self.comments),
        };
        let rows = pages::rows(self.page, &self.project, &data, &self.options);
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

    /// A row the cursor can be on: one about something, not a heading or
    /// a note.
    fn selectable(&self, i: usize) -> bool {
        self.rows.get(i).is_some_and(|r| {
            !r.heading
                && (r.item.is_some() || r.open.is_some() || r.target.is_some() || r.url.is_some())
        })
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

    /// The item under the cursor, for an edit.
    fn current_item(&mut self) -> Option<Item> {
        let id = self.current().and_then(|r| r.item.clone());
        match id.and_then(|id| self.project.item(&id).cloned()) {
            Some(item) => Some(item),
            None => {
                self.say("No item under the cursor", true);
                None
            }
        }
    }

    /// One key press.
    pub fn key(&mut self, k: KeyPress) -> Effect {
        self.message = None;
        if self.prompt.is_some() {
            return self.prompt_key(k);
        }
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
            if k == KeyPress::char('h') {
                return self.home();
            }
        }
        let half = (self.visible / 2).max(1) as isize;
        match k.code {
            KeyCode::Char('q') if ctrl => return Effect::Quit,
            KeyCode::Char('d') if ctrl => self.step(half),
            KeyCode::Char('u') if ctrl => self.step(-half),
            KeyCode::Char('o') if ctrl => self.history(true),
            KeyCode::Char('i') if ctrl => self.history(false),
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
            KeyCode::Char('v') => match self.current().and_then(|r| r.item.clone()) {
                Some(id) => self.view(id, true),
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
            KeyCode::Char(c @ ('H' | 'L')) if self.page == Page::Board => {
                return self.shift_status(if c == 'L' { 1 } else { -1 });
            }
            KeyCode::Char('b') if self.page == Page::Changes => self.queue(Job::Tags),
            KeyCode::Char('w') if self.page == Page::Exports => self.write_export(false),
            KeyCode::Char('W') if self.page == Page::Exports => self.write_export(true),
            KeyCode::Char('P') if self.page == Page::Exports => self.ask_publish(),
            KeyCode::Char('J') => match self.current().and_then(|r| r.item.clone()) {
                Some(id) => self.queue(Job::JiraPull(id.to_string())),
                None => self.say("No item under the cursor", true),
            },
            KeyCode::Char('s') => self.start_edit(EditKey::Status),
            KeyCode::Char('l') => self.start_edit(EditKey::Link),
            KeyCode::Char('U') => self.start_edit(EditKey::Unlink),
            KeyCode::Char('n') => self.start_edit(EditKey::New),
            KeyCode::Char('N') => self.start_edit(EditKey::NewChild),
            KeyCode::Char('R') => {
                // Pages read on a thread are read again too.
                match self.page {
                    Page::Exports => self.exports = Remote::NotAsked,
                    Page::Comments => self.comments = Remote::NotAsked,
                    _ => {}
                }
                return Effect::Reload;
            }
            KeyCode::Char('q') => return Effect::Quit,
            KeyCode::Char(' ') => self.leader_path = Some(Vec::new()),
            KeyCode::Named(NamedKey::Tab) | KeyCode::Char(']') => {
                self.switch(Page::ALL[(self.page.index() + 1) % Page::ALL.len()]);
            }
            KeyCode::Char('[') => {
                self.switch(Page::ALL[(self.page.index() + Page::ALL.len() - 1) % Page::ALL.len()]);
            }
            KeyCode::Char(c @ '1'..='9') => {
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

    /// `SPC h`, `gh`: back to Home, unless a job is running, whose answer
    /// would have no page to land on (a publish half seen is worse than
    /// a wait).
    fn home(&mut self) -> Effect {
        match self.running.as_ref().or(self.queued.as_ref()) {
            Some(job) => {
                let busy = job.describe();
                self.say(format!("Busy: {busy} Home once it is done"), true);
                Effect::None
            }
            None => Effect::Home,
        }
    }

    /// The Item page for `id`; `remember` keeps the one shown before for
    /// `C-o`.
    fn view(&mut self, id: ItemId, remember: bool) {
        if remember {
            if let Some(before) = self.options.item.clone().filter(|b| *b != id) {
                self.back.push(before);
                self.forward.clear();
            }
        }
        self.options.item = Some(id);
        self.states[Page::Item.index()] = PageState::default();
        self.switch(Page::Item);
        self.first_selectable();
    }

    /// `C-o` (`back`) and `C-i` on the Item page.
    fn history(&mut self, back: bool) {
        if self.page != Page::Item {
            return;
        }
        let (from, to) = if back {
            (&mut self.back, &mut self.forward)
        } else {
            (&mut self.forward, &mut self.back)
        };
        let Some(next) = from.pop() else {
            self.say(
                if back {
                    "No item viewed before"
                } else {
                    "No item viewed after"
                },
                true,
            );
            return;
        };
        if let Some(now) = self.options.item.take() {
            to.push(now);
        }
        self.view(next, false);
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
            Some(Action::Edit(e)) => {
                self.start_edit(e);
                Effect::None
            }
            Some(Action::Reload) => Effect::Reload,
            Some(Action::Home) => self.home(),
            Some(Action::Quit) => Effect::Quit,
            None => {
                self.leader_path = Some(path);
                Effect::None
            }
        }
    }

    /// Asks for what `edit` needs, about the item under the cursor.
    fn start_edit(&mut self, edit: EditKey) {
        let p = &self.project;
        if edit == EditKey::New {
            let choices = p
                .config
                .types
                .iter()
                .map(|t| Choice {
                    value: t.name.clone(),
                    label: format!("{}  ({}-)", t.name, t.prefix),
                })
                .collect();
            self.prompt = Some(Prompt::pick(
                "New item of type",
                choices,
                Then::NewType { parent: None },
            ));
            return;
        }
        let file = self.current().and_then(|r| r.file.clone());
        let Some(item) = self.current_item() else {
            return;
        };
        let p = &self.project;
        let id = item.id.clone();
        let ty = p.type_of(&item).cloned();
        let prompt = match edit {
            EditKey::Status => {
                let Some(ty) = ty else {
                    self.say(
                        format!("{id}'s type `{}` isn't declared", item.type_name),
                        true,
                    );
                    return;
                };
                let choices = ty
                    .statuses
                    .iter()
                    .map(|s| Choice {
                        value: s.clone(),
                        label: if *s == item.status {
                            format!("{s}  (now)")
                        } else {
                            s.clone()
                        },
                    })
                    .collect();
                Prompt::pick(
                    format!("Status of {id}"),
                    choices,
                    Then::Set {
                        id,
                        key: "status".into(),
                    },
                )
            }
            EditKey::Field("parent") => {
                let allowed = ty.map(|t| t.parents).unwrap_or_default();
                let mut choices = vec![Choice {
                    value: String::new(),
                    label: "(no parent)".into(),
                }];
                choices.extend(
                    first_copies(p)
                        .into_iter()
                        .filter(|c| allowed.contains(&c.type_name) && c.id != id)
                        .filter(|c| !p.ancestors(&c.id).iter().any(|a| a.id == id))
                        .map(|c| Choice {
                            value: c.id.to_string(),
                            label: labelled(c),
                        }),
                );
                Prompt::pick(
                    format!("Parent of {id}"),
                    choices,
                    Then::Set {
                        id,
                        key: "parent".into(),
                    },
                )
            }
            EditKey::Field(key) => {
                let now = match key {
                    "title" => item.title.clone(),
                    "owner" => item.owner.clone().unwrap_or_default(),
                    _ => item.due.map(|d| d.to_string()).unwrap_or_default(),
                };
                let label = match key {
                    "due" => format!("Due date of {id} (YYYY-MM-DD, empty for none)"),
                    "owner" => format!("Owner of {id} (empty for none)"),
                    _ => format!("Title of {id}"),
                };
                Prompt::text(
                    label,
                    now,
                    Then::Set {
                        id,
                        key: key.into(),
                    },
                )
            }
            EditKey::Link => {
                let choices: Vec<Choice> = p
                    .config
                    .links
                    .iter()
                    .filter(|r| r.allows_from(&item.type_name))
                    .map(|r| Choice {
                        value: r.name.clone(),
                        label: format!("{}  (to {})", r.name, r.to.join(", ")),
                    })
                    .collect();
                if choices.is_empty() {
                    self.say(format!("No link role starts at a {}", item.type_name), true);
                    return;
                }
                Prompt::pick(format!("Link {id} by"), choices, Then::LinkRole { id })
            }
            EditKey::Unlink => {
                let choices: Vec<Choice> = item
                    .links
                    .iter()
                    .map(|l| Choice {
                        value: format!("{} {}", l.role, l.target.raw),
                        label: format!(
                            "{} {}",
                            l.role,
                            l.target
                                .id
                                .as_ref()
                                .ok()
                                .and_then(|t| p.item(t))
                                .map(labelled)
                                .unwrap_or_else(|| l.target.raw.clone())
                        ),
                    })
                    .collect();
                if choices.is_empty() {
                    self.say(format!("{id} has no links"), true);
                    return;
                }
                Prompt::pick(format!("Unlink from {id}"), choices, Then::Unlink { id })
            }
            EditKey::NewChild => {
                let choices: Vec<Choice> = p
                    .config
                    .types
                    .iter()
                    .filter(|t| t.parents.contains(&item.type_name))
                    .map(|t| Choice {
                        value: t.name.clone(),
                        label: format!("{}  ({}-)", t.name, t.prefix),
                    })
                    .collect();
                if choices.is_empty() {
                    self.say(
                        format!("No type can have a {} as parent", item.type_name),
                        true,
                    );
                    return;
                }
                Prompt::pick(
                    format!("New child of {id}, of type"),
                    choices,
                    Then::NewType { parent: Some(id) },
                )
            }
            EditKey::Renumber => {
                let next = p.next_id(id.prefix()).to_string();
                Prompt::text(
                    format!("Renumber {id} to"),
                    next,
                    Then::Renumber { from: id, file },
                )
            }
            EditKey::New => return,
        };
        self.prompt = Some(prompt);
    }

    fn prompt_key(&mut self, k: KeyPress) -> Effect {
        let Some(prompt) = self.prompt.as_mut() else {
            return Effect::None;
        };
        match prompt.key(k) {
            Outcome::Stay => Effect::None,
            Outcome::Cancel => {
                self.prompt = None;
                Effect::None
            }
            Outcome::Answer(answer) => {
                let then = prompt.then.clone();
                self.answer(then, answer)
            }
        }
    }

    /// Keeps the prompt open with why `answer` wasn't taken.
    fn refuse(&mut self, why: String) -> Effect {
        if let Some(p) = self.prompt.as_mut() {
            p.error = Some(why);
        }
        Effect::None
    }

    fn answer(&mut self, then: Then, answer: String) -> Effect {
        let p = &self.project;
        let item = |id: &ItemId| p.item(id).cloned();
        match then {
            Then::Set { id, key } => {
                let Some(it) = item(&id) else {
                    return self.refuse(format!("{id} isn't in the project any more"));
                };
                if let Err(e) = kiln_index::plan_set(p, &it, &key, &answer) {
                    return self.refuse(e);
                }
                self.prompt = None;
                let now = match key.as_str() {
                    "status" => it.status.clone(),
                    "title" => it.title.clone(),
                    "parent" => it.parent.map(|r| r.raw).unwrap_or_default(),
                    "owner" => it.owner.unwrap_or_default(),
                    _ => it.due.map(|d| d.to_string()).unwrap_or_default(),
                };
                if now == answer {
                    self.say(format!("{id}: {key} is already {}", show(&answer)), false);
                    return Effect::None;
                }
                Effect::Edit(Edit::Set {
                    id,
                    key,
                    value: answer,
                })
            }
            Then::LinkRole { id } => {
                let Some(it) = item(&id) else {
                    return self.refuse(format!("{id} isn't in the project any more"));
                };
                let Some(role) = p.config.role(&answer) else {
                    return self.refuse(format!("`{answer}` isn't a link role"));
                };
                let linked: Vec<&str> = it
                    .links
                    .iter()
                    .filter(|l| l.role == answer)
                    .map(|l| l.target.raw.as_str())
                    .collect();
                let choices: Vec<Choice> = first_copies(p)
                    .into_iter()
                    .filter(|c| role.allows_to(&c.type_name) && c.id != id)
                    .filter(|c| !linked.contains(&c.id.to_string().as_str()))
                    .map(|c| Choice {
                        value: c.id.to_string(),
                        label: labelled(c),
                    })
                    .collect();
                if choices.is_empty() {
                    self.prompt = None;
                    self.say(
                        format!("No item {id} can `{answer}` that it doesn't already"),
                        true,
                    );
                    return Effect::None;
                }
                self.prompt = Some(Prompt::pick(
                    format!("{id} {answer}"),
                    choices,
                    Then::LinkTarget { id, role: answer },
                ));
                Effect::None
            }
            Then::LinkTarget { id, role } => {
                let Ok(target) = answer.parse::<ItemId>() else {
                    return self.refuse(format!("`{answer}` isn't an ID"));
                };
                let Some(it) = item(&id) else {
                    return self.refuse(format!("{id} isn't in the project any more"));
                };
                if let Err(e) = kiln_index::check_link(p, &it, &role, &target) {
                    return self.refuse(e);
                }
                self.prompt = None;
                Effect::Edit(Edit::Link {
                    id,
                    role,
                    target,
                    add: true,
                })
            }
            Then::Unlink { id } => {
                let Some((role, target)) = answer.split_once(' ') else {
                    return self.refuse(format!("`{answer}` isn't a link"));
                };
                let Ok(target) = target.parse::<ItemId>() else {
                    return self.refuse(format!("`{target}` isn't an ID"));
                };
                self.prompt = Some(Prompt::confirm(
                    format!("Remove `{role} {target}` from {id}? y/n"),
                    Then::UnlinkConfirm {
                        id,
                        role: role.to_owned(),
                        target,
                    },
                ));
                Effect::None
            }
            Then::UnlinkConfirm { id, role, target } => {
                self.prompt = None;
                Effect::Edit(Edit::Link {
                    id,
                    role,
                    target,
                    add: false,
                })
            }
            Then::NewType { parent } => {
                let Some(ty) = p.config.type_named(&answer) else {
                    return self.refuse(format!("`{answer}` isn't a type"));
                };
                self.prompt = Some(Prompt::text(
                    format!(
                        "Title of the new {} (empty: \"Untitled {}\")",
                        ty.name,
                        ty.title.to_lowercase()
                    ),
                    "",
                    Then::NewTitle {
                        type_name: answer,
                        parent,
                    },
                ));
                Effect::None
            }
            Then::NewTitle { type_name, parent } => {
                if answer.chars().count() > kiln_parse::MAX_TITLE {
                    return self.refuse(format!(
                        "a title is one line of 1 to {} characters",
                        kiln_parse::MAX_TITLE
                    ));
                }
                self.prompt = None;
                Effect::Edit(Edit::New {
                    type_name,
                    title: (!answer.is_empty()).then_some(answer),
                    parent,
                })
            }
            Then::Renumber { from, file } => {
                let to = match answer.parse::<ItemId>() {
                    Ok(to) => to,
                    Err(e) => return self.refuse(format!("`{answer}`: {e}")),
                };
                if to.prefix() != from.prefix() {
                    return self.refuse(format!("{to} doesn't have {from}'s prefix"));
                }
                if p.item(&to).is_some() {
                    return self.refuse(format!("{to} already exists"));
                }
                self.prompt = Some(Prompt::confirm(
                    format!("Renumber {from} → {to}, rewriting every reference? y/n"),
                    Then::RenumberConfirm { from, to, file },
                ));
                Effect::None
            }
            Then::RenumberConfirm { from, to, file } => {
                self.prompt = None;
                Effect::Edit(Edit::Renumber { from, to, file })
            }
            Then::Publish { target, shown } => {
                self.prompt = None;
                self.queue(Job::Publish { target, shown });
                Effect::None
            }
            Then::Baseline => {
                self.prompt = None;
                self.queue(Job::Baseline(answer));
                Effect::None
            }
            // Home's questions; never asked on a project's pages.
            Then::AddProject | Then::RemoveProject { .. } => {
                self.prompt = None;
                Effect::None
            }
        }
    }

    /// Board: `L` moves the item to the next status in its type's order,
    /// `H` to the previous one.
    fn shift_status(&mut self, by: isize) -> Effect {
        let Some(item) = self.current_item() else {
            return Effect::None;
        };
        let Some(ty) = self.project.type_of(&item) else {
            self.say(format!("{}'s type isn't declared", item.id), true);
            return Effect::None;
        };
        let at = ty.statuses.iter().position(|s| *s == item.status);
        let next = at.and_then(|i| i.checked_add_signed(by));
        match next.and_then(|i| ty.statuses.get(i)) {
            Some(status) => Effect::Edit(Edit::Set {
                id: item.id.clone(),
                key: "status".into(),
                value: status.clone(),
            }),
            None => {
                self.say(
                    format!(
                        "{} is {} already",
                        item.id,
                        if by > 0 { "last" } else { "first" }
                    ),
                    true,
                );
                Effect::None
            }
        }
    }

    fn report(&mut self) -> Option<ExportReport> {
        let name = self.current().and_then(|r| r.target.clone());
        let found = match (&self.exports, name) {
            (Remote::Ready(reports), Some(name)) => {
                reports.iter().find(|r| r.name == name).cloned()
            }
            _ => None,
        };
        if found.is_none() {
            self.say("No export target under the cursor", true);
        }
        found
    }

    /// `w` writes the file target under the cursor, `W` every one.
    fn write_export(&mut self, all: bool) {
        let names: Vec<String> = if all {
            match &self.exports {
                Remote::Ready(reports) => reports
                    .iter()
                    .filter(|r| !r.confluence && !r.jira)
                    .map(|r| r.name.clone())
                    .collect(),
                _ => Vec::new(),
            }
        } else {
            let Some(r) = self.report() else {
                return;
            };
            if r.confluence || r.jira {
                self.say(
                    format!("`{}` isn't a file target: P publishes it", r.name),
                    true,
                );
                return;
            }
            vec![r.name]
        };
        if names.is_empty() {
            self.say("No file export targets", true);
            return;
        }
        self.queue(Job::WriteExports(names));
    }

    /// `P`: publishing a Confluence target, after a `y`.
    fn ask_publish(&mut self) {
        let Some(r) = self.report() else {
            return;
        };
        if !r.confluence && !r.jira {
            self.say(format!("`{}` is a file target: w writes it", r.name), true);
            return;
        }
        if let Some(e) = &r.error {
            self.say(format!("`{}` has no plan: {e}", r.name), true);
            return;
        }
        let refused = r
            .lines
            .iter()
            .find(|l| l.trim_start().starts_with("refused:"))
            .cloned();
        if let Some(why) = refused {
            self.say(
                format!(
                    "{}; the command line decides that: kiln export {} --force or --adopt (CONFLUENCE.md)",
                    why.trim(),
                    r.name
                ),
                true,
            );
            return;
        }
        let head = r.lines.first().cloned().unwrap_or_else(|| r.name.clone());
        self.prompt = Some(Prompt::confirm(
            format!("Publish {head}? y/n"),
            Then::Publish {
                shown: r.plan(),
                target: r.name,
            },
        ));
    }

    fn open(&mut self) -> Effect {
        let row = self.current().cloned();
        if let Some(url) = row.as_ref().and_then(|r| r.url.clone()) {
            return Effect::OpenUrl(url);
        }
        match row.and_then(|r| r.open) {
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
        if self.prompt.is_some() {
            return Effect::None;
        }
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
            let current = *p == self.page;
            let role = if current { Role::Accent } else { Role::Muted };
            let name = if current {
                format!("[{}]", p.name())
            } else {
                p.name().to_owned()
            };
            let text = if n < 9 {
                format!("{} {name}  ", n + 1)
            } else {
                format!("{name}  ")
            };
            top.push(span(text, role));
        }
        top.push(span(
            format!("  {}", self.project.config.name),
            Role::Heading,
        ));
        let errors = self
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count();
        if errors > 0 {
            top.push(span(
                format!("  {errors} error{}", if errors == 1 { "" } else { "s" }),
                Role::Error,
            ));
        }
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
            ..Screen::default()
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
        if let Some(job) = &self.running {
            line.push(span(format!("{}  ", job.describe()), Role::Warn));
        }
        if !self.filter.is_empty() {
            line.push(span(format!("/{}  ", self.filter), Role::Accent));
        }
        let count = (0..self.rows.len()).filter(|i| self.selectable(*i)).count();
        let noun = if count == 1 { "row" } else { "rows" };
        line.push(span(format!("{count} {noun}  "), Role::Muted));
        let mut hints = String::from(
            "Enter open · i impact · v item · / filter · SPC pages · SPC h projects · q quit",
        );
        match self.page {
            Page::Tree | Page::Board => {
                hints.push_str(if self.options.closed {
                    " · a hide closed"
                } else {
                    " · a show closed"
                });
                if self.page == Page::Board {
                    hints.push_str(" · H/L status");
                }
            }
            Page::Trace => hints.push_str(if self.options.gaps_only {
                " · m all · T type"
            } else {
                " · m gaps only · T type"
            }),
            Page::Item => hints.push_str(" · C-o back · J status from Jira"),
            Page::Changes => hints.push_str(" · b baseline"),
            Page::Exports => hints.push_str(" · w write · W write all · P publish · R plan again"),
            Page::Comments => hints.push_str(" · R read again"),
            _ => {}
        }
        hints.push_str(" · s status · l link · n new · SPC e edit");
        line.push(span(hints, Role::Muted));
        (line, false)
    }

    fn popup(&self) -> Vec<Line> {
        if let Some(p) = &self.prompt {
            let matches = p.matches();
            let first = p.selected.saturating_sub(prompt::SHOWN - 1);
            let mut out: Vec<Line> = matches
                .iter()
                .enumerate()
                .skip(first)
                .take(prompt::SHOWN)
                .map(|(i, c)| {
                    if i == p.selected {
                        vec![
                            span("> ", Role::Accent),
                            span(c.label.clone(), Role::Accent),
                        ]
                    } else {
                        vec![
                            span("  ", Role::Normal),
                            span(c.label.clone(), Role::Normal),
                        ]
                    }
                })
                .collect();
            if matches.len() > prompt::SHOWN {
                out.push(vec![span(
                    format!("  … {} in all; type to narrow", matches.len()),
                    Role::Muted,
                )]);
            }
            return out;
        }
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

/// Each ID once (the first copy), in project order: what picks list.
fn first_copies(p: &Project) -> Vec<&Item> {
    let mut seen = std::collections::HashSet::new();
    p.items.iter().filter(|i| seen.insert(&i.id)).collect()
}

fn show(s: &str) -> &str {
    if s.is_empty() {
        "(none)"
    } else {
        s
    }
}
