//! `kiln gui` (and `kiln` alone): the window (docs/GUI.md). Loads the
//! project, the baseline for the Changes page (`--since REV`, else the
//! most recent tag) and the user's `settings.toml`, then hands over, with
//! a [`kiln_gui::Host`] for what the window can't do itself: exports,
//! Confluence and git. The host runs the commands' own code
//! (`export::run_with`, `confluence::read`), so the window shows what the
//! terminal would print and asks Confluence nothing the command line
//! wouldn't.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use kiln_gui::{Comment, CommentGroup, Done, ExportReport, Job};
use kiln_model::ExportFormat;

use super::export::{self, Answer};
use crate::project::{at_revision, folders, load};
use crate::{Fail, TextOrJson, EXIT_USAGE};

pub fn run(
    project: Option<&Path>,
    since: Option<String>,
    item: Option<String>,
) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let root = loaded.source.root().to_path_buf();
    let select = item.as_deref().map(super::parse_id).transpose()?;
    let baseline = since
        .or_else(|| kiln_git::latest_tag(&root))
        .map(|rev| baseline(&root, &rev))
        .transpose()?;
    let (settings, notes) = folders()
        .map(|f| f.settings.join("settings.toml"))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|text| kiln_gui::parse_settings(&text))
        .unwrap_or_default();
    // The Jira mirror loads beside the window, so a slow Jira never holds
    // it up; a project without `[jira] url` or keys asks for nothing.
    let keys = kiln_jira::keys(&loaded.project);
    let jira: Option<kiln_gui::JiraLoad> = match loaded.project.config.jira_url.clone() {
        Some(url) if !keys.is_empty() => {
            Some(Box::new(move || super::jira::mirror_for(&url, &keys)))
        }
        _ => None,
    };
    kiln_gui::run(kiln_gui::Options {
        host: Arc::new(GuiHost { root: root.clone() }),
        root,
        jira,
        project: loaded.project,
        baseline,
        settings,
        notes,
        select,
    })
    .map(|()| 0)
    .map_err(|e| Fail::new(EXIT_USAGE, format!("gui: {e}")))
}

/// The project at `rev`, labelled `v1.0 (1a2b3c4)`.
fn baseline(root: &Path, rev: &str) -> Result<kiln_gui::Baseline, Fail> {
    let (old, commit) = at_revision(root, rev)?;
    let short = &commit[..commit.len().min(7)];
    Ok(kiln_gui::Baseline {
        label: if rev.starts_with(short) {
            short.to_owned()
        } else {
            format!("{rev} ({short})")
        },
        project: old,
    })
}

struct GuiHost {
    root: PathBuf,
}

impl kiln_gui::Host for GuiHost {
    fn run(&self, job: &Job) -> Done {
        let root = self.root.as_path();
        match job {
            Job::PlanExports => Done::Exports(plan_exports(root)),
            Job::WriteExports(names) => Done::Wrote(
                exporting(root, names.clone(), false, &mut |_, _, _| Answer::CantAsk)
                    .map(|(lines, _)| lines),
            ),
            Job::Publish { target, shown } if target == "jira" => {
                Done::Published(jira_sync(root, shown))
            }
            Job::Publish { target, shown } => Done::Published(publish(root, target, shown)),
            Job::JiraPull(id) => Done::Pulled(jira_pull(root, id)),
            Job::Comments => Done::Comments(comments(root)),
            Job::Tags => Done::Tags(kiln_git::tags(root).map_err(|e| e.to_string())),
            Job::Baseline(rev) => {
                Done::Baseline(baseline(root, rev).map(Box::new).map_err(|f| f.message))
            }
        }
    }
}

/// `kiln export NAMES` (`--dry-run` when `dry`), its lines and exit code.
fn exporting(
    root: &Path,
    names: Vec<String>,
    dry: bool,
    confirm: export::Confirm,
) -> Result<(Vec<String>, u8), String> {
    let options = export::Options {
        names,
        dry_run: dry,
        check: false,
        require_clean: false,
        allow_shallow: false,
        snapshot: None,
        save_snapshot: None,
        yes: false,
        force: false,
        adopt: false,
        prune_removed: None,
        since: None,
        format: TextOrJson::Text,
    };
    let mut out: Vec<u8> = Vec::new();
    let code = export::run_with(Some(root), options, &mut out, confirm).map_err(|f| f.message)?;
    let lines = String::from_utf8_lossy(&out)
        .lines()
        .map(str::to_owned)
        .collect();
    Ok((lines, code))
}

/// Every target's dry run, one at a time, so a Confluence target without
/// a token doesn't hide the file targets' plans.
fn plan_exports(root: &Path) -> Vec<ExportReport> {
    let (targets, jira) = match load(Some(root)) {
        Ok(l) => (
            l.project.config.exports.clone(),
            l.project.config.jira_sync.is_some(),
        ),
        Err(f) => {
            return vec![ExportReport {
                name: "kiln.toml".into(),
                confluence: false,
                jira: false,
                lines: Vec::new(),
                error: Some(f.message),
            }]
        }
    };
    let mut reports: Vec<ExportReport> = targets
        .into_iter()
        .map(|t| {
            let confluence = t.format == ExportFormat::Confluence;
            match exporting(root, vec![t.name.clone()], true, &mut |_, _, _| {
                Answer::CantAsk
            }) {
                Ok((lines, _)) => ExportReport {
                    name: t.name,
                    confluence,
                    jira: false,
                    lines,
                    error: None,
                },
                Err(e) => ExportReport {
                    lines: vec![format!(
                        "{} ({}, {})",
                        t.name,
                        t.format.as_str(),
                        t.layout.as_str()
                    )],
                    name: t.name,
                    confluence,
                    jira: false,
                    error: Some(e),
                },
            }
        })
        .collect();
    if jira {
        reports.push(jira_plan(root));
    }
    reports
}

/// `kiln jira sync --dry-run`, as one more target on the Exports page.
fn jira_plan(root: &Path) -> ExportReport {
    let mut out: Vec<u8> = Vec::new();
    let result = super::jira::sync_with(Some(root), true, false, &mut out, &mut |_, _| {
        Answer::CantAsk
    });
    let mut lines: Vec<String> = String::from_utf8_lossy(&out)
        .lines()
        .map(str::to_owned)
        .collect();
    if lines.is_empty() {
        lines.push("jira (kiln jira sync)".into());
    }
    ExportReport {
        name: "jira".into(),
        confluence: false,
        jira: true,
        lines,
        error: result.err().map(|f| f.message),
    }
}

/// Syncs Jira, but only the plan that was shown.
fn jira_sync(root: &Path, shown: &str) -> Result<Vec<String>, String> {
    let mut moved = false;
    let mut out: Vec<u8> = Vec::new();
    super::jira::sync_with(Some(root), false, false, &mut out, &mut |_, plan| {
        if plan == shown {
            Answer::Yes
        } else {
            moved = true;
            Answer::No
        }
    })
    .map_err(|f| f.message)?;
    if moved {
        return Err(
            "Jira not synced: its plan changed since it was shown; look at the new plan and press P again"
                .into(),
        );
    }
    Ok(String::from_utf8_lossy(&out)
        .lines()
        .map(str::to_owned)
        .collect())
}

/// `kiln jira pull ID`: its lines, or its first warning as the error.
fn jira_pull(root: &Path, id: &str) -> Result<Vec<String>, String> {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = super::jira::pull_with(Some(root), &[id.to_owned()], &mut out, &mut err)
        .map_err(|f| f.message)?;
    let err = String::from_utf8_lossy(&err).trim().to_owned();
    if code != 0 && !err.is_empty() {
        return Err(err
            .lines()
            .next()
            .unwrap_or("")
            .trim_start_matches("kiln: ")
            .to_owned());
    }
    Ok(String::from_utf8_lossy(&out)
        .lines()
        .map(str::to_owned)
        .collect())
}

/// Publishes `target`, but only the plan that was shown: when the space
/// or the files moved on since, nothing is published and the GUI plans
/// again.
fn publish(root: &Path, target: &str, shown: &str) -> Result<Vec<String>, String> {
    let mut moved = false;
    let (lines, _) = exporting(root, vec![target.to_owned()], false, &mut |_, _, plan| {
        if plan == shown {
            Answer::Yes
        } else {
            moved = true;
            Answer::No
        }
    })?;
    if moved {
        return Err(format!(
            "`{target}` not published: its plan changed since it was shown; look at the new plan and press P again"
        ));
    }
    if !lines.iter().any(|l| l.contains(": published ")) {
        return Ok(vec![format!("`{target}`: nothing to publish")]);
    }
    Ok(lines)
}

fn comments(root: &Path) -> Result<Vec<CommentGroup>, String> {
    let targets = super::confluence::read(Some(root), Vec::new()).map_err(|f| f.message)?;
    Ok(targets
        .into_iter()
        .map(|t| CommentGroup {
            target: t.name,
            space: t.space,
            published: t.published,
            comments: t
                .comments
                .into_iter()
                .map(|c| Comment {
                    date: c.created.get(..10).unwrap_or(&c.created).to_owned(),
                    item: c.item,
                    page_title: c.page_title,
                    author: c.author,
                    text: c.text,
                    url: c.url,
                })
                .collect(),
        })
        .collect())
}
