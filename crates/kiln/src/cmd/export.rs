//! `kiln export`: renders every (or the named) export target and writes
//! file targets whose bytes changed; `--dry-run` shows what would change,
//! `--check` also exits 1 if anything would. A Confluence target reads the
//! space (or a snapshot file), prints the plan, and without `--dry-run`
//! publishes it: after `--yes`, or a "y" at an interactive prompt.
//!
//! A non-interactive run without `--yes` stops after the plan and exits 1,
//! so a CI job that forgot `--yes` is red rather than green and silent.

use std::collections::{HashMap, HashSet};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use kiln_confluence::{
    desired_pages, plan, take_snapshot, titles, Applied, ApplyOptions, Attachment,
    AttachmentSource, Confluence, DesiredContext, DesiredPage, OpKind, PageCache, Plan,
    PlanOptions, Snapshot,
};
use kiln_export::{
    plan_files, prepare, render_files, resolve_all, Assembly, FenixPictures, FilePlan, Pictures,
    Prepared, RepoLinks, Revision,
};
use kiln_index::FileSource;
use kiln_model::{ExportFormat, ExportTarget, Severity};
use serde_json::{json, Value};

use super::json_out;
use crate::project::{load, today};
use crate::{Fail, TextOrJson, EXIT_FINDINGS, EXIT_NETWORK};

pub struct Options {
    pub names: Vec<String>,
    pub dry_run: bool,
    pub check: bool,
    pub require_clean: bool,
    pub allow_shallow: bool,
    pub snapshot: Option<String>,
    pub save_snapshot: Option<String>,
    /// Publish without asking.
    pub yes: bool,
    /// Overwrite pages edited in Confluence.
    pub force: bool,
    /// Publish into a `page =` that already has content.
    pub adopt: bool,
    /// Trash retired pages older than this many days.
    pub prune_removed: Option<u32>,
    /// A baseline: the changes report and New/Changed markers.
    pub since: Option<String>,
    pub format: TextOrJson,
}

/// A snapshot older than this is read again before publishing
/// (CONFLUENCE.md, "Apply").
const FRESH: Duration = Duration::from_secs(120);

/// What exporting one target came to.
enum Outcome {
    Files(FilePlan),
    Confluence(Box<ConfluenceRun>),
}

/// A Confluence target's plan and what publishing it needs.
struct ConfluenceRun {
    plan: Plan,
    space: String,
    anchor: u64,
    desired: Vec<DesiredPage>,
    snapshot: Snapshot,
    options: PlanOptions,
    /// `None` when planning from `--snapshot`.
    client: Option<Box<dyn Confluence>>,
    /// The page cache file; `None` without a state folder.
    cache: Option<PathBuf>,
    read_at: Instant,
    restrict_edits: bool,
    /// What publishing did, once it has.
    applied: Option<Applied>,
}

pub fn run(project: Option<&Path>, o: Options) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let root = loaded.source.root().to_path_buf();
    let config = &p.config;

    let mut selected: Vec<&ExportTarget> = Vec::new();
    for name in &o.names {
        let target = config.export(name).ok_or_else(|| {
            let names: Vec<&str> = config.exports.iter().map(|e| e.name.as_str()).collect();
            Fail::usage(format!(
                "no export target `{name}` ({})",
                if names.is_empty() {
                    "none declared".to_owned()
                } else {
                    names.join(", ")
                }
            ))
        })?;
        selected.push(target);
    }
    if o.names.is_empty() {
        selected = config.exports.iter().collect();
    }
    if selected.is_empty() {
        return Err(Fail::usage(
            "kiln.toml declares no [[export]] targets (docs/EXPORT.md)",
        ));
    }

    // A target or template `kiln check` calls broken doesn't export.
    let findings = kiln_check::check(p, &loaded.source, today());
    let blocking: Vec<_> = findings
        .iter()
        .filter(|f| f.severity == Severity::Error && matches!(f.rule.code(), "K005" | "K112"))
        .collect();
    if let Some(first) = blocking.first() {
        return Err(Fail::usage(format!(
            "{first} (`kiln check` lists every problem)"
        )));
    }
    let publishing = !(o.dry_run || o.check);
    if selected
        .iter()
        .any(|t| t.format == ExportFormat::Confluence)
        && publishing
        && o.snapshot.is_some()
    {
        return Err(Fail::usage(
            "--snapshot plans offline; publishing reads the live space (add --dry-run)",
        ));
    }

    let commit = kiln_git::head_commit(&root).ok();
    if o.require_clean && kiln_git::is_dirty(&root).unwrap_or(false) {
        return Err(Fail::usage(
            "the working tree has uncommitted changes (--require-clean)",
        ));
    }
    let date = commit
        .as_deref()
        .and_then(|c| kiln_git::commit_date(&root, c).ok());
    let since = match &o.since {
        Some(rev) => {
            let (old, base) = crate::project::at_revision(&root, rev)?;
            Some(kiln_render::Since {
                label: rev.clone(),
                commit: base,
                changes: kiln_index::diff(&old, p),
            })
        }
        None => None,
    };

    // Targets link into their `link_to` targets, so those are prepared too.
    let mut wanted: Vec<&ExportTarget> = selected.clone();
    for t in &selected {
        for name in &t.link_to {
            if let Some(other) = config.export(name) {
                if !wanted.iter().any(|w| w.name == other.name) {
                    wanted.push(other);
                }
            }
        }
    }
    let in_tree = |t: &ExportTarget| {
        t.format.is_file()
            && t.path
                .as_deref()
                .is_some_and(|path| !kiln_git::is_ignored(&root, path).unwrap_or(false))
    };
    let mut prepared: Vec<Prepared> = Vec::new();
    for t in &wanted {
        let template = match &t.template {
            Some(path) => {
                let bytes = loaded
                    .source
                    .read(path)
                    .map_err(|e| Fail::usage(format!("{path}: {e}")))?;
                Some(
                    kiln_parse::normalize(&bytes)
                        .ok_or_else(|| Fail::usage(format!("{path} isn't UTF-8")))?,
                )
            }
            None => None,
        };
        let revision = Revision {
            commit: commit.clone(),
            date: date.clone(),
            in_tree: in_tree(t),
        };
        let assembly = Assembly {
            project: p,
            findings: &findings,
            revision: &revision,
            since: since.as_ref(),
        };
        prepared.push(prepare(t, template.as_deref(), &assembly));
    }
    let all = prepared.clone();
    resolve_all(&mut prepared, &all, p);

    // Each file's last commit, for links that name commits.
    let needs_commits = selected.iter().any(|t| !in_tree(t)) && commit.is_some();
    let last_commits: HashMap<String, String> = if needs_commits {
        if kiln_git::is_shallow(&root).unwrap_or(false) && !o.allow_shallow {
            return Err(Fail::usage(
                "the clone is shallow: fetch full history (GitLab: GIT_DEPTH: 0; GitHub: fetch-depth: 0), or pass --allow-shallow",
            ));
        }
        kiln_git::last_commits(&root).unwrap_or_default()
    } else {
        HashMap::new()
    };
    let web = |in_tree: bool| match (&config.repository_web, &commit) {
        _ if in_tree => RepoLinks::Relative,
        (Some(template), Some(c)) => RepoLinks::Web {
            template: template.clone(),
            commits: last_commits.clone(),
            fallback: c.clone(),
        },
        _ => RepoLinks::None,
    };
    let pictures = FenixPictures {
        cache: crate::project::folders().map(|f| f.diagram_cache()),
    };

    let mut results: Vec<(&Prepared, Outcome)> = Vec::new();
    for t in &selected {
        let Some(prep) = prepared.iter().find(|x| x.target.name == t.name) else {
            continue;
        };
        let outcome = if t.format.is_file() {
            let files = render_files(
                prep,
                &prepared,
                &web(in_tree(t)),
                config.jira_url.as_deref(),
                &pictures,
                &loaded.source,
            );
            Outcome::Files(plan_files(t, files, &loaded.source))
        } else {
            let planning = Planning {
                all: &prepared,
                project: p,
                source: &loaded.source,
                o: &o,
                revision: &Revision {
                    commit: commit.clone(),
                    date: date.clone(),
                    in_tree: false,
                },
                last_commits: &last_commits,
                repo: &web(false),
            };
            Outcome::Confluence(Box::new(confluence_plan(prep, &planning)?))
        };
        results.push((prep, outcome));
    }

    let mut changed = false;
    let mut refused = false;
    // JSON output can't ask, so it publishes (with --yes) before printing.
    let mut stopped = false;
    if publishing && o.format == TextOrJson::Json {
        for (prep, outcome) in &mut results {
            if let Outcome::Confluence(run) = outcome {
                stopped |= publish(
                    prep,
                    run,
                    &o,
                    &pictures,
                    &loaded.source,
                    commit.as_deref(),
                    false,
                )?;
            }
        }
    }
    if publishing {
        for (_, outcome) in &results {
            if let Outcome::Files(plan) = outcome {
                kiln_export::apply(&root, plan)
                    .map_err(|e| Fail::usage(format!("writing the export: {e}")))?;
            }
        }
    }
    let would = o.dry_run || o.check;
    let mut json_targets = Vec::new();
    for (prep, outcome) in &results {
        let t = &prep.target;
        let left_out: Vec<String> = prep.left_out.iter().map(ToString::to_string).collect();
        match outcome {
            Outcome::Files(plan) => {
                changed |= plan.changes();
                if o.format == TextOrJson::Json {
                    json_targets.push(json!({
                        "name": t.name, "format": t.format.as_str(), "layout": t.layout.as_str(),
                        "write": plan.writes.iter().map(|(p, _, new)| json!({ "path": p, "new": new })).collect::<Vec<_>>(),
                        "unchanged": plan.unchanged, "remove": plan.removes, "leftOut": left_out,
                    }));
                    continue;
                }
                println!(
                    "{} ({}, {}) → {}",
                    t.name,
                    t.format.as_str(),
                    t.layout.as_str(),
                    t.path.as_deref().unwrap_or("")
                );
                let verb = |v: &str| {
                    if would {
                        format!("would {v}")
                    } else {
                        v.to_owned()
                    }
                };
                for (path, _, new) in &plan.writes {
                    println!("  {} {path}", verb(if *new { "create" } else { "update" }));
                }
                for path in &plan.removes {
                    println!("  {} {path} (no longer produced)", verb("remove"));
                }
                if !plan.changes() {
                    println!(
                        "  unchanged ({} file{})",
                        plan.unchanged.len(),
                        if plan.unchanged.len() == 1 { "" } else { "s" }
                    );
                }
                if !left_out.is_empty() {
                    println!("  leaves out: {}", left_out.join(", "));
                }
            }
            Outcome::Confluence(run) => {
                let ConfluenceRun {
                    plan,
                    space,
                    anchor,
                    applied,
                    ..
                } = run.as_ref();
                changed |= plan.changes();
                refused |= !plan.refusals.is_empty();
                if o.format == TextOrJson::Json {
                    json_targets.push(json!({
                        "name": t.name, "format": "confluence", "layout": t.layout.as_str(), "space": space,
                        "parent": anchor, "plan": plan, "leftOut": left_out,
                        "applied": applied.as_ref().map(applied_json),
                    }));
                    continue;
                }
                println!(
                    "{} (confluence, {}) → space {space}, page {anchor}",
                    t.name,
                    t.layout.as_str()
                );
                for op in &plan.ops {
                    let kind = match op.kind {
                        OpKind::Create => "create",
                        OpKind::Update => "update",
                        OpKind::Unchanged => "unchanged",
                        OpKind::Drift => "drift",
                        OpKind::Retire => "retire",
                        OpKind::Prune => "prune",
                    };
                    let reasons = if op.reasons.is_empty() {
                        String::new()
                    } else {
                        format!(": {}", op.reasons.join("; "))
                    };
                    let size = if op.size > 0 {
                        format!(" ({} KiB)", op.size.div_ceil(1024))
                    } else {
                        String::new()
                    };
                    println!("  {kind:9} \"{}\"{reasons}{size}", op.title);
                }
                for title in &plan.foreign {
                    println!("  foreign   \"{title}\" (not Kiln's; left alone)");
                }
                println!(
                    "  {} create, {} update, {} unchanged, {} drift, {} retire{}",
                    plan.count(OpKind::Create),
                    plan.count(OpKind::Update),
                    plan.count(OpKind::Unchanged),
                    plan.count(OpKind::Drift),
                    plan.count(OpKind::Retire),
                    match plan.count(OpKind::Prune) {
                        0 => String::new(),
                        n => format!(", {n} prune"),
                    }
                );
                for w in &plan.warnings {
                    println!("  warning: {w}");
                }
                for r in &plan.refusals {
                    println!("  refused: {r}");
                }
                if !left_out.is_empty() {
                    println!("  leaves out: {}", left_out.join(", "));
                }
            }
        }
    }
    if o.format == TextOrJson::Json {
        print!("{}", json_out(&Value::Array(json_targets)));
    } else if publishing {
        for (prep, outcome) in &mut results {
            if let Outcome::Confluence(run) = outcome {
                stopped |= publish(
                    prep,
                    run,
                    &o,
                    &pictures,
                    &loaded.source,
                    commit.as_deref(),
                    true,
                )?;
            }
        }
    }
    if stopped {
        return Ok(EXIT_FINDINGS);
    }
    if o.check && changed {
        eprintln!("kiln: exports are out of date; run `kiln export` and commit the result");
        return Ok(EXIT_FINDINGS);
    }
    if refused {
        return Ok(EXIT_FINDINGS);
    }
    Ok(0)
}

/// What planning a Confluence target needs besides the target.
struct Planning<'a> {
    all: &'a [Prepared],
    project: &'a kiln_index::Project,
    source: &'a kiln_index::DiskSource,
    o: &'a Options,
    revision: &'a Revision,
    last_commits: &'a HashMap<String, String>,
    repo: &'a RepoLinks,
}

fn confluence_plan(prep: &Prepared, ctx: &Planning) -> Result<ConfluenceRun, Fail> {
    let t = &prep.target;
    let (project, o) = (ctx.project, ctx.o);
    let config = project
        .config
        .confluence
        .as_ref()
        .ok_or_else(|| Fail::usage(format!("export `{}` needs [confluence]", t.name)))?;
    let space = t
        .confluence
        .space
        .clone()
        .unwrap_or_else(|| config.space.clone());
    let anchor = t.confluence.page.or(t.confluence.parent).unwrap_or(0);
    let wanted: Vec<String> = titles(prep, &HashSet::new()).into_values().collect();
    let cache = crate::project::folders().map(|f| {
        f.state
            .join("confluence")
            .join(kiln_confluence::cache_file_name(
                &space,
                &project.config.name,
                &t.name,
            ))
    });
    let read_at = Instant::now();
    let (snapshot, client): (Snapshot, Option<Box<dyn Confluence>>) = match &o.snapshot {
        Some(path) => {
            let text =
                std::fs::read_to_string(path).map_err(|e| Fail::usage(format!("{path}: {e}")))?;
            let snapshot = serde_json::from_str(&text)
                .map_err(|e| Fail::usage(format!("{path}: not a Kiln snapshot: {e}")))?;
            (snapshot, None)
        }
        None => {
            let (token, _) = kiln_secrets::confluence_token_for(&config.url)
                .map_err(Fail::usage)?
                .ok_or_else(|| {
                    Fail::usage(format!(
                        "no token for {}: run `kiln auth confluence`, or set {}{}",
                        config.url,
                        kiln_secrets::CONFLUENCE_TOKEN,
                        if o.dry_run || o.check {
                            " (or plan offline with --snapshot FILE)"
                        } else {
                            ""
                        }
                    ))
                })?;
            let user = kiln_secrets::confluence_user().unwrap_or_else(|| config.user.clone());
            let client = kiln_confluence::connect(config, &token, &user);
            let known = cache
                .as_deref()
                .and_then(|path| std::fs::read_to_string(path).ok())
                .map(|text| PageCache::parse(&text))
                .unwrap_or_default();
            let snapshot = take_snapshot(
                client.as_ref(),
                t,
                &project.config.name,
                &space,
                &wanted,
                &known,
            )
            .map_err(|e| Fail::new(EXIT_NETWORK, e.to_string()))?;
            save_cache(cache.as_deref(), &PageCache::from_snapshot(&snapshot));
            (snapshot, Some(client))
        }
    };
    if let Some(path) = &o.save_snapshot {
        let text = serde_json::to_string_pretty(&snapshot).unwrap_or_default();
        fenix_storage::write(Path::new(path), format!("{text}\n").as_bytes())
            .map_err(|e| Fail::usage(format!("{path}: {e}")))?;
    }
    let taken: HashSet<String> = snapshot.taken_titles.iter().cloned().collect();
    let image_hash = |path: &str| {
        ctx.source.read(path).ok().map(|b| {
            kiln_confluence::hash(&b)
                .trim_start_matches("sha256:")
                .to_owned()
        })
    };
    let desired_ctx = DesiredContext {
        config,
        project,
        revision: ctx.revision,
        last_commits: ctx.last_commits,
        repo: ctx.repo,
        image_hash: &image_hash,
        all: ctx.all,
        taken_titles: &taken,
    };
    let desired = desired_pages(prep, &desired_ctx);
    let options = PlanOptions {
        project: project.config.name.clone(),
        export: t.name.clone(),
        on_drift: config.on_drift,
        force: o.force,
        max_page_kib: config.max_page_kib,
        adopting: t.confluence.page.is_some(),
        adopt: o.adopt,
        commit: ctx.revision.commit.clone().unwrap_or_default(),
        prune_after_days: o.prune_removed,
        today: today(),
    };
    Ok(ConfluenceRun {
        plan: plan(&desired, &snapshot, &options),
        space,
        anchor,
        desired,
        snapshot,
        options,
        client,
        cache,
        read_at,
        restrict_edits: config.restrict_edits,
        applied: None,
    })
}

/// The cache only saves reads, so failing to write it is not an error.
fn save_cache(path: Option<&Path>, cache: &PageCache) {
    if let Some(path) = path {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = fenix_storage::write(path, format!("{}\n", cache.to_text()).as_bytes());
    }
}

/// Publishes one Confluence target's plan, asking first unless `--yes`.
/// `Ok(true)` when it stopped after the plan (refused, declined, or
/// non-interactive without `--yes`).
fn publish(
    prep: &Prepared,
    run: &mut ConfluenceRun,
    o: &Options,
    pictures: &FenixPictures,
    source: &kiln_index::DiskSource,
    commit: Option<&str>,
    text: bool,
) -> Result<bool, Fail> {
    let t = &prep.target;
    if !run.plan.refusals.is_empty() {
        // Already reported with the plan; exit 1 follows.
        return Ok(false);
    }
    if !run.plan.changes() {
        return Ok(false);
    }
    let Some(client) = run.client.as_deref() else {
        return Ok(false);
    };
    if !o.yes {
        let interactive = text && std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
        if !interactive {
            eprintln!(
                "kiln: `{}` not published: pass --yes to publish without asking",
                t.name
            );
            return Ok(true);
        }
        print!("Publish `{}` to space {}? [y/N] ", t.name, run.space);
        let _ = std::io::stdout().flush();
        let mut answer = String::new();
        let _ = std::io::stdin().read_line(&mut answer);
        if !matches!(answer.trim(), "y" | "Y" | "yes") {
            println!("  not published");
            return Ok(true);
        }
    }
    // The space may have moved on while the plan was read or confirmed.
    if run.read_at.elapsed() > FRESH {
        let cache = PageCache::from_snapshot(&run.snapshot);
        let wanted: Vec<String> = titles(prep, &HashSet::new()).into_values().collect();
        let fresh = take_snapshot(client, t, &run.options.project, &run.space, &wanted, &cache)
            .map_err(|e| Fail::new(EXIT_NETWORK, e.to_string()))?;
        let again = plan(&run.desired, &fresh, &run.options);
        let shape = |p: &Plan| -> Vec<(OpKind, String)> {
            p.ops.iter().map(|op| (op.kind, op.key.clone())).collect()
        };
        if shape(&again) != shape(&run.plan) || !again.refusals.is_empty() {
            return Err(Fail::new(
                EXIT_FINDINGS,
                format!(
                    "`{}`: the space changed since the plan was read; run again to see the new plan",
                    t.name
                ),
            ));
        }
        run.snapshot = fresh;
        run.plan = again;
    }
    let bytes = |a: &Attachment| match &a.source {
        AttachmentSource::Diagram(src) => pictures.diagram(src).map(|p| p.png),
        AttachmentSource::File(path) => source.read(path).map_err(|e| format!("{path}: {e}")),
    };
    let config = run.options.clone();
    let apply_options = ApplyOptions {
        space: &run.space,
        anchor: run.anchor,
        adopting: config.adopting,
        project: &config.project,
        export: &config.export,
        target_title: &t.title,
        commit: commit.unwrap_or_default(),
        restrict_edits: run.restrict_edits,
        restrict_adopted: t.confluence.restrict_adopted,
        on_drift: config.on_drift,
        force: o.force,
        today: config.today,
    };
    let result = kiln_confluence::apply(
        client,
        &run.desired,
        &run.plan,
        &run.snapshot,
        &bytes,
        &apply_options,
    );
    let mut cache = PageCache::from_snapshot(&run.snapshot);
    let applied = result.map_err(|e| {
        Fail::new(
            EXIT_NETWORK,
            format!("publishing `{}` stopped: {e}; run again to finish", t.name),
        )
    })?;
    for (id, (version, property)) in &applied.pages {
        cache.remember(*id, *version, Some(property.clone()));
    }
    save_cache(run.cache.as_deref(), &cache);
    if text {
        println!(
            "{}: published {} created, {} updated, {} retired, {} pruned",
            t.name, applied.created, applied.updated, applied.retired, applied.pruned
        );
        for why in &applied.skipped {
            println!("  skipped: {why}");
        }
    }
    run.applied = Some(applied);
    Ok(false)
}

fn applied_json(a: &Applied) -> Value {
    json!({
        "created": a.created, "updated": a.updated, "retired": a.retired,
        "pruned": a.pruned, "skipped": a.skipped,
    })
}
