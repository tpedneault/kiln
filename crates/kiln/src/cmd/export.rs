//! `kiln export`: renders every (or the named) export target and writes
//! file targets whose bytes changed; `--dry-run` shows what would change,
//! `--check` also exits 1 if anything would. Confluence targets are
//! read-only in K2: `--dry-run` reads the space (or a snapshot file) and
//! prints the plan; publishing arrives in K3.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use kiln_confluence::{
    desired_pages, plan, take_snapshot, titles, DesiredContext, OpKind, Plan, PlanOptions, Snapshot,
};
use kiln_export::{
    plan_files, prepare, render_files, resolve_all, Assembly, FenixPictures, FilePlan, Prepared,
    RepoLinks, Revision,
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
    pub format: TextOrJson,
}

/// What exporting one target came to.
enum Outcome {
    Files(FilePlan),
    Confluence {
        plan: Plan,
        space: String,
        anchor: u64,
    },
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
    if selected
        .iter()
        .any(|t| t.format == ExportFormat::Confluence)
        && !(o.dry_run || o.check)
    {
        return Err(Fail::usage("publishing to Confluence arrives in K3; `kiln export --dry-run` shows the plan without writing"));
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
            confluence_plan(
                prep,
                &prepared,
                p,
                &loaded.source,
                &o,
                &Revision {
                    commit: commit.clone(),
                    date: date.clone(),
                    in_tree: false,
                },
                &last_commits,
                &web(false),
            )?
        };
        results.push((prep, outcome));
    }

    let mut changed = false;
    let mut refused = false;
    if !(o.dry_run || o.check) {
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
            Outcome::Confluence {
                plan,
                space,
                anchor,
            } => {
                changed |= plan.changes();
                refused |= !plan.refusals.is_empty();
                if o.format == TextOrJson::Json {
                    json_targets.push(json!({
                        "name": t.name, "format": "confluence", "layout": t.layout.as_str(), "space": space,
                        "parent": anchor, "plan": plan, "leftOut": left_out,
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
                    "  {} create, {} update, {} unchanged, {} drift, {} retire",
                    plan.count(OpKind::Create),
                    plan.count(OpKind::Update),
                    plan.count(OpKind::Unchanged),
                    plan.count(OpKind::Drift),
                    plan.count(OpKind::Retire)
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

#[allow(clippy::too_many_arguments)]
fn confluence_plan(
    prep: &Prepared,
    all: &[Prepared],
    project: &kiln_index::Project,
    source: &kiln_index::DiskSource,
    o: &Options,
    revision: &Revision,
    last_commits: &HashMap<String, String>,
    repo: &RepoLinks,
) -> Result<Outcome, Fail> {
    let t = &prep.target;
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
    let snapshot: Snapshot = match &o.snapshot {
        Some(path) => {
            let text =
                std::fs::read_to_string(path).map_err(|e| Fail::usage(format!("{path}: {e}")))?;
            serde_json::from_str(&text)
                .map_err(|e| Fail::usage(format!("{path}: not a Kiln snapshot: {e}")))?
        }
        None => {
            let token = kiln_secrets::confluence_token().ok_or_else(|| {
                Fail::usage(format!(
                    "a dry run reads Confluence and needs {} (or --snapshot FILE)",
                    kiln_secrets::CONFLUENCE_TOKEN
                ))
            })?;
            let user = kiln_secrets::confluence_user().unwrap_or_else(|| config.user.clone());
            let client = kiln_confluence::connect(config, &token, &user);
            take_snapshot(client.as_ref(), t, &project.config.name, &space, &wanted)
                .map_err(|e| Fail::new(EXIT_NETWORK, e.to_string()))?
        }
    };
    if let Some(path) = &o.save_snapshot {
        let text = serde_json::to_string_pretty(&snapshot).unwrap_or_default();
        fenix_storage::write(Path::new(path), format!("{text}\n").as_bytes())
            .map_err(|e| Fail::usage(format!("{path}: {e}")))?;
    }
    let taken: HashSet<String> = snapshot.taken_titles.iter().cloned().collect();
    let image_hash = |path: &str| {
        source.read(path).ok().map(|b| {
            kiln_confluence::hash(&b)
                .trim_start_matches("sha256:")
                .to_owned()
        })
    };
    let ctx = DesiredContext {
        config,
        project,
        revision,
        last_commits,
        repo,
        image_hash: &image_hash,
        all,
        taken_titles: &taken,
    };
    let desired = desired_pages(prep, &ctx);
    let options = PlanOptions {
        project: project.config.name.clone(),
        export: t.name.clone(),
        on_drift: config.on_drift,
        force: false,
        max_page_kib: config.max_page_kib,
        adopting: t.confluence.page.is_some(),
        adopt: false,
    };
    Ok(Outcome::Confluence {
        plan: plan(&desired, &snapshot, &options),
        space,
        anchor,
    })
}
