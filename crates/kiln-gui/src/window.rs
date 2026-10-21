//! The window: winit's event loop, the GPU, and the glue between them and
//! what it shows: [`Home`], the projects dashboard, or a project's pages
//! ([`App`]). Keys become [`KeyPress`]es for either; their effects open
//! files in Fenix or addresses in the browser, make edits, reload, switch
//! projects or quit. A project is re-read when its item files or
//! `kiln.toml` change, looked at every two seconds. `kiln check` runs on
//! a thread of its own after each load, since on a large project it
//! takes seconds the window shouldn't freeze for. The app's jobs run one
//! at a time on a thread through the binary's [`Host`]. Opening a project
//! from Home, and Home's summaries, run on threads too. Threads wake the
//! loop when they are done.
//!
//! Switching projects drops the one shown with its threads' answers (a
//! running job keeps Home away, see `App`), so nothing from one project
//! lands on another's pages.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use kiln_index::{DiskSource, FileSource, Project, Stamp};
use kiln_model::{Date, Finding, ItemId, Rule, Severity};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::{Icon, Window, WindowId};

use crate::app::{App, Baseline, Effect};
use crate::gpu::{Gpu, Surface};
use crate::home::{self, Home};
use crate::host::{Done, Host};
use crate::projects::{self, Projects, Summary};
use crate::render::{layout, Layout, Renderer};
use crate::screen::Screen;
use crate::settings::Settings;

/// How often the project is checked for changes on disk.
const POLL: Duration = Duration::from_secs(2);
/// Two clicks on a row this close together open it.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// A project, read and ready to show: what the binary hands over, at
/// start or when Home opens one.
pub struct Opened {
    pub root: PathBuf,
    pub project: Project,
    pub baseline: Option<Baseline>,
    /// Reads the Jira mirror; run on a thread of its own.
    pub jira: Option<JiraLoad>,
    /// Exports, Confluence and git, for the app's jobs.
    pub host: Arc<dyn Host>,
}

/// How Home opens a project: the binary's, which reads the baseline from
/// git and knows where Jira's token and cache are. Run on a thread.
pub type Opener = Arc<dyn Fn(&Path) -> Result<Opened, String> + Send + Sync>;

/// What the window starts with.
pub struct Options {
    /// The project to show; `None` starts on Home.
    pub start: Option<Opened>,
    pub settings: Settings,
    /// Problems with the settings, shown in the footer.
    pub notes: Vec<String>,
    /// `--item`: the item to put the cursor on.
    pub select: Option<ItemId>,
    pub open: Opener,
    /// `projects.toml`, Home's list; `None` (no state folder) keeps the
    /// list for this run only.
    pub projects: Option<PathBuf>,
    /// For `~` on Home.
    pub home_dir: Option<PathBuf>,
}

/// How the window gets the Jira mirror: the binary's, which knows where
/// tokens and the cache live.
pub type JiraLoad = Box<dyn FnOnce() -> Result<kiln_jira::Mirror, String> + Send>;

/// Opens the window and runs until it is closed.
pub fn run(o: Options) -> Result<(), String> {
    let event_loop = event_loop()?;
    event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + POLL));
    let proxy = event_loop.create_proxy();
    let (projects, problems) = match &o.projects {
        Some(path) => Projects::load(path),
        None => (Projects::default(), Vec::new()),
    };
    let (summary_tx, summary_rx) = std::sync::mpsc::channel();
    let mut win = Win {
        proxy,
        opener: o.open,
        projects_file: o.projects,
        projects,
        home_dir: o.home_dir,
        view: View::Home(Box::new(Home::new(
            Projects::default(),
            today(),
            projects::now(),
            None,
        ))),
        opening: None,
        summary_tx,
        summary_rx,
        settings: o.settings,
        window: None,
        gpu: None,
        surface: None,
        renderer: None,
        mods: ModifiersState::empty(),
        mouse: (0.0, 0.0),
        last_click: None,
        failed: None,
    };
    match o.start {
        Some(opened) => {
            win.show_project(opened);
            if let (Some(id), View::Project(s)) = (&o.select, &mut win.view) {
                if !s.app.select(id) {
                    s.app.say(format!("{id} isn't in the project"), true);
                }
            }
        }
        None => win.show_home(),
    }
    for note in o.notes.iter().chain(&problems).take(1) {
        win.say(note.clone(), true);
    }
    event_loop
        .run_app(&mut win)
        .map_err(|e| format!("the window's event loop failed: {e}"))?;
    match win.failed {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// winit's event loop, or why there is none. winit panics, rather than
/// failing, when a library it loads at run time is missing (X11's
/// `libxkbcommon-x11` on a minimal Linux); that becomes an error naming
/// it, since Kiln never panics on its environment.
fn event_loop() -> Result<EventLoop<()>, String> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let made = std::panic::catch_unwind(EventLoop::new);
    std::panic::set_hook(hook);
    match made {
        Ok(Ok(l)) => Ok(l),
        Ok(Err(e)) => Err(format!("couldn't connect to the display: {e}")),
        Err(panic) => {
            let why = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                .unwrap_or_else(|| "the window system failed to start".into());
            Err(format!("couldn't connect to the display: {why}"))
        }
    }
}

/// What the window shows.
enum View {
    Home(Box<Home>),
    Project(Box<Session>),
}

/// A project shown, with its threads.
struct Session {
    app: App,
    root: PathBuf,
    host: Arc<dyn Host>,
    stamps: BTreeMap<String, Stamp>,
    /// The Jira mirror, until it arrives.
    jira: Option<Receiver<Result<kiln_jira::Mirror, String>>>,
    /// The running job's answer, until it arrives.
    job: Option<Receiver<Done>>,
    /// The latest `kiln check`'s findings, until they arrive. A newer
    /// check replaces it, so an older one's answer is dropped.
    check: Option<Receiver<Vec<Finding>>>,
}

struct Win {
    /// Wakes the loop when a thread has something.
    proxy: EventLoopProxy<()>,
    opener: Opener,
    projects_file: Option<PathBuf>,
    /// Home's list, kept while a project is shown.
    projects: Projects,
    home_dir: Option<PathBuf>,
    view: View,
    /// A project Home is opening, until it is read.
    opening: Option<Receiver<Result<Opened, String>>>,
    /// Home's summaries, as each project is read.
    summary_tx: Sender<(PathBuf, Result<Summary, String>)>,
    summary_rx: Receiver<(PathBuf, Result<Summary, String>)>,
    settings: Settings,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    surface: Option<Surface>,
    renderer: Option<Renderer>,
    mods: ModifiersState,
    mouse: (f32, f32),
    last_click: Option<(Instant, usize)>,
    /// Why the window couldn't start.
    failed: Option<String>,
}

/// Today in UTC, for the rules that compare dates.
fn today() -> Date {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Date::from_unix_days((secs / 86_400) as i64)
}

/// `kiln check` on the project as it is on disk, with K207 once the Jira
/// mirror is here (and `[check]` turns it on). The thread reads the
/// project itself rather than sharing the window's: a `Project` isn't
/// shared between threads, and a file saved in between is followed by a
/// reload, and so by another check, anyway.
fn findings(root: &std::path::Path, jira: Option<&kiln_jira::Mirror>) -> Vec<Finding> {
    let source = DiskSource::new(root);
    let Ok(project) = Project::load(&source) else {
        // The window's own load says why.
        return Vec::new();
    };
    let project = &project;
    let mut out = kiln_check::check(project, &source, today());
    let severity = project.config.severity(Rule::K207);
    if let Some(mirror) = jira.filter(|_| severity != Severity::Off) {
        for mut f in kiln_jira::k207(project, mirror) {
            f.severity = severity;
            out.push(f);
        }
        out.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    }
    out
}

/// Item files and `kiln.toml` with their stamps: what a change shows in.
fn stamps(root: &std::path::Path, project: &Project) -> BTreeMap<String, Stamp> {
    let disk = DiskSource::new(root);
    let mut out = BTreeMap::new();
    if let Ok(listing) = disk.list(&project.config.paths, &[], Some("md")) {
        for f in listing.files {
            if let Ok(s) = Stamp::of(&disk.path(&f)) {
                out.insert(f, s);
            }
        }
    }
    if let Ok(s) = Stamp::of(&root.join("kiln.toml")) {
        out.insert("kiln.toml".to_owned(), s);
    }
    out
}

/// The window's icon (title bar, Alt-Tab, the X11 taskbar), drawn from
/// `kiln-brand`: the geometry the `kiln` crate's build script embeds in
/// the Windows executables, which Explorer and a taskbar pin read before
/// the process runs, so the two can't disagree. Wayland ignores it; the
/// desktop file's icon is used there. A window without its icon is still
/// a window, so a failure here is no error.
fn kiln_icon() -> Option<Icon> {
    let icon = kiln_brand::app_icon(256);
    Icon::from_rgba(icon.rgba, icon.width, icon.height).ok()
}

impl Session {
    /// A project to show, its check started and its Jira mirror asked for.
    fn new(o: Opened, proxy: &EventLoopProxy<()>) -> Session {
        let app = App::new(o.root.clone(), o.project, o.baseline);
        let stamps = stamps(&o.root, app.project());
        let jira = o.jira.map(|load| {
            let (tx, rx) = std::sync::mpsc::channel();
            let wake = proxy.clone();
            std::thread::spawn(move || {
                let _ = tx.send(load());
                let _ = wake.send_event(());
            });
            rx
        });
        let mut s = Session {
            app,
            root: o.root,
            host: o.host,
            stamps,
            jira,
            job: None,
            check: None,
        };
        s.check(proxy);
        s
    }

    /// Starts the app's next job, if it has one and none is running.
    fn jobs(&mut self, proxy: &EventLoopProxy<()>) {
        if self.job.is_some() {
            return;
        }
        let Some(job) = self.app.next_job() else {
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let host = self.host.clone();
        let wake = proxy.clone();
        std::thread::spawn(move || {
            let _ = tx.send(host.run(&job));
            let _ = wake.send_event(());
        });
        self.job = Some(rx);
    }

    /// Runs `kiln check` on a thread; its findings replace the app's when
    /// they arrive.
    fn check(&mut self, proxy: &EventLoopProxy<()>) {
        let (tx, rx) = std::sync::mpsc::channel();
        let root = self.root.clone();
        let jira = self.app.jira().cloned();
        let wake = proxy.clone();
        std::thread::spawn(move || {
            let _ = tx.send(findings(&root, jira.as_ref()));
            let _ = wake.send_event(());
        });
        self.check = Some(rx);
        self.app.set_checking();
    }

    /// What the threads have sent: the Jira mirror, the findings, a job's
    /// answer. `true` when something arrived.
    fn arrivals(&mut self, proxy: &EventLoopProxy<()>) -> bool {
        let mut any = false;
        if let Some(answer) = self.jira.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.jira = None;
            match answer {
                Ok(mirror) => {
                    self.app.set_jira(mirror);
                    // K207 needs the mirror.
                    self.check(proxy);
                }
                Err(e) => self.app.say(format!("Jira: {e}"), true),
            }
            any = true;
        }
        match self.check.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(found)) => {
                self.check = None;
                self.app.set_findings(found);
                any = true;
            }
            Some(Err(TryRecvError::Disconnected)) => {
                self.check = None;
                self.app.set_findings(Vec::new());
                self.app.say("kiln check stopped without an answer", true);
                any = true;
            }
            Some(Err(TryRecvError::Empty)) | None => {}
        }
        match self.job.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(done)) => {
                self.job = None;
                self.app.done(done);
            }
            // The thread ended without an answer.
            Some(Err(TryRecvError::Disconnected)) => {
                self.job = None;
                self.app
                    .job_failed("the job stopped without an answer".into());
            }
            Some(Err(TryRecvError::Empty)) | None => return any,
        }
        self.jobs(proxy);
        true
    }

    /// Reads the project again; `told` says the user asked.
    fn reload(&mut self, told: bool, proxy: &EventLoopProxy<()>) {
        match Project::load(&DiskSource::new(&self.root)) {
            Ok(project) => {
                self.stamps = stamps(&self.root, &project);
                self.app.reload(project);
                self.check(proxy);
                if told {
                    self.app.say("Reloaded", false);
                }
            }
            Err(e) => self.app.say(format!("couldn't reload: {e}"), true),
        }
    }

    /// Makes an edit, then reloads.
    fn edit(&mut self, edit: crate::edit::Edit, proxy: &EventLoopProxy<()>) {
        match crate::edit::apply(&self.root, &edit) {
            Ok(applied) => {
                self.reload(false, proxy);
                self.app.edited(&applied);
                if let Some((path, line)) = &applied.created {
                    // The item exists whether or not this works.
                    let how = match kiln_fenix::open(path, *line as usize) {
                        Ok(how) => how.describe().to_owned(),
                        Err(e) => format!("couldn't open it: {e}"),
                    };
                    self.app.say(format!("{}; {how}", applied.message), false);
                }
            }
            Err(e) => self.app.say(e, true),
        }
    }
}

impl Win {
    fn start(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let attributes = Window::default_attributes()
            .with_title(self.title())
            .with_inner_size(LogicalSize::new(1100.0, 760.0))
            .with_window_icon(kiln_icon());
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(|e| format!("couldn't open a window: {e}"))?,
        );
        let mut descriptor = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
            event_loop.owned_display_handle(),
        ));
        // DX12 on Windows, as Flint: it starts faster than Vulkan and its
        // fallback (WARP) is on every Windows. `WGPU_BACKEND` overrides.
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let instance = wgpu::Instance::new(descriptor.with_env());
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| format!("couldn't draw in the window: {e}"))?;
        let gpu = Gpu::new(instance, Some(&surface))?;
        let surface = Surface::new(&gpu, window.clone(), Some(surface), None)?;
        let renderer = Renderer::new(&gpu, surface.config.format, self.settings.font.as_deref());
        if gpu.summary.fallback {
            self.say(
                format!(
                    "Drawing with the software adapter ({}): no usable GPU here",
                    gpu.summary.name
                ),
                false,
            );
        }
        self.window = Some(window.clone());
        self.gpu = Some(gpu);
        self.surface = Some(surface);
        self.renderer = Some(renderer);
        self.relayout();
        window.request_redraw();
        Ok(())
    }

    /// "Kiln" on Home, "Kiln — Flint" on a project.
    fn title(&self) -> String {
        match &self.view {
            View::Home(_) => "Kiln".into(),
            View::Project(s) => format!("Kiln — {}", s.app.project().config.name),
        }
    }

    fn say(&mut self, text: String, error: bool) {
        match &mut self.view {
            View::Home(h) => h.say(text, error),
            View::Project(s) => s.app.say(text, error),
        }
    }

    /// Shows `opened`, and puts it first on Home's list.
    fn show_project(&mut self, opened: Opened) {
        self.projects.opened(&opened.root, projects::now());
        self.save_projects();
        self.view = View::Project(Box::new(Session::new(opened, &self.proxy)));
        self.retitle();
        self.relayout();
    }

    /// Shows Home, the cursor on the most recently opened project.
    fn show_home(&mut self) {
        let mut home = Home::new(
            self.projects.clone(),
            today(),
            projects::now(),
            self.home_dir.clone(),
        );
        if let Some(first) = self.projects.list.first() {
            home.select(&first.root.clone());
        }
        self.view = View::Home(Box::new(home));
        self.summaries();
        self.retitle();
        self.relayout();
    }

    fn retitle(&self) {
        if let Some(w) = &self.window {
            w.set_title(&self.title());
        }
    }

    /// Writes Home's list, saying so in the footer when it can't.
    fn save_projects(&mut self) {
        let Some(path) = &self.projects_file else {
            return;
        };
        if let Err(e) = self.projects.save(path) {
            self.say(e, true);
        }
    }

    /// Reads the summaries Home wants, one project after another, on a
    /// thread.
    fn summaries(&mut self) {
        let View::Home(home) = &mut self.view else {
            return;
        };
        let roots = home.wanted();
        if roots.is_empty() {
            return;
        }
        let tx = self.summary_tx.clone();
        let wake = self.proxy.clone();
        std::thread::spawn(move || {
            for root in roots {
                let summary = projects::summarize(&root, today());
                if tx.send((root, summary)).is_err() {
                    return;
                }
                let _ = wake.send_event(());
            }
        });
    }

    fn layout(&self) -> Option<Layout> {
        let window = self.window.as_ref()?;
        let size = window.inner_size();
        Some(layout(
            size.width as f32,
            size.height as f32,
            window.scale_factor() as f32,
            self.settings.font_size,
        ))
    }

    fn relayout(&mut self) {
        let Some(l) = self.layout() else {
            return;
        };
        match &mut self.view {
            View::Project(s) => s.app.set_visible(l.rows),
            View::Home(h) => {
                let cell = self
                    .renderer
                    .as_mut()
                    .map_or(l.line * 0.5, |r| r.cell_width(&l));
                h.set_size(l.rows, l.columns(cell));
            }
        }
    }

    fn redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn effect(&mut self, effect: Effect, event_loop: &ActiveEventLoop) {
        let View::Project(s) = &mut self.view else {
            return;
        };
        match effect {
            Effect::None => {}
            Effect::Quit => event_loop.exit(),
            Effect::Reload => s.reload(true, &self.proxy),
            Effect::Open { path, line } => match kiln_fenix::open(&path, line as usize) {
                Ok(how) => s.app.say(how.describe(), false),
                Err(e) => s.app.say(e, true),
            },
            Effect::OpenUrl(url) => match kiln_fenix::open_url(&url) {
                Ok(()) => s.app.say("Opened in the browser", false),
                Err(e) => s.app.say(e, true),
            },
            Effect::Edit(edit) => s.edit(edit, &self.proxy),
            Effect::Home => {
                self.show_home();
                self.redraw();
                return;
            }
        }
        s.jobs(&self.proxy);
        self.redraw();
    }

    fn home_effect(&mut self, effect: home::Effect, event_loop: &ActiveEventLoop) {
        let View::Home(home) = &mut self.view else {
            return;
        };
        match effect {
            home::Effect::None => {}
            home::Effect::Quit => event_loop.exit(),
            home::Effect::Save => {
                self.projects = home.projects().clone();
                self.save_projects();
                self.summaries();
            }
            home::Effect::Open(root) => {
                let name = root
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| root.display().to_string());
                home.set_opening(Some(name));
                let (tx, rx) = std::sync::mpsc::channel();
                let open = self.opener.clone();
                let wake = self.proxy.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(open(&root));
                    let _ = wake.send_event(());
                });
                self.opening = Some(rx);
            }
        }
        self.redraw();
    }

    /// What the threads have sent.
    fn arrivals(&mut self) {
        let mut any = false;
        while let Ok((root, summary)) = self.summary_rx.try_recv() {
            if let View::Home(h) = &mut self.view {
                h.summarized(&root, summary);
                any = true;
            }
        }
        match self.opening.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(Ok(opened))) => {
                self.opening = None;
                self.show_project(opened);
                any = true;
            }
            Some(Ok(Err(e))) => {
                self.opening = None;
                if let View::Home(h) = &mut self.view {
                    h.set_opening(None);
                    h.say(e, true);
                }
                any = true;
            }
            Some(Err(TryRecvError::Disconnected)) => {
                self.opening = None;
                if let View::Home(h) = &mut self.view {
                    h.set_opening(None);
                    h.say("the project couldn't be opened", true);
                }
                any = true;
            }
            Some(Err(TryRecvError::Empty)) | None => {}
        }
        if let View::Project(s) = &mut self.view {
            any |= s.arrivals(&self.proxy);
        }
        if any {
            self.redraw();
        }
    }

    fn screen(&self) -> Screen {
        match &self.view {
            View::Home(h) => h.screen(),
            View::Project(s) => s.app.screen(),
        }
    }
}

impl ApplicationHandler for Win {
    fn user_event(&mut self, _: &ActiveEventLoop, (): ()) {
        self.arrivals();
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        if let Err(e) = self.start(event_loop) {
            self.failed = Some(e);
            event_loop.exit();
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::ResumeTimeReached { .. } = cause {
            self.arrivals();
            if let View::Project(s) = &mut self.view {
                if stamps(&s.root, s.app.project()) != s.stamps {
                    s.reload(false, &self.proxy);
                    self.redraw();
                }
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + POLL));
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(s) = &mut self.surface {
                    s.resize(size.width, size.height);
                }
                self.relayout();
                self.redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.relayout();
                self.redraw();
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                for press in crate::keys::presses(&event.logical_key, self.mods) {
                    match &mut self.view {
                        View::Project(s) => {
                            let effect = s.app.key(press);
                            self.effect(effect, event_loop);
                        }
                        View::Home(h) => {
                            let effect = h.key(press);
                            self.home_effect(effect, event_loop);
                            self.summaries();
                        }
                    }
                }
                self.redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -(y * 3.0) as isize,
                    MouseScrollDelta::PixelDelta(p) => {
                        let line = self.layout().map_or(20.0, |l| l.line as f64);
                        -(p.y / line) as isize
                    }
                };
                match &mut self.view {
                    View::Project(s) => s.app.wheel(lines),
                    View::Home(h) => h.wheel(lines),
                }
                self.redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse = (position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let Some(l) = self.layout() else {
                    return;
                };
                if let Some(row) = l.row_at(self.mouse.1) {
                    let now = Instant::now();
                    let double = self
                        .last_click
                        .is_some_and(|(at, r)| r == row && now.duration_since(at) < DOUBLE_CLICK);
                    self.last_click = Some((now, row));
                    match &mut self.view {
                        View::Project(s) => {
                            let effect = s.app.click(row, double);
                            self.effect(effect, event_loop);
                        }
                        View::Home(h) => {
                            let cell = self
                                .renderer
                                .as_mut()
                                .map_or(l.line * 0.5, |r| r.cell_width(&l));
                            let effect = h.click(row, l.column_at(self.mouse.0, cell), double);
                            self.home_effect(effect, event_loop);
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let Some(l) = self.layout() else {
                    return;
                };
                let screen = self.screen();
                if let (Some(gpu), Some(surface), Some(renderer)) =
                    (&self.gpu, &mut self.surface, &mut self.renderer)
                {
                    if !renderer.draw(gpu, surface, &screen, &l, self.settings.palette) {
                        self.redraw();
                    }
                }
            }
            _ => {}
        }
    }
}
