//! The window: winit's event loop, the GPU, and the glue between them and
//! [`App`]. Keys become [`KeyPress`]es for the app; its effects open
//! files in Fenix or addresses in the browser, make edits, reload or
//! quit; the project is re-read when its item files or `kiln.toml`
//! change, looked at every two seconds. `kiln check` runs on a thread of
//! its own after each load, since on a large project it takes seconds the
//! window shouldn't freeze for. The app's jobs run one at a time on a
//! thread through the binary's [`Host`]. Threads wake the loop when they
//! are done.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use kiln_index::{DiskSource, FileSource, Project, Stamp};
use kiln_model::{Date, Finding, ItemId, Rule, Severity};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

use crate::app::{App, Baseline, Effect};
use crate::gpu::{Gpu, Surface};
use crate::host::{Done, Host};
use crate::render::{layout, Layout, Renderer};
use crate::settings::Settings;

/// How often the project is checked for changes on disk.
const POLL: Duration = Duration::from_secs(2);
/// Two clicks on a row this close together open it.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// What the window starts with.
pub struct Options {
    pub root: PathBuf,
    pub project: Project,
    pub baseline: Option<Baseline>,
    pub settings: Settings,
    /// Problems with the settings, shown in the footer.
    pub notes: Vec<String>,
    /// `--item`: the item to put the cursor on.
    pub select: Option<ItemId>,
    /// Reads the Jira mirror; run on a thread of its own.
    pub jira: Option<JiraLoad>,
    /// Exports, Confluence and git, for the app's jobs.
    pub host: Arc<dyn Host>,
}

/// How the window gets the Jira mirror: the binary's, which knows where
/// tokens and the cache live.
pub type JiraLoad = Box<dyn FnOnce() -> Result<kiln_jira::Mirror, String> + Send>;

/// Opens the window and runs until it is closed.
pub fn run(o: Options) -> Result<(), String> {
    let event_loop = event_loop()?;
    event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + POLL));
    let proxy = event_loop.create_proxy();
    let mut app = App::new(o.root.clone(), o.project, o.baseline);
    if let Some(id) = &o.select {
        if !app.select(id) {
            app.say(format!("{id} isn't in the project"), true);
        }
    }
    if let Some(first) = o.notes.first() {
        app.say(first.clone(), true);
    }
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
    let mut win = Win {
        jira,
        host: o.host,
        proxy,
        job: None,
        check: None,
        app,
        root: o.root,
        settings: o.settings,
        stamps,
        window: None,
        gpu: None,
        surface: None,
        renderer: None,
        mods: ModifiersState::empty(),
        mouse_y: 0.0,
        last_click: None,
        failed: None,
    };
    win.check();
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

struct Win {
    /// The Jira mirror, until it arrives.
    jira: Option<std::sync::mpsc::Receiver<Result<kiln_jira::Mirror, String>>>,
    host: Arc<dyn Host>,
    /// Wakes the loop when a thread has something.
    proxy: EventLoopProxy<()>,
    /// The running job's answer, until it arrives.
    job: Option<std::sync::mpsc::Receiver<Done>>,
    /// The latest `kiln check`'s findings, until they arrive. A newer
    /// check replaces it, so an older one's answer is dropped.
    check: Option<std::sync::mpsc::Receiver<Vec<Finding>>>,
    app: App,
    root: PathBuf,
    settings: Settings,
    stamps: BTreeMap<String, Stamp>,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    surface: Option<Surface>,
    renderer: Option<Renderer>,
    mods: ModifiersState,
    mouse_y: f32,
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

impl Win {
    fn start(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let attributes = Window::default_attributes()
            .with_title(format!("Kiln — {}", self.app.project().config.name))
            .with_inner_size(LogicalSize::new(1100.0, 760.0));
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
            self.app.say(
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
        if let Some(l) = self.layout() {
            self.app.set_visible(l.rows);
        }
    }

    fn redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn effect(&mut self, effect: Effect, event_loop: &ActiveEventLoop) {
        match effect {
            Effect::None => {}
            Effect::Quit => event_loop.exit(),
            Effect::Reload => self.reload(true),
            Effect::Open { path, line } => match kiln_fenix::open(&path, line as usize) {
                Ok(how) => self.app.say(how.describe(), false),
                Err(e) => self.app.say(e, true),
            },
            Effect::OpenUrl(url) => match kiln_fenix::open_url(&url) {
                Ok(()) => self.app.say("Opened in the browser", false),
                Err(e) => self.app.say(e, true),
            },
            Effect::Edit(edit) => match crate::edit::apply(&self.root, &edit) {
                Ok(applied) => {
                    self.reload(false);
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
            },
        }
        self.jobs();
        self.redraw();
    }

    /// Starts the app's next job, if it has one and none is running.
    fn jobs(&mut self) {
        if self.job.is_some() {
            return;
        }
        let Some(job) = self.app.next_job() else {
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let host = self.host.clone();
        let wake = self.proxy.clone();
        std::thread::spawn(move || {
            let _ = tx.send(host.run(&job));
            let _ = wake.send_event(());
        });
        self.job = Some(rx);
    }

    /// Runs `kiln check` on a thread; its findings replace the app's when
    /// they arrive.
    fn check(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        let root = self.root.clone();
        let jira = self.app.jira().cloned();
        let wake = self.proxy.clone();
        std::thread::spawn(move || {
            let _ = tx.send(findings(&root, jira.as_ref()));
            let _ = wake.send_event(());
        });
        self.check = Some(rx);
        self.app.set_checking();
    }

    /// What the threads have sent: the Jira mirror, the findings, a job's
    /// answer.
    fn arrivals(&mut self) {
        if let Some(answer) = self.jira.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.jira = None;
            match answer {
                Ok(mirror) => {
                    self.app.set_jira(mirror);
                    // K207 needs the mirror.
                    self.check();
                }
                Err(e) => self.app.say(format!("Jira: {e}"), true),
            }
            self.redraw();
        }
        match self.check.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(found)) => {
                self.check = None;
                self.app.set_findings(found);
                self.redraw();
            }
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                self.check = None;
                self.app.set_findings(Vec::new());
                self.app.say("kiln check stopped without an answer", true);
                self.redraw();
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) | None => {}
        }
        let answer = self.job.as_ref().map(|rx| rx.try_recv());
        match answer {
            Some(Ok(done)) => {
                self.job = None;
                self.app.done(done);
            }
            // The thread ended without an answer.
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                self.job = None;
                self.app
                    .job_failed("the job stopped without an answer".into());
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) | None => return,
        }
        self.jobs();
        self.redraw();
    }

    /// Reads the project again; `told` says the user asked.
    fn reload(&mut self, told: bool) {
        match Project::load(&DiskSource::new(&self.root)) {
            Ok(project) => {
                self.stamps = stamps(&self.root, &project);
                self.app.reload(project);
                self.check();
                if told {
                    self.app.say("Reloaded", false);
                }
            }
            Err(e) => self.app.say(format!("couldn't reload: {e}"), true),
        }
        self.redraw();
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
            if stamps(&self.root, self.app.project()) != self.stamps {
                self.reload(false);
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
                    let effect = self.app.key(press);
                    self.effect(effect, event_loop);
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
                self.app.wheel(lines);
                self.redraw();
            }
            WindowEvent::CursorMoved { position, .. } => self.mouse_y = position.y as f32,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(row) = self.layout().and_then(|l| l.row_at(self.mouse_y)) {
                    let now = Instant::now();
                    let double = self
                        .last_click
                        .is_some_and(|(at, r)| r == row && now.duration_since(at) < DOUBLE_CLICK);
                    self.last_click = Some((now, row));
                    let effect = self.app.click(row, double);
                    self.effect(effect, event_loop);
                }
            }
            WindowEvent::RedrawRequested => {
                let Some(l) = self.layout() else {
                    return;
                };
                let screen = self.app.screen();
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
