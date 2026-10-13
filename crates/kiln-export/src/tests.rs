//! Assembly, layouts, back ends and the file destination over one sample
//! project (EXPORT.md, "Testing"). Golden files live in `golden/`; run
//! with `KILN_BLESS=1` to rewrite them after a deliberate change, and
//! review the diff.

use std::path::PathBuf;

use kiln_index::{MemorySource, Project};
use kiln_model::{ExportTarget, Layout};

use crate::{
    plan_files, prepare, render_files, resolve_all, Assembly, FakePictures, Node, Prepared,
    RepoLinks, Revision, SectionKind,
};

fn item(id: &str, ty: &str, title: &str, status: &str, rest: &str) -> String {
    let (fm, body) = rest.split_once("+++").unwrap_or((rest, ""));
    format!("+++\nid = \"{id}\"\ntype = \"{ty}\"\ntitle = \"{title}\"\nstatus = \"{status}\"\n{fm}+++\n{body}")
}

const TEMPLATE: &str = "# Introduction\n\n<!-- kiln:summary -->\n\nThis document describes <!-- kiln:value name -->.\n\n<!-- kiln:toc -->\n\n# Requirements\n\n## Performance\n\n<!-- kiln:items type=\"requirement\" tag=\"performance\" -->\n\n## Everything else\n\n<!-- kiln:items type=\"requirement\" -->\n\n# Decisions\n\n<!-- kiln:items type=\"decision\" status=\"accepted,superseded\" style=\"row\" -->\n\n# Appendix: traceability\n\n<!-- kiln:report traceability -->\n";

fn source(targets: &str) -> MemorySource {
    let config = format!(
        "{}\n[code]\npaths = [\"src\"]\n\n[repository]\nweb = \"https://git.example.com/flint/-/blob/{{commit}}/{{path}}#L{{line}}\"\n\n[jira]\nurl = \"https://jira.example.com\"\n{targets}",
        kiln_model::default_config("Flint")
    );
    MemorySource::new()
        .with("kiln.toml", config)
        .with("design/templates/doc.md", TEMPLATE)
        .with("design/img/flow.png", b"\x89PNG image")
        .with("design/REQ-001-launcher.md", item("REQ-001", "requirement", "Flint is a launcher", "approved", "jira = \"FLINT-12\"\n+++\nFlint finds and opens things; see [[ADR-002]].\n\n## Scope\n\nWindows and Linux.\n"))
        .with(
            "design/REQ-002-fast.md",
            item("REQ-002", "requirement", "Fast enough to be a reflex", "approved", "parent = \"REQ-001\"\ntags = [\"performance\"]\n+++\n> [!NOTE]\n> Measured on the reference laptop.\n\n```mermaid\nflowchart TD\n  hotkey --> frame\n```\n\n![flow](img/flow.png)\n"),
        )
        .with("design/REQ-003-themes.md", item("REQ-003", "requirement", "Themes follow Fenix", "draft", "parent = \"REQ-001\"\n+++\n| Theme | Source |\n|---|---|\n| dark | Fenix |\n"))
        .with("design/REQ-004-old.md", item("REQ-004", "requirement", "An old idea", "obsolete", ""))
        .with("design/ADR-001-yaml.md", item("ADR-001", "decision", "YAML front matter", "superseded", ""))
        .with("design/ADR-002-toml.md", item("ADR-002", "decision", "TOML front matter", "accepted", "[links]\nsupersedes = [\"ADR-001\"]\n+++\nComments survive edits.\n"))
        .with("design/TEST-001-bench.md", item("TEST-001", "test", "Hotkey benchmark", "passing", "[links]\nverifies = [\"REQ-002\"]\n"))
        .with("src/launcher.rs", format!("fn show() {{}}\n// {}: REQ-002\n", "implements"))
}

fn project(targets: &str) -> (Project, MemorySource) {
    let source = source(targets);
    let project = Project::load(&source).unwrap();
    assert!(project.findings.is_empty(), "{:?}", project.findings);
    (project, source)
}

fn target(project: &Project, name: &str) -> ExportTarget {
    project
        .config
        .export(name)
        .unwrap_or_else(|| panic!("no target {name}"))
        .clone()
}

const REVISION: Revision = Revision {
    commit: None,
    date: None,
    in_tree: false,
};

fn revision() -> Revision {
    Revision {
        commit: Some("9f8e7d6c5b4a39281706f5e4d3c2b1a098765432".into()),
        date: Some("2026-10-12".into()),
        in_tree: false,
    }
}

fn prepared(project: &Project, source: &MemorySource, names: &[&str]) -> Vec<Prepared> {
    let rev = revision();
    let a = Assembly {
        project,
        findings: &[],
        revision: &rev,
        since: None,
    };
    let all: Vec<Prepared> = names
        .iter()
        .map(|n| {
            let t = target(project, n);
            let template = t.template.as_ref().map(|p| {
                String::from_utf8(kiln_index::FileSource::read(source, p).unwrap()).unwrap()
            });
            prepare(&t, template.as_deref(), &a)
        })
        .collect();
    let mut resolved = all.clone();
    resolve_all(&mut resolved, &all, project);
    resolved
}

fn web(project: &Project) -> RepoLinks {
    RepoLinks::Web {
        template: project.config.repository_web.clone().unwrap(),
        commits: [("src/launcher.rs".to_owned(), "c0de".to_owned())]
            .into_iter()
            .collect(),
        fallback: "9f8e7d6c5b4a".into(),
    }
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

fn files_dump(files: &[(String, Vec<u8>)]) -> String {
    files
        .iter()
        .map(|(p, b)| match std::str::from_utf8(b) {
            Ok(t) => format!("==> {p} <==\n{t}"),
            Err(_) => format!("==> {p} <== ({} bytes)\n", b.len()),
        })
        .collect()
}

/// A readable dump of a document tree: sections with numbers and kinds,
/// blocks by kind.
fn dump(nodes: &[Node], indent: usize, out: &mut String) {
    for n in nodes {
        let pad = "  ".repeat(indent);
        match n {
            Node::Section(s) => {
                let kind = match &s.kind {
                    SectionKind::Heading => "heading".to_owned(),
                    SectionKind::Item(id) => format!("item {id}"),
                    SectionKind::Report(r) => format!("report {r}"),
                };
                out.push_str(&format!(
                    "{pad}§ [{}] depth {} {kind}{}: {}\n",
                    s.number,
                    s.depth,
                    if s.own_page { " own-page" } else { "" },
                    kiln_render::plain(&s.title)
                ));
                dump(&s.nodes, indent + 1, out);
            }
            Node::Block(b) => {
                let text = format!("{b:?}");
                let short: String = text.chars().take(100).collect();
                out.push_str(&format!("{pad}- {short}\n"));
            }
        }
    }
}

const TARGETS: &str = r#"
[[export]]
name = "doc"
format = "markdown"
title = "Flint — design document"
template = "design/templates/doc.md"
path = "out/doc.md"

[[export]]
name = "plain"
format = "markdown"
path = "out/plain.md"
reports = ["traceability", "roadmap", "risks", "findings"]

[[export]]
name = "sections"
format = "markdown"
layout = "sections"
template = "design/templates/doc.md"
path = "out/sections"

[[export]]
name = "tree"
format = "markdown"
layout = "tree"
types = ["requirement", "decision"]
path = "out/tree"
link_to = ["doc"]

[[export]]
name = "github"
format = "markdown"
template = "design/templates/doc.md"
path = "out/github.md"
[export.markdown]
flavour = "github"
diagrams = "image"

[[export]]
name = "commonmark"
format = "markdown"
path = "out/cm.md"
reports = []
item_style = "table"
[export.markdown]
flavour = "commonmark"

[[export]]
name = "html"
format = "html"
template = "design/templates/doc.md"
path = "out/doc.html"
"#;

#[test]
fn assembly_places_each_item_once_nested_and_numbered() {
    let (project, source) = project(TARGETS);
    let rev = revision();
    let a = Assembly {
        project: &project,
        findings: &[],
        revision: &rev,
        since: None,
    };
    let template = String::from_utf8(
        kiln_index::FileSource::read(&source, "design/templates/doc.md").unwrap(),
    )
    .unwrap();
    let with = crate::assemble(&target(&project, "doc"), Some(&template), &a);
    let mut out = String::new();
    dump(&with.document.nodes, 0, &mut out);
    out.push_str(&format!(
        "left out: {:?}\n",
        with.left_out
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    ));
    golden("assembly-template.txt", &out);

    let without = crate::assemble(&target(&project, "plain"), None, &a);
    let mut out = String::new();
    dump(&without.document.nodes, 0, &mut out);
    out.push_str(&format!(
        "left out: {:?}\n",
        without
            .left_out
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    ));
    golden("assembly-default.txt", &out);
}

#[test]
fn layouts_split_the_same_document() {
    let (project, source) = project(TARGETS);
    let mut out = String::new();
    for name in ["doc", "sections", "tree"] {
        let p = &prepared(&project, &source, &[name])[0];
        out.push_str(&format!("== {name} ({:?})\n", p.target.layout));
        for page in &p.split.pages {
            let items: Vec<String> = page.items().iter().map(ToString::to_string).collect();
            out.push_str(&format!(
                "{} | {} | parent {:?} | base {} | items {:?}\n",
                page.key, page.title, page.parent, page.base_depth, items
            ));
        }
    }
    golden("layouts.txt", &out);
    assert_eq!(
        prepared(&project, &source, &["tree"])[0].target.layout,
        Layout::Tree
    );
}

#[test]
// verifies: REQ-002
fn markdown_golden_per_layout_and_flavour() {
    let (project, source) = project(TARGETS);
    let all = prepared(
        &project,
        &source,
        &["doc", "plain", "sections", "tree", "github", "commonmark"],
    );
    for p in &all {
        let files = render_files(
            p,
            &all,
            &web(&project),
            project.config.jira_url.as_deref(),
            &FakePictures,
            &source,
        );
        golden(
            &format!("markdown-{}.md", p.target.name),
            &files_dump(&files),
        );
    }
}

#[test]
// verifies: REQ-002
fn html_is_self_contained_and_golden() {
    let (project, source) = project(TARGETS);
    let all = prepared(&project, &source, &["html"]);
    let files = render_files(
        &all[0],
        &all,
        &web(&project),
        project.config.jira_url.as_deref(),
        &FakePictures,
        &source,
    );
    let html = String::from_utf8(files[0].1.clone()).unwrap();
    for forbidden in [
        "<script",
        "<link",
        "src=\"http",
        "src=\"//",
        "@import",
        "url(",
    ] {
        assert!(!html.contains(forbidden), "HTML holds {forbidden}");
    }
    assert!(html.contains("src=\"data:image/png;base64,"));
    golden("html-doc.html", &html);
}

#[test]
fn exporting_twice_gives_the_same_bytes() {
    let (project, source) = project(TARGETS);
    let names = ["doc", "sections", "tree", "html"];
    let render = || {
        let all = prepared(&project, &source, &names);
        all.iter()
            .flat_map(|p| render_files(p, &all, &web(&project), None, &FakePictures, &source))
            .collect::<Vec<_>>()
    };
    assert_eq!(render(), render());
}

#[test]
// verifies: REQ-002
fn in_tree_exports_link_relatively_and_name_no_commit() {
    let (project, source) = project(TARGETS);
    let rev = Revision {
        in_tree: true,
        ..revision()
    };
    let a = Assembly {
        project: &project,
        findings: &[],
        revision: &rev,
        since: None,
    };
    let t = target(&project, "plain");
    let all = vec![prepare(&t, None, &a)];
    let mut resolved = all.clone();
    resolve_all(&mut resolved, &all, &project);
    let files = render_files(
        &resolved[0],
        &resolved,
        &RepoLinks::Relative,
        None,
        &FakePictures,
        &source,
    );
    let text = String::from_utf8(files[0].1.clone()).unwrap();
    assert!(
        text.contains("Generated by Kiln from the files in this repository."),
        "{text}"
    );
    assert!(!text.contains("9f8e7d6"), "{text}");
    assert!(
        text.contains("[src/launcher.rs:2](../src/launcher.rs#L2)"),
        "{text}"
    );
    let _ = REVISION;
}

#[test]
fn file_plans_write_only_changes_and_remove_what_kiln_wrote() {
    let (project, mut source) = project(TARGETS);
    let all = prepared(&project, &source, &["sections"]);
    let p = &all[0];
    let files = render_files(p, &all, &RepoLinks::None, None, &FakePictures, &source);
    let first = plan_files(&p.target, files.clone(), &source);
    assert_eq!(first.writes.len(), files.len());
    assert!(first.writes.iter().all(|(_, _, new)| *new));
    let (manifest, list) = first.manifest.clone().unwrap();
    assert_eq!(manifest, "out/sections/.kiln-export");
    // As if written, plus a stale page from an earlier export and a file
    // Kiln didn't write.
    for (path, bytes) in &files {
        source.insert(path, bytes);
    }
    let mut list = String::from_utf8(list.unwrap()).unwrap();
    list.push_str("sections\tout/sections/gone.md\n");
    source.insert(&manifest, list);
    source.insert("out/sections/gone.md", "old");
    source.insert("out/sections/mine.md", "not Kiln's");
    let second = plan_files(&p.target, files, &source);
    assert!(second.writes.is_empty());
    assert_eq!(second.removes, ["out/sections/gone.md"]);
    assert!(second.manifest.is_some(), "the stale entry leaves the list");
    assert!(second.changes());
}

#[test]
fn since_marks_new_and_changed_items_next_to_their_heading() {
    use kiln_index::{ChangeKind, ItemChange};
    let (project, source) = project(TARGETS);
    let change = |id: &str, kind| ItemChange {
        id: id.parse().unwrap(),
        kind,
        title: String::new(),
        type_name: "requirement".into(),
        path: String::new(),
        details: Vec::new(),
    };
    let since = kiln_render::Since {
        label: "v1.0".into(),
        commit: "1a2b3c4d5e6f".into(),
        changes: vec![
            change("REQ-002", ChangeKind::Changed),
            change("REQ-003", ChangeKind::Added),
        ],
    };
    let rev = revision();
    let a = Assembly {
        project: &project,
        findings: &[],
        revision: &rev,
        since: Some(&since),
    };
    let t = target(&project, "html");
    let template = String::from_utf8(
        kiln_index::FileSource::read(&source, "design/templates/doc.md").unwrap(),
    )
    .unwrap();
    let all = vec![prepare(&t, Some(&template), &a)];
    let mut resolved = all.clone();
    resolve_all(&mut resolved, &all, &project);
    let files = render_files(
        &resolved[0],
        &resolved,
        &RepoLinks::None,
        None,
        &FakePictures,
        &source,
    );
    let html = String::from_utf8(files[0].1.clone()).unwrap();
    assert!(
        html.contains(
            "REQ-002 Fast enough to be a reflex <span class=\"status yellow\">Changed</span></h"
        ),
        "{html}"
    );
    assert!(
        html.contains("REQ-003 Themes follow Fenix <span class=\"status green\">New</span></h"),
        "{html}"
    );
    assert!(
        !html.contains("REQ-001 Flint is a launcher <span"),
        "unchanged: no marker"
    );
    // Anchors and titles don't move with a baseline.
    let without = prepared(&project, &source, &["html"]);
    let ids = |p: &Prepared| {
        let mut out = Vec::new();
        fn walk(nodes: &[Node], out: &mut Vec<String>) {
            for n in nodes {
                if let Node::Section(s) = n {
                    out.push(s.heading_text());
                    walk(&s.nodes, out);
                }
            }
        }
        for page in &p.split.pages {
            walk(&page.nodes, &mut out);
        }
        out
    };
    assert_eq!(ids(&resolved[0]), ids(&without[0]));
}
