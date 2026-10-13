//! The emitter (golden storage, well-formed), the planner (table-driven)
//! and the read client against the fake server in both flavours.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use kiln_export::{prepare, resolve_all, Assembly, RepoLinks, Revision};
use kiln_index::{MemorySource, Project};
use kiln_model::{ConfluenceConfig, ConfluenceFlavour, Diagrams, OnDrift};
use kiln_secrets::Token;
use serde_json::json;

use crate::fake::{Fake, FakePage, State};
use crate::{
    desired_pages, normalise, plan, take_snapshot, DesiredContext, DesiredPage, Http, KilnProperty,
    OpKind, PlanOptions, RemotePage, Snapshot,
};

fn item(id: &str, ty: &str, title: &str, status: &str, rest: &str) -> String {
    let (fm, body) = rest.split_once("+++").unwrap_or((rest, ""));
    format!("+++\nid = \"{id}\"\ntype = \"{ty}\"\ntitle = \"{title}\"\nstatus = \"{status}\"\n{fm}+++\n{body}")
}

const TARGETS: &str = r#"
[confluence]
url = "https://confluence.example.com"
space = "FLINT"
labels = ["flint"]

[[export]]
name = "spec"
format = "confluence"
title = "Flint — design document"
[export.confluence]
parent = 100

[[export]]
name = "browse"
format = "confluence"
layout = "tree"
types = ["requirement"]
reports = []
link_to = ["spec"]
[export.confluence]
parent = 100
labels = ["browse"]
"#;

fn project() -> (Project, MemorySource) {
    let config = format!(
        "{}\n[code]\npaths = [\"src\"]\n\n[repository]\nweb = \"https://git.example.com/flint/-/blob/{{commit}}/{{path}}#L{{line}}\"\n\n[jira]\nurl = \"https://jira.example.com\"\n{TARGETS}",
        kiln_model::default_config("Flint")
    );
    let source = MemorySource::new()
        .with("kiln.toml", config)
        .with("design/img/a.png", b"\x89PNG a")
        .with("design/REQ-001-launcher.md", item("REQ-001", "requirement", "Flint is a launcher", "approved", "jira = \"FLINT-12\"\n+++\nFinds things & opens them; see [[ADR-001]].\n\n![a](img/a.png)\n"))
        .with(
            "design/REQ-002-fast.md",
            item("REQ-002", "requirement", "Fast <enough>", "verified", "parent = \"REQ-001\"\n+++\n> [!WARNING]\n> Careful.\n\n```mermaid\nflowchart TD\n  a --> b\n```\n\n```rust\nlet x = \"]]>\";\n```\n\n- [x] done\n"),
        )
        .with("design/ADR-001-toml.md", item("ADR-001", "decision", "TOML front matter", "accepted", ""))
        .with("src/a.rs", format!("// {}: REQ-002\n", "verifies"));
    let project = Project::load(&source).unwrap();
    assert!(project.findings.is_empty(), "{:?}", project.findings);
    (project, source)
}

fn desired(
    project: &Project,
    source: &MemorySource,
    name: &str,
    taken: &HashSet<String>,
) -> Vec<DesiredPage> {
    let rev = Revision {
        commit: Some("9f8e7d6c5b4a39281706f5e4d3c2b1a098765432".into()),
        date: Some("2026-10-12".into()),
        in_tree: false,
    };
    let a = Assembly {
        project,
        findings: &[],
        revision: &rev,
    };
    let all: Vec<_> = project
        .config
        .exports
        .iter()
        .map(|t| prepare(t, None, &a))
        .collect();
    let mut resolved = all.clone();
    resolve_all(&mut resolved, &all, project);
    let p = resolved.iter().find(|p| p.target.name == name).unwrap();
    let repo = RepoLinks::Web {
        template: project.config.repository_web.clone().unwrap(),
        commits: HashMap::from([(
            "design/REQ-001-launcher.md".to_owned(),
            "c0ffee1".to_owned(),
        )]),
        fallback: "9f8e7d6c5b4a".into(),
    };
    let last = HashMap::from([(
        "design/REQ-001-launcher.md".to_owned(),
        "c0ffee1".to_owned(),
    )]);
    let image_hash = |path: &str| {
        kiln_index::FileSource::read(source, path)
            .ok()
            .map(|b| crate::hash(&b).trim_start_matches("sha256:").to_owned())
    };
    let ctx = DesiredContext {
        config: project.config.confluence.as_ref().unwrap(),
        project,
        revision: &rev,
        last_commits: &last,
        repo: &repo,
        image_hash: &image_hash,
        all: &resolved,
        taken_titles: taken,
    };
    desired_pages(p, &ctx)
}

fn golden(name: &str, actual: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("golden")
        .join(name);
    if std::env::var_os("KILN_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing golden {name}; run with KILN_BLESS=1"));
    assert!(expected == actual, "{name} differs from its golden file; run with KILN_BLESS=1 and review the diff\n--- actual ---\n{actual}");
}

/// One element per line, so golden diffs are readable.
fn pretty(body: &str) -> String {
    body.replace("><", ">\n<")
}

#[test]
fn storage_is_well_formed_and_golden() {
    let (project, source) = project();
    for name in ["spec", "browse"] {
        let pages = desired(&project, &source, name, &HashSet::new());
        let mut out = String::new();
        for page in &pages {
            normalise(&page.body).unwrap_or_else(|e| panic!("{}: {e}", page.key));
            let attachments: Vec<String> = page
                .attachments
                .iter()
                .map(|a| format!("{} ({:?})", a.name, a.source))
                .collect();
            out.push_str(&format!(
                "=== {} | {} | parent {:?} | labels {:?}\nitems {:?}\nattachments {attachments:?}\n{}\n",
                page.key,
                page.title,
                page.parent,
                page.labels,
                page.items,
                pretty(&page.body)
            ));
        }
        golden(&format!("storage-{name}.xml"), &out);
    }
}

#[test]
fn rendering_is_deterministic_and_titles_disambiguate() {
    let (project, source) = project();
    assert_eq!(
        desired(&project, &source, "browse", &HashSet::new()),
        desired(&project, &source, "browse", &HashSet::new())
    );
    let taken = HashSet::from(["REQ-002 Fast <enough>".to_owned()]);
    let pages = desired(&project, &source, "browse", &taken);
    let titles: Vec<&str> = pages.iter().map(|p| p.title.as_str()).collect();
    assert!(
        titles.contains(&"REQ-002 Fast <enough> (Flint)"),
        "{titles:?}"
    );
}

fn page(key: &str, body: &str, hash: &str) -> DesiredPage {
    DesiredPage {
        key: key.into(),
        title: format!("title {key}"),
        parent: (key != "doc").then(|| "doc".to_owned()),
        body: body.into(),
        labels: vec![],
        attachments: vec![],
        items: BTreeMap::from([("REQ-001".to_owned(), "c2".to_owned())]),
        render_hash: hash.into(),
    }
}

fn remote(id: u64, key: &str, version: u64, render: &str, body_hash: Option<&str>) -> RemotePage {
    RemotePage {
        id,
        title: format!("title {key}"),
        parent_id: (key != "doc").then_some(10),
        version,
        version_author: "alice".into(),
        version_when: "2026-10-10T14:02".into(),
        property: Some(KilnProperty {
            schema: 1,
            project: "Flint".into(),
            export: "spec".into(),
            key: key.into(),
            items: BTreeMap::from([("REQ-001".to_owned(), "c1".to_owned())]),
            published: "c1".into(),
            render_hash: render.into(),
            remote_hash: "R".into(),
            published_version: 3,
            attachments: BTreeMap::new(),
            retired: false,
            retired_at: String::new(),
        }),
        body_hash: body_hash.map(str::to_owned),
        empty: false,
        leftover: false,
    }
}

fn options() -> PlanOptions {
    PlanOptions {
        project: "Flint".into(),
        export: "spec".into(),
        on_drift: OnDrift::Stop,
        force: false,
        max_page_kib: 1,
        adopting: false,
        adopt: false,
        commit: "9f8e7d6c5b4a".into(),
        prune_after_days: None,
        today: kiln_model::Date {
            year: 2026,
            month: 10,
            day: 13,
        },
    }
}

#[test]
fn planner_table() {
    let snapshot = |root: Option<RemotePage>, descendants: Vec<RemotePage>| Snapshot {
        anchor_page: 1,
        root,
        descendants,
        taken_titles: vec![],
    };
    let kinds = |p: &crate::Plan| {
        p.ops
            .iter()
            .map(|o| format!("{:?} {}", o.kind, o.key))
            .collect::<Vec<_>>()
    };

    // Nothing there: create.
    let p = plan(
        &[page("doc", "b", "H")],
        &snapshot(None, vec![]),
        &options(),
    );
    assert_eq!(kinds(&p), ["Create doc"]);
    // Same render hash: unchanged, no call.
    let p = plan(
        &[page("doc", "b", "H")],
        &snapshot(Some(remote(10, "doc", 3, "H", None)), vec![]),
        &options(),
    );
    assert_eq!(kinds(&p), ["Unchanged doc"]);
    assert!(!p.changes());
    // Different: update, naming the items that changed.
    let p = plan(
        &[page("doc", "b", "H2")],
        &snapshot(Some(remote(10, "doc", 3, "H", None)), vec![]),
        &options(),
    );
    assert_eq!(p.ops[0].reasons, ["REQ-001 changed"]);
    // A version bump with an identical body isn't drift.
    let p = plan(
        &[page("doc", "b", "H")],
        &snapshot(Some(remote(10, "doc", 4, "H", Some("R"))), vec![]),
        &options(),
    );
    assert_eq!(kinds(&p), ["Unchanged doc"]);
    // Drift: stop refuses the plan; skip leaves the page; --force overwrites.
    let drifted = snapshot(Some(remote(10, "doc", 7, "H", Some("X"))), vec![]);
    let p = plan(&[page("doc", "b", "H2")], &drifted, &options());
    assert_eq!(kinds(&p), ["Drift doc"]);
    assert!(
        p.refusals[0]
            .contains("was edited in Confluence (edited by alice at 2026-10-10T14:02, version 7)"),
        "{:?}",
        p.refusals
    );
    let p = plan(
        &[page("doc", "b", "H2")],
        &drifted,
        &PlanOptions {
            on_drift: OnDrift::Skip,
            ..options()
        },
    );
    assert_eq!(
        (kinds(&p), p.refusals.len()),
        (vec!["Drift doc".to_owned()], 0)
    );
    let p = plan(
        &[page("doc", "b", "H2")],
        &drifted,
        &PlanOptions {
            force: true,
            ..options()
        },
    );
    assert_eq!(kinds(&p), ["Update doc"]);
    // Pages nothing produces are retired; foreign pages are listed only.
    let mut foreign = remote(30, "x", 1, "", None);
    foreign.property = None;
    foreign.title = "Someone's notes".into();
    let p = plan(
        &[page("doc", "b", "H")],
        &snapshot(
            Some(remote(10, "doc", 3, "H", None)),
            vec![remote(20, "item:REQ-009", 3, "Z", None), foreign],
        ),
        &options(),
    );
    assert_eq!(kinds(&p), ["Unchanged doc", "Retire item:REQ-009"]);
    assert_eq!(p.foreign, ["Someone's notes"]);
    // A page that moved under another parent.
    let mut moved = remote(20, "section:a", 3, "old", None);
    moved.parent_id = Some(99);
    let p = plan(
        &[page("doc", "b", "H"), page("section:a", "b", "new")],
        &snapshot(Some(remote(10, "doc", 3, "H", None)), vec![moved]),
        &options(),
    );
    assert_eq!(p.ops[1].reasons[0], "parent ? → doc");
    // Size: a warning past max_page_kib, a refusal past four times it.
    let p = plan(
        &[page("doc", &"x".repeat(2048), "H")],
        &snapshot(None, vec![]),
        &options(),
    );
    assert_eq!((p.warnings.len(), p.refusals.len()), (1, 0));
    let p = plan(
        &[page("doc", &"x".repeat(5000), "H")],
        &snapshot(None, vec![]),
        &options(),
    );
    assert_eq!(p.refusals.len(), 1);
    // Adopting a page with content needs --adopt; an empty one doesn't.
    let mut adopted = remote(10, "doc", 3, "", None);
    adopted.property = None;
    let p = plan(
        &[page("doc", "b", "H")],
        &snapshot(Some(adopted.clone()), vec![]),
        &PlanOptions {
            adopting: true,
            ..options()
        },
    );
    assert!(p.refusals[0].contains("needs --adopt"));
    adopted.empty = true;
    let p = plan(
        &[page("doc", "b", "H")],
        &snapshot(Some(adopted), vec![]),
        &PlanOptions {
            adopting: true,
            ..options()
        },
    );
    assert_eq!(
        (kinds(&p), p.refusals.len()),
        (vec!["Update doc".to_owned()], 0)
    );
    assert_eq!(p.count(OpKind::Update), 1);

    // Version messages name the commit and what changed.
    let p = plan(
        &[page("doc", "b", "H2")],
        &snapshot(Some(remote(10, "doc", 3, "H", None)), vec![]),
        &options(),
    );
    assert_eq!(p.ops[0].message, "kiln 9f8e7d6: REQ-001 changed");

    // A retired page is left alone, pruned once old enough, and moved back
    // when its item returns.
    let mut gone = remote(20, "item:REQ-009", 3, "Z", None);
    let prop = gone.property.as_mut().unwrap();
    prop.retired = true;
    prop.retired_at = "2026-09-01".into();
    let with_gone = snapshot(Some(remote(10, "doc", 3, "H", None)), vec![gone]);
    let p = plan(&[page("doc", "b", "H")], &with_gone, &options());
    assert_eq!(kinds(&p), ["Unchanged doc"]);
    let p = plan(
        &[page("doc", "b", "H")],
        &with_gone,
        &PlanOptions {
            prune_after_days: Some(30),
            ..options()
        },
    );
    assert_eq!(kinds(&p), ["Unchanged doc", "Prune item:REQ-009"]);
    let p = plan(
        &[page("doc", "b", "H")],
        &with_gone,
        &PlanOptions {
            prune_after_days: Some(60),
            ..options()
        },
    );
    assert_eq!(kinds(&p), ["Unchanged doc"]);
    let p = plan(
        &[page("doc", "b", "H"), page("item:REQ-009", "b", "Z")],
        &with_gone,
        &options(),
    );
    assert_eq!(kinds(&p), ["Unchanged doc", "Update item:REQ-009"]);
    assert_eq!(p.ops[1].reasons, ["back in the repository"]);
}

fn fake_state() -> State {
    let property = |key: &str| {
        json!({ "schema": 1, "project": "Flint", "export": "browse", "key": key, "items": {}, "published": "c1",
                "renderHash": "H", "remoteHash": crate::body_hash("<p>x</p>"), "publishedVersion": 2, "attachments": {} })
    };
    let page = |title: &str,
                parent: Option<u64>,
                version: u64,
                body: &str,
                property: Option<serde_json::Value>| FakePage {
        title: title.into(),
        parent,
        version,
        author: "alice".into(),
        body: body.into(),
        property,
    };
    let mut state = State::default();
    state.pages.insert(100, page("Designs", None, 1, "", None));
    state
        .pages
        .insert(150, page("Unrelated sibling", Some(100), 1, "", None));
    state.pages.insert(
        200,
        page(
            "Flint (browse)",
            Some(100),
            2,
            "<p>x</p>",
            Some(property("doc")),
        ),
    );
    state.pages.insert(
        300,
        page(
            "1 Requirements",
            Some(200),
            5,
            "<p>edited</p>",
            Some(property("section:requirements")),
        ),
    );
    state
        .pages
        .insert(301, page("Someone's notes", Some(200), 1, "", None));
    state
        .pages
        .insert(400, page("REQ-002 Fast <enough>", Some(999), 1, "", None));
    state
        .rate_limit_once
        .push("/rest/api/content/100/child".into());
    state
        .rate_limit_once
        .push("/wiki/api/v2/pages/100/children".into());
    state
}

fn client(fake: &Fake, cloud: bool) -> Box<dyn crate::Confluence> {
    let config = ConfluenceConfig {
        url: fake.url.clone(),
        flavour: if cloud {
            ConfluenceFlavour::Cloud
        } else {
            ConfluenceFlavour::DataCenter
        },
        space: "FLINT".into(),
        user: String::new(),
        auth: "bearer".into(),
        labels: vec![],
        jira_macro: true,
        jira_server_id: String::new(),
        restrict_edits: false,
        on_drift: OnDrift::Stop,
        concurrency: 4,
        diagrams: Diagrams::Image,
        max_page_kib: 1024,
    };
    let mut http = Http::new(&config, &Token::new("test-token"), "");
    http.sleep = |_| {};
    if cloud {
        Box::new(crate::cloud::Cloud::new(http))
    } else {
        Box::new(crate::dc::DataCenter { http })
    }
}

#[test]
fn snapshots_read_the_same_from_both_flavours() {
    let (project, _) = project();
    let target = project.config.export("browse").unwrap();
    let wanted = vec![
        "REQ-002 Fast <enough>".to_owned(),
        "1 Requirements".to_owned(),
    ];
    let mut snapshots = Vec::new();
    for cloud in [false, true] {
        let fake = Fake::start(fake_state(), cloud);
        let c = client(&fake, cloud);
        assert_eq!(c.whoami().unwrap(), "Kiln Test");
        let mut snapshot = take_snapshot(
            c.as_ref(),
            target,
            "Flint",
            "FLINT",
            &wanted,
            &crate::PageCache::default(),
        )
        .unwrap();
        // Authors are names on Data Center and account ids on Cloud; both
        // are "alice" here. Times are the same string in the fake.
        snapshot.descendants.sort_by_key(|p| p.id);
        assert_eq!(snapshot.root.as_ref().map(|r| r.id), Some(200));
        assert_eq!(
            snapshot
                .descendants
                .iter()
                .map(|p| p.id)
                .collect::<Vec<_>>(),
            [300, 301]
        );
        assert_eq!(snapshot.descendants[0].parent_id, Some(200));
        assert!(
            snapshot.root.as_ref().unwrap().body_hash.is_none(),
            "version 2 is the published one: no body read"
        );
        assert_ne!(
            snapshot.descendants[0].body_hash, None,
            "version 5 > 2: the body is read for drift"
        );
        assert_eq!(snapshot.taken_titles, ["REQ-002 Fast <enough>"]);
        let log = fake.state.lock().unwrap().log.clone();
        assert!(
            log.iter().filter(|p| p.contains("/100/child")).count() >= 2,
            "429 retried: {log:?}"
        );
        snapshots.push(snapshot);
    }
    assert_eq!(snapshots[0], snapshots[1]);
}

#[test]
fn a_refused_token_is_one_line_without_the_token() {
    let fake = Fake::start(fake_state(), false);
    let config = ConfluenceConfig {
        url: fake.url.clone(),
        flavour: ConfluenceFlavour::DataCenter,
        space: "FLINT".into(),
        user: String::new(),
        auth: "bearer".into(),
        labels: vec![],
        jira_macro: true,
        jira_server_id: String::new(),
        restrict_edits: false,
        on_drift: OnDrift::Stop,
        concurrency: 4,
        diagrams: Diagrams::Image,
        max_page_kib: 1024,
    };
    let http = Http::new(&config, &Token::new("wrong-secret"), "");
    let c = crate::dc::DataCenter { http };
    let e = crate::Confluence::whoami(&c).unwrap_err().to_string();
    assert!(
        e.starts_with("Confluence refused the token for http://127.0.0.1:"),
        "{e}"
    );
    assert!(!e.contains("wrong-secret") && !e.contains('\n'));
    assert_eq!(crate::Confluence::page(&c, 1).unwrap_err().to_string(), e);
}

/// A fake space with an empty parent page 100 for targets to publish
/// under.
fn empty_space() -> State {
    let mut state = State::default();
    state.pages.insert(
        100,
        FakePage {
            title: "Designs".into(),
            parent: None,
            version: 1,
            author: "alice".into(),
            body: String::new(),
            property: None,
        },
    );
    state
}

/// Snapshot, plan and (unless the plan is refused) apply `name` from
/// `source`; the plan, and what applying did.
fn publish(
    fake: &Fake,
    cloud: bool,
    source: &MemorySource,
    name: &str,
    tweak: impl Fn(&mut PlanOptions),
) -> (crate::Plan, Result<crate::Applied, crate::ClientError>) {
    let project = Project::load(source).unwrap();
    let c = client(fake, cloud);
    let target = project.config.export(name).unwrap().clone();
    let rev = Revision {
        commit: Some("9f8e7d6c5b4a39281706f5e4d3c2b1a098765432".into()),
        date: Some("2026-10-12".into()),
        in_tree: false,
    };
    let a = Assembly {
        project: &project,
        findings: &[],
        revision: &rev,
    };
    let all: Vec<_> = project
        .config
        .exports
        .iter()
        .map(|t| prepare(t, None, &a))
        .collect();
    let mut resolved = all.clone();
    resolve_all(&mut resolved, &all, &project);
    let p = resolved.iter().find(|p| p.target.name == name).unwrap();
    let wanted: Vec<String> = crate::titles(p, &HashSet::new()).into_values().collect();
    let snapshot = take_snapshot(
        c.as_ref(),
        &target,
        "Flint",
        "FLINT",
        &wanted,
        &crate::PageCache::default(),
    )
    .unwrap();
    let taken: HashSet<String> = snapshot.taken_titles.iter().cloned().collect();
    let image_hash = |path: &str| {
        kiln_index::FileSource::read(source, path)
            .ok()
            .map(|b| crate::hash(&b).trim_start_matches("sha256:").to_owned())
    };
    let config = project.config.confluence.clone().unwrap();
    let ctx = DesiredContext {
        config: &config,
        project: &project,
        revision: &rev,
        last_commits: &HashMap::new(),
        repo: &RepoLinks::None,
        image_hash: &image_hash,
        all: &resolved,
        taken_titles: &taken,
    };
    let desired = desired_pages(p, &ctx);
    let mut options = PlanOptions {
        export: name.into(),
        max_page_kib: 1024,
        adopting: target.confluence.page.is_some(),
        ..options()
    };
    tweak(&mut options);
    let the_plan = plan(&desired, &snapshot, &options);
    if !the_plan.refusals.is_empty() {
        return (the_plan, Err(crate::ClientError::new("refused")));
    }
    let bytes = |a: &crate::Attachment| match &a.source {
        crate::AttachmentSource::Diagram(src) => {
            kiln_export::Pictures::diagram(&kiln_export::FakePictures, src).map(|p| p.png)
        }
        crate::AttachmentSource::File(path) => {
            kiln_index::FileSource::read(source, path).map_err(|e| e.to_string())
        }
    };
    let apply_options = crate::ApplyOptions {
        space: "FLINT",
        anchor: target.confluence.page.or(target.confluence.parent).unwrap(),
        adopting: options.adopting,
        project: "Flint",
        export: name,
        target_title: &target.title,
        commit: "9f8e7d6c5b4a39281706f5e4d3c2b1a098765432",
        restrict_edits: config.restrict_edits,
        restrict_adopted: target.confluence.restrict_adopted,
        on_drift: options.on_drift,
        force: options.force,
        today: options.today,
    };
    let applied = crate::apply(
        c.as_ref(),
        &desired,
        &the_plan,
        &snapshot,
        &bytes,
        &apply_options,
    );
    (the_plan, applied)
}

fn kinds_of(p: &crate::Plan) -> Vec<String> {
    p.ops
        .iter()
        .map(|o| format!("{:?} {}", o.kind, o.key))
        .collect()
}

/// The fake's pages as (title, parent title, body, labels), without ids,
/// for comparing two end states.
fn end_state(state: &State) -> Vec<(String, String, String, Vec<String>)> {
    let mut out: Vec<_> = state
        .pages
        .iter()
        .map(|(id, p)| {
            let parent = p
                .parent
                .and_then(|x| state.pages.get(&x))
                .map(|x| x.title.clone())
                .unwrap_or_default();
            let mut labels = state
                .extra
                .get(id)
                .map(|e| e.labels.clone())
                .unwrap_or_default();
            labels.sort();
            (p.title.clone(), parent, p.body.clone(), labels)
        })
        .collect();
    out.sort();
    out
}

#[test]
fn publishing_a_document_then_nothing_to_do() {
    let (_, source) = project();
    for cloud in [false, true] {
        let fake = Fake::start(empty_space(), cloud);
        let (p, applied) = publish(&fake, cloud, &source, "spec", |_| {});
        assert_eq!(kinds_of(&p), ["Create doc"]);
        let applied = applied.unwrap();
        assert_eq!((applied.created, applied.updated), (1, 0));
        {
            let s = fake.state.lock().unwrap();
            let (id, page) = s
                .pages
                .iter()
                .find(|(_, p)| p.title == "Flint — design document")
                .unwrap();
            assert_eq!(page.parent, Some(100));
            assert!(page
                .body
                .contains("<ac:parameter ac:name=\"\">REQ-002</ac:parameter>"));
            let extra = &s.extra[id];
            assert_eq!(extra.labels, ["flint"]);
            let names: Vec<&str> = extra
                .attachments
                .iter()
                .map(|(_, n, _)| n.as_str())
                .collect();
            assert_eq!(names.len(), 2, "{names:?}");
            assert!(
                names.iter().any(|n| n.starts_with("diagram-"))
                    && names.iter().any(|n| n.starts_with("image-"))
            );
            let image = extra
                .attachments
                .iter()
                .find(|(_, n, _)| n.starts_with("image-"))
                .unwrap();
            assert_eq!(image.2, b"\x89PNG a", "uploaded bytes arrive intact");
            let property = page.property.as_ref().unwrap();
            assert_eq!(property["key"], "doc");
            assert_eq!(property["publishedVersion"], page.version);
            assert_eq!(property["remoteHash"], crate::body_hash(&page.body));
        }
        let before = fake.state.lock().unwrap().writes();
        let (p, applied) = publish(&fake, cloud, &source, "spec", |_| {});
        assert_eq!(kinds_of(&p), ["Unchanged doc"], "cloud {cloud}");
        assert!(!p.changes());
        let _ = applied.unwrap();
        assert_eq!(
            fake.state.lock().unwrap().writes(),
            before,
            "an unchanged publish writes nothing"
        );
    }
}

#[test]
fn a_tree_publishes_parent_first_and_retires_removed_items() {
    let (_, source) = project();
    for cloud in [false, true] {
        let fake = Fake::start(empty_space(), cloud);
        let (p, applied) = publish(&fake, cloud, &source, "browse", |_| {});
        assert_eq!(
            kinds_of(&p),
            [
                "Create doc",
                "Create section:requirements",
                "Create item:REQ-001",
                "Create item:REQ-002"
            ]
        );
        assert_eq!(applied.unwrap().created, 4);
        {
            let s = fake.state.lock().unwrap();
            let parent_of = |title: &str| {
                let page = s.pages.values().find(|p| p.title == title).unwrap();
                s.pages[&page.parent.unwrap()].title.clone()
            };
            assert_eq!(
                parent_of("REQ-002 Fast <enough>"),
                "REQ-001 Flint is a launcher"
            );
            assert_eq!(parent_of("1 Requirements"), "Flint");
        }
        let (p, _) = publish(&fake, cloud, &source, "browse", |_| {});
        assert!(!p.changes(), "{:?}", kinds_of(&p));

        // REQ-002 leaves the repository.
        let mut smaller = source.clone();
        smaller.insert("design/REQ-002-fast.md", "removed: not an item any more\n");
        let (p, applied) = publish(&fake, cloud, &smaller, "browse", |_| {});
        assert!(
            kinds_of(&p).contains(&"Retire item:REQ-002".to_owned()),
            "{:?}",
            kinds_of(&p)
        );
        assert_eq!(applied.unwrap().retired, 1);
        {
            let s = fake.state.lock().unwrap();
            let (id, gone) = s
                .pages
                .iter()
                .find(|(_, p)| p.title == "REQ-002 Fast <enough>")
                .unwrap();
            let removed = &s.pages[&gone.parent.unwrap()];
            assert_eq!(removed.title, "Removed from the repository");
            assert!(gone
                .body
                .starts_with("<ac:structured-macro ac:name=\"warning\">"));
            assert!(s.extra[id].labels.contains(&"kiln-removed".to_owned()));
            assert_eq!(gone.property.as_ref().unwrap()["retired"], true);
        }
        let (p, _) = publish(&fake, cloud, &smaller, "browse", |_| {});
        assert!(!p.changes(), "a retired page stays put: {:?}", kinds_of(&p));
        // It comes back.
        let (p, applied) = publish(&fake, cloud, &source, "browse", |_| {});
        assert!(
            p.ops
                .iter()
                .any(|o| o.key == "item:REQ-002" && o.reasons == ["back in the repository"]),
            "{:?}",
            p.ops
        );
        applied.unwrap();
        let s = fake.state.lock().unwrap();
        let back = s
            .pages
            .values()
            .find(|p| p.title == "REQ-002 Fast <enough>")
            .unwrap();
        assert_eq!(
            s.pages[&back.parent.unwrap()].title,
            "REQ-001 Flint is a launcher"
        );
    }
}

#[test]
fn drift_stops_skips_or_is_overwritten() {
    let (_, source) = project();
    let fake = Fake::start(empty_space(), false);
    publish(&fake, false, &source, "spec", |_| {}).1.unwrap();
    let id = {
        let mut s = fake.state.lock().unwrap();
        let id = *s
            .pages
            .iter()
            .find(|(_, p)| p.title == "Flint — design document")
            .unwrap()
            .0;
        s.edit(id, "<p>Someone fixed a typo here.</p>");
        id
    };
    let (p, applied) = publish(&fake, false, &source, "spec", |_| {});
    assert_eq!(kinds_of(&p), ["Drift doc"]);
    assert!(
        p.refusals[0].contains("edited by alice"),
        "{:?}",
        p.refusals
    );
    assert!(applied.is_err());
    assert_eq!(
        fake.state.lock().unwrap().pages[&id].body,
        "<p>Someone fixed a typo here.</p>",
        "nothing written"
    );
    let (p, _) = publish(&fake, false, &source, "spec", |o| {
        o.on_drift = OnDrift::Skip
    });
    assert_eq!(kinds_of(&p), ["Drift doc"]);
    let (p, applied) = publish(&fake, false, &source, "spec", |o| o.force = true);
    assert_eq!(kinds_of(&p), ["Update doc"]);
    applied.unwrap();
    assert!(fake.state.lock().unwrap().pages[&id]
        .body
        .contains("REQ-002"));
    let (p, _) = publish(&fake, false, &source, "spec", |_| {});
    assert_eq!(kinds_of(&p), ["Unchanged doc"]);
}

#[test]
fn conflicts_are_retried_once_and_oversized_pages_are_named() {
    let (_, source) = project();
    let fake = Fake::start(empty_space(), true);
    publish(&fake, true, &source, "spec", |_| {}).1.unwrap();
    let mut changed = source.clone();
    changed.insert(
        "design/ADR-001-toml.md",
        item(
            "ADR-001",
            "decision",
            "TOML front matter, really",
            "accepted",
            "",
        ),
    );
    let id = *fake
        .state
        .lock()
        .unwrap()
        .pages
        .iter()
        .find(|(_, p)| p.title == "Flint — design document")
        .unwrap()
        .0;
    fake.state.lock().unwrap().conflict_once.push(id);
    let (p, applied) = publish(&fake, true, &changed, "spec", |_| {});
    // Without git history every item has the same commit, so the change
    // shows as content rather than as an item.
    assert_eq!(
        p.ops[0].reasons,
        ["content changed (template, layout or references)"]
    );
    assert_eq!(applied.unwrap().updated, 1, "the 409 was retried");

    let fake = Fake::start(empty_space(), false);
    fake.state.lock().unwrap().max_body = Some(500);
    let (_, applied) = publish(&fake, false, &source, "spec", |_| {});
    let e = applied.unwrap_err().to_string();
    assert!(
        e.contains("is too large for the server; use layout = \"sections\""),
        "{e}"
    );
}

#[test]
fn an_interrupted_publish_finishes_on_the_next_run() {
    let (_, source) = project();
    let clean = Fake::start(empty_space(), false);
    publish(&clean, false, &source, "browse", |_| {}).1.unwrap();
    for fail_after in [1, 3, 6, 10, 15] {
        let fake = Fake::start(empty_space(), false);
        fake.state.lock().unwrap().writes_left = Some(fail_after);
        let (_, applied) = publish(&fake, false, &source, "browse", |_| {});
        assert!(applied.is_err(), "failing after {fail_after} writes");
        fake.state.lock().unwrap().writes_left = None;
        {
            let (p, a) = publish(&fake, false, &source, "browse", |_| {});
            if let Err(e) = a {
                panic!(
                    "after failing at {fail_after}: {e} {:?} {:?}",
                    p.refusals,
                    kinds_of(&p)
                );
            }
        }
        assert_eq!(
            end_state(&fake.state.lock().unwrap()),
            end_state(&clean.state.lock().unwrap()),
            "after failing at {fail_after}"
        );
        let (p, _) = publish(&fake, false, &source, "browse", |_| {});
        assert!(
            !p.changes(),
            "after failing at {fail_after}: {:?}",
            kinds_of(&p)
        );
    }
}

#[test]
fn changing_layout_retires_pages_and_restrictions_apply() {
    let (_, source) = project();
    let fake = Fake::start(empty_space(), false);
    publish(&fake, false, &source, "browse", |_| {}).1.unwrap();
    // The same target as one document now.
    let mut as_document = source.clone();
    let config =
        String::from_utf8(kiln_index::FileSource::read(&source, "kiln.toml").unwrap()).unwrap();
    as_document.insert(
        "kiln.toml",
        config
            .replace("layout = \"tree\"\n", "")
            .replace("[confluence]\n", "[confluence]\nrestrict_edits = true\n"),
    );
    let (p, applied) = publish(&fake, false, &as_document, "browse", |_| {});
    let kinds = kinds_of(&p);
    assert!(
        kinds.contains(&"Update doc".to_owned())
            && kinds.iter().filter(|k| k.starts_with("Retire")).count() == 3,
        "{kinds:?}"
    );
    assert_eq!(applied.unwrap().retired, 3);

    let fresh = Fake::start(empty_space(), true);
    publish(&fresh, true, &as_document, "spec", |_| {})
        .1
        .unwrap();
    let s = fresh.state.lock().unwrap();
    let (id, _) = s
        .pages
        .iter()
        .find(|(_, p)| p.title == "Flint — design document")
        .unwrap();
    assert_eq!(s.extra[id].restricted_to.as_deref(), Some("acc-kiln"));
}
