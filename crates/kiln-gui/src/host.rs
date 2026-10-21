//! What the GUI asks of the binary: the work that needs git, the export
//! pipeline, Confluence or a token, none of which `kiln-gui` depends on.
//! The window runs each [`Job`] on a thread of its own and hands the
//! [`Done`] back to the application, so a slow Confluence never holds up
//! the window and the GUI never sees a token (GUI.md, "How it is built").
//!
//! Exports are described by the command line's own lines (`kiln export
//! --dry-run`), so the Exports page shows exactly what the terminal would.

use crate::app::Baseline;

/// A piece of work for the binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    /// `kiln export --dry-run`, one target at a time.
    PlanExports,
    /// `kiln export NAME…` for these file targets.
    WriteExports(Vec<String>),
    /// Publishes a Confluence target (or syncs Jira, target `jira`), if
    /// its plan is still `shown`.
    Publish { target: String, shown: String },
    /// `kiln jira pull ID`.
    JiraPull(String),
    /// `kiln confluence comments`.
    Comments,
    /// The tags, newest first, for picking a baseline.
    Tags,
    /// The project at a revision, for the Changes page.
    Baseline(String),
}

impl Job {
    /// For the footer while it runs.
    pub fn describe(&self) -> String {
        match self {
            Job::PlanExports => "Planning the exports…".into(),
            Job::WriteExports(names) => format!("Writing {}…", names.join(", ")),
            Job::Publish { target, .. } if target == "jira" => "Syncing Jira…".into(),
            Job::Publish { target, .. } => format!("Publishing {target}…"),
            Job::JiraPull(id) => format!("Taking {id}'s status from Jira…"),
            Job::Comments => "Reading Confluence comments…".into(),
            Job::Tags => "Reading tags…".into(),
            Job::Baseline(rev) => format!("Reading the project at {rev}…"),
        }
    }
}

/// What a job came to.
pub enum Done {
    Exports(Vec<ExportReport>),
    /// The lines `kiln export` printed, or why it failed.
    Wrote(Result<Vec<String>, String>),
    Published(Result<Vec<String>, String>),
    /// What `kiln jira pull` printed.
    Pulled(Result<Vec<String>, String>),
    Comments(Result<Vec<CommentGroup>, String>),
    Tags(Result<Vec<String>, String>),
    Baseline(Result<Box<Baseline>, String>),
}

/// One export target's dry run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReport {
    pub name: String,
    pub confluence: bool,
    /// The Jira sync (`kiln jira sync`), shown as one more target.
    pub jira: bool,
    /// As `kiln export --dry-run` prints them: the target's line first.
    pub lines: Vec<String>,
    /// Why it couldn't be planned (no token, a server error…).
    pub error: Option<String>,
}

impl ExportReport {
    /// The plan as printed, which a publish is confirmed against.
    pub fn plan(&self) -> String {
        self.lines.iter().map(|l| format!("{l}\n")).collect()
    }
}

/// One Confluence target's unresolved comments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentGroup {
    pub target: String,
    pub space: String,
    pub published: bool,
    pub comments: Vec<Comment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    /// The item it is about, when Kiln can tell.
    pub item: Option<String>,
    pub page_title: String,
    pub author: String,
    /// `2026-10-12`.
    pub date: String,
    pub text: String,
    pub url: String,
}

/// The binary's side: runs a job to completion (on the window's worker
/// thread).
pub trait Host: Send + Sync {
    fn run(&self, job: &Job) -> Done;
}
