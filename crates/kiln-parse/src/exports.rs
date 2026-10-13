//! `[[export]]` and `[confluence]` (docs/EXPORT.md, "Targets";
//! docs/CONFLUENCE.md, "Configuration"): K005 for a target that can't
//! work, K003 for unknown keys. An invalid target is reported and left
//! out, so the others still export.

use std::collections::HashSet;

use kiln_model::{
    ConfluenceConfig, ConfluenceFlavour, ConfluenceTarget, Diagrams, DocxOptions, ExportFormat,
    ExportTarget, Flavour, HtmlOptions, ItemStyle, Layout, MarkdownOptions, OnDrift, Rule, Span,
    TypeDef, REPORTS,
};
use toml_edit::Item;

use crate::config::{confined, is_word};
use crate::toml_walk::Walk;

const TARGET_KEYS: &[&str] = &[
    "name",
    "format",
    "layout",
    "title",
    "template",
    "types",
    "exclude_status",
    "reports",
    "numbering",
    "item_style",
    "link_to",
    "path",
    "markdown",
    "html",
    "docx",
    "confluence",
];
const CONFLUENCE_KEYS: &[&str] = &[
    "url",
    "flavour",
    "space",
    "user",
    "auth",
    "labels",
    "jira_macro",
    "jira_server_id",
    "restrict_edits",
    "on_drift",
    "concurrency",
    "diagrams",
    "max_page_kib",
];

/// `[confluence]`. `None` when absent or missing `url` or `space` (the
/// targets that need it say so).
pub fn confluence(w: &mut Walk, item: Option<&Item>) -> Option<ConfluenceConfig> {
    let table = item.and_then(|i| w.table(i, Rule::K002, "`[confluence]`"))?;
    w.unknown_keys(table, CONFLUENCE_KEYS, Rule::K003, "in [confluence]");
    let string = |w: &mut Walk, key: &str| {
        table
            .get(key)
            .and_then(|i| w.string(i, Rule::K002, &format!("`confluence.{key}`")))
    };
    let url = string(w, "url");
    let space = string(w, "space");
    let user = string(w, "user").map(|(s, _)| s).unwrap_or_default();
    let jira_server_id = string(w, "jira_server_id")
        .map(|(s, _)| s)
        .unwrap_or_default();
    let auth = match string(w, "auth") {
        Some((s, _)) if ["auto", "bearer", "basic"].contains(&s.as_str()) => s,
        Some((s, span)) => {
            w.report(
                Rule::K002,
                span,
                format!("confluence.auth `{s}` isn't auto, bearer or basic"),
            );
            "auto".to_owned()
        }
        None => "auto".to_owned(),
    };
    let flavour = match string(w, "flavour") {
        None => ConfluenceFlavour::Auto,
        Some((s, span)) => match s.as_str() {
            "auto" => ConfluenceFlavour::Auto,
            "cloud" => ConfluenceFlavour::Cloud,
            "data-center" => ConfluenceFlavour::DataCenter,
            _ => {
                w.report(
                    Rule::K002,
                    span,
                    format!("confluence.flavour `{s}` isn't auto, cloud or data-center"),
                );
                ConfluenceFlavour::Auto
            }
        },
    };
    let on_drift = match string(w, "on_drift") {
        None => OnDrift::Stop,
        Some((s, span)) => match s.as_str() {
            "stop" => OnDrift::Stop,
            "skip" => OnDrift::Skip,
            "overwrite" => OnDrift::Overwrite,
            _ => {
                w.report(
                    Rule::K002,
                    span,
                    format!("confluence.on_drift `{s}` isn't stop, skip or overwrite"),
                );
                OnDrift::Stop
            }
        },
    };
    let diagrams = match string(w, "diagrams") {
        None => Diagrams::Image,
        Some((s, span)) => Diagrams::parse(&s).unwrap_or_else(|| {
            w.report(
                Rule::K002,
                span,
                format!("confluence.diagrams `{s}` isn't image or source"),
            );
            Diagrams::Image
        }),
    };
    let labels = table
        .get("labels")
        .and_then(|i| w.strings(i, Rule::K002, "`confluence.labels`"))
        .unwrap_or_default()
        .into_iter()
        .map(|(s, _)| s)
        .collect();
    let flag = |w: &mut Walk, key: &str, default: bool| {
        table
            .get(key)
            .and_then(|i| w.boolean(i, Rule::K002, &format!("`confluence.{key}`")))
            .unwrap_or(default)
    };
    let jira_macro = flag(w, "jira_macro", true);
    let restrict_edits = flag(w, "restrict_edits", false);
    let number = |w: &mut Walk, key: &str, default: u64, min: u64| match table.get(key) {
        None => default,
        Some(i) => match i.as_integer() {
            Some(n) if n >= min as i64 => n as u64,
            _ => {
                let span = w.item_span(i);
                w.report(
                    Rule::K002,
                    span,
                    format!("`confluence.{key}` must be a whole number of at least {min}"),
                );
                default
            }
        },
    };
    let concurrency = number(w, "concurrency", 4, 1) as u32;
    let max_page_kib = number(w, "max_page_kib", 1024, 1);
    let url = match url {
        Some((u, span)) if !(u.starts_with("https://") || u.starts_with("http://")) => {
            w.report(
                Rule::K002,
                span,
                format!("confluence.url `{u}` must start with https:// (or http://)"),
            );
            None
        }
        u => u.map(|(u, _)| u),
    };
    Some(ConfluenceConfig {
        url: url?,
        flavour,
        space: space?.0,
        user,
        auth,
        labels,
        jira_macro,
        jira_server_id,
        restrict_edits,
        on_drift,
        concurrency,
        diagrams,
        max_page_kib,
    })
}

/// Every `[[export]]`; `confluence_present` says whether `[confluence]`
/// is usable.
pub fn exports(
    w: &mut Walk,
    item: Option<&Item>,
    types: &[TypeDef],
    project_name: &str,
    confluence: Option<&ConfluenceConfig>,
) -> Vec<ExportTarget> {
    let Some(item) = item else { return Vec::new() };
    let Some(array) = item.as_array_of_tables() else {
        let span = w.item_span(item);
        w.report(Rule::K005, span, "exports are written as [[export]] tables");
        return Vec::new();
    };
    let mut out: Vec<(ExportTarget, Vec<(String, Span)>)> = Vec::new();
    let mut names = HashSet::new();
    for table in array.iter() {
        if let Some((target, link_to)) = target(w, table, types, project_name, confluence) {
            if !names.insert(target.name.clone()) {
                let span = w.span_of_key(table, "name");
                w.report(
                    Rule::K005,
                    span,
                    format!("export `{}` is declared twice", target.name),
                );
                continue;
            }
            out.push((target, link_to));
        }
    }
    // `link_to` may name a target declared later.
    let all: Vec<String> = out.iter().map(|(t, _)| t.name.clone()).collect();
    out.into_iter()
        .map(|(mut target, link_to)| {
            for (name, span) in link_to {
                if all.contains(&name) && name != target.name {
                    target.link_to.push(name);
                } else {
                    w.report(
                        Rule::K005,
                        span,
                        format!("link_to: `{name}` isn't another export target"),
                    );
                }
            }
            target
        })
        .collect()
}

fn target(
    w: &mut Walk,
    t: &toml_edit::Table,
    types: &[TypeDef],
    project_name: &str,
    confluence: Option<&ConfluenceConfig>,
) -> Option<(ExportTarget, Vec<(String, Span)>)> {
    let header = w.span(t.span());
    w.unknown_keys(t, TARGET_KEYS, Rule::K003, "in [[export]]");
    let invalid = |w: &mut Walk, span: Span, message: String| {
        w.report(Rule::K005, span, message);
    };
    let string = |w: &mut Walk, key: &str| {
        t.get(key)
            .and_then(|i| w.string(i, Rule::K005, &format!("`{key}`")))
    };
    let Some((name, name_span)) = string(w, "name") else {
        invalid(w, header, "an [[export]] needs a `name`".into());
        return None;
    };
    if !is_word(&name) {
        invalid(
            w,
            name_span,
            format!("export name `{name}` must be lowercase letters, digits and `-`"),
        );
        return None;
    }
    let format = match string(w, "format") {
        None => {
            invalid(
                w,
                name_span,
                format!("export `{name}` needs a `format` (confluence, markdown, html or docx)"),
            );
            return None;
        }
        Some((f, span)) => match ExportFormat::parse(&f) {
            Some(f) => f,
            None => {
                invalid(
                    w,
                    span,
                    format!("format `{f}` isn't confluence, markdown, html or docx"),
                );
                return None;
            }
        },
    };
    let layout = match string(w, "layout") {
        None => Layout::Document,
        Some((l, span)) => match Layout::parse(&l) {
            Some(l) => l,
            None => {
                invalid(
                    w,
                    span,
                    format!("layout `{l}` isn't document, sections or tree"),
                );
                return None;
            }
        },
    };
    let title = string(w, "title")
        .map(|(s, _)| s)
        .unwrap_or_else(|| project_name.to_owned());
    let template = match string(w, "template") {
        Some((p, span)) => match confined(&p) {
            Some(p) => Some(p),
            None => {
                invalid(
                    w,
                    span,
                    format!("template `{p}` must be relative and inside the project"),
                );
                return None;
            }
        },
        None => None,
    };
    let mut ok = true;
    let types_list = match t
        .get("types")
        .and_then(|i| w.strings(i, Rule::K005, "`types`"))
    {
        Some(list) => {
            let mut chosen = Vec::new();
            for (ty, span) in list {
                if types.iter().any(|d| d.name == ty) {
                    chosen.push(ty);
                } else {
                    invalid(w, span, format!("types: `{ty}` isn't a type"));
                    ok = false;
                }
            }
            chosen
        }
        None => types.iter().map(|t| t.name.clone()).collect(),
    };
    let exclude_status = t
        .get("exclude_status")
        .and_then(|i| w.strings(i, Rule::K005, "`exclude_status`"))
        .map(|list| {
            list.into_iter()
                .filter_map(|(s, span)| {
                    if types.iter().any(|t| t.statuses.contains(&s)) {
                        Some(s)
                    } else {
                        w.report(
                            Rule::K005,
                            span,
                            format!("exclude_status: `{s}` isn't a status of any type"),
                        );
                        None
                    }
                })
                .collect()
        });
    let reports = match t
        .get("reports")
        .and_then(|i| w.strings(i, Rule::K005, "`reports`"))
    {
        None => vec!["traceability".to_owned(), "findings".to_owned()],
        Some(list) => {
            let mut out = Vec::new();
            for (r, span) in list {
                if REPORTS.contains(&r.as_str()) {
                    out.push(r);
                } else {
                    invalid(
                        w,
                        span,
                        format!("report `{r}` isn't one of traceability, findings, roadmap, risks"),
                    );
                    ok = false;
                }
            }
            out
        }
    };
    let numbering = t
        .get("numbering")
        .and_then(|i| w.boolean(i, Rule::K005, "`numbering`"))
        .unwrap_or(true);
    let item_style = match string(w, "item_style") {
        None => None,
        Some((s, span)) => match ItemStyle::parse(&s) {
            Some(style) => Some(style),
            None => {
                invalid(
                    w,
                    span,
                    format!("item_style `{s}` isn't line, table or none"),
                );
                ok = false;
                None
            }
        },
    };
    let link_to = t
        .get("link_to")
        .and_then(|i| w.strings(i, Rule::K005, "`link_to`"))
        .unwrap_or_default();
    let path = match string(w, "path") {
        Some((p, span)) => match confined(&p) {
            Some(p) if p != "." => Some(p),
            _ => {
                invalid(
                    w,
                    span,
                    format!("path `{p}` must be a relative path inside the project"),
                );
                return None;
            }
        },
        None => None,
    };
    if format.is_file() && path.is_none() {
        invalid(
            w,
            name_span,
            format!(
                "export `{name}` writes {} files and needs a `path`",
                format.as_str()
            ),
        );
        return None;
    }
    if !format.is_file() && path.is_some() {
        let span = w.span_of_key(t, "path");
        invalid(w, span, "`path` is for file formats; a Confluence target sets [export.confluence] parent or page".into());
        ok = false;
    }

    let markdown = markdown_options(w, t.get("markdown"));
    let html = html_options(w, t.get("html"));
    let docx = docx_options(w, t.get("docx"));
    let target_confluence = confluence_target(w, t.get("confluence"));
    if target_confluence.page.is_some() && layout != Layout::Document {
        let span = w.span_of_key(t, "confluence");
        invalid(w, span, "[export.confluence] page = … writes one existing page, so it needs layout = \"document\"".into());
        ok = false;
    }
    if format == ExportFormat::Confluence {
        let mut missing = Vec::new();
        if confluence.is_none() {
            missing.push("[confluence] url and space");
        }
        if target_confluence.parent.is_none() && target_confluence.page.is_none() {
            missing.push("[export.confluence] parent or page");
        }
        if !missing.is_empty() {
            invalid(
                w,
                name_span,
                format!("Confluence target `{name}` needs {}", missing.join(" and ")),
            );
            ok = false;
        }
    }
    if !ok {
        return None;
    }
    Some((
        ExportTarget {
            name,
            format,
            layout,
            title,
            template,
            types: types_list,
            exclude_status,
            reports,
            numbering,
            item_style,
            link_to: Vec::new(),
            path,
            markdown,
            html,
            docx,
            confluence: target_confluence,
        },
        link_to,
    ))
}

fn markdown_options(w: &mut Walk, item: Option<&Item>) -> MarkdownOptions {
    let mut out = MarkdownOptions::default();
    let Some(t) = item.and_then(|i| w.table(i, Rule::K005, "`[export.markdown]`")) else {
        return out;
    };
    w.unknown_keys(
        t,
        &["flavour", "diagrams"],
        Rule::K003,
        "in [export.markdown]",
    );
    if let Some((s, span)) = t
        .get("flavour")
        .and_then(|i| w.string(i, Rule::K005, "`flavour`"))
    {
        match Flavour::parse(&s) {
            Some(f) => out.flavour = f,
            None => w.report(
                Rule::K005,
                span,
                format!("flavour `{s}` isn't gitlab, github or commonmark"),
            ),
        }
    }
    if let Some((s, span)) = t
        .get("diagrams")
        .and_then(|i| w.string(i, Rule::K005, "`diagrams`"))
    {
        match Diagrams::parse(&s) {
            Some(d) => out.diagrams = d,
            None => w.report(
                Rule::K005,
                span,
                format!("diagrams `{s}` isn't source or image"),
            ),
        }
    }
    out
}

fn html_options(w: &mut Walk, item: Option<&Item>) -> HtmlOptions {
    let Some(t) = item.and_then(|i| w.table(i, Rule::K005, "`[export.html]`")) else {
        return HtmlOptions::default();
    };
    w.unknown_keys(t, &["theme"], Rule::K003, "in [export.html]");
    let theme = t
        .get("theme")
        .and_then(|i| w.string(i, Rule::K005, "`theme`"));
    if let Some((name, span)) = &theme {
        if name != "light" {
            w.report(
                Rule::K005,
                *span,
                format!("theme `{name}`: only the built-in `light` theme exists so far"),
            );
        }
    }
    HtmlOptions {
        theme: theme.map(|(s, _)| s),
    }
}

fn docx_options(w: &mut Walk, item: Option<&Item>) -> DocxOptions {
    let Some(t) = item.and_then(|i| w.table(i, Rule::K005, "`[export.docx]`")) else {
        return DocxOptions::default();
    };
    w.unknown_keys(t, &["reference"], Rule::K003, "in [export.docx]");
    let reference = t
        .get("reference")
        .and_then(|i| w.string(i, Rule::K005, "`reference`"));
    if let Some((path, span)) = &reference {
        if !path.to_ascii_lowercase().ends_with(".docx") {
            w.report(
                Rule::K005,
                *span,
                format!("reference `{path}` isn't a .docx file"),
            );
        }
    }
    DocxOptions {
        reference: reference.map(|(s, _)| s),
    }
}

fn confluence_target(w: &mut Walk, item: Option<&Item>) -> ConfluenceTarget {
    let mut out = ConfluenceTarget::default();
    let Some(t) = item.and_then(|i| w.table(i, Rule::K005, "`[export.confluence]`")) else {
        return out;
    };
    w.unknown_keys(
        t,
        &[
            "parent",
            "page",
            "space",
            "labels",
            "toc_levels",
            "restrict_adopted",
        ],
        Rule::K003,
        "in [export.confluence]",
    );
    let id = |w: &mut Walk, key: &str| match t.get(key) {
        None => None,
        Some(i) => match i.as_integer() {
            Some(n) if n > 0 => Some(n as u64),
            _ => {
                let span = w.item_span(i);
                w.report(
                    Rule::K005,
                    span,
                    format!("`{key}` must be a page id (a positive whole number)"),
                );
                None
            }
        },
    };
    out.parent = id(w, "parent");
    out.page = id(w, "page");
    if out.parent.is_some() && out.page.is_some() {
        let span = w.span_of_key(t, "page");
        w.report(Rule::K005, span, "set `parent` or `page`, not both");
    }
    out.space = t
        .get("space")
        .and_then(|i| w.string(i, Rule::K005, "`space`"))
        .map(|(s, _)| s);
    out.labels = t
        .get("labels")
        .and_then(|i| w.strings(i, Rule::K005, "`labels`"))
        .unwrap_or_default()
        .into_iter()
        .map(|(s, _)| s)
        .collect();
    if let Some(i) = t.get("toc_levels") {
        match i.as_integer() {
            Some(n @ 1..=6) => out.toc_levels = Some(n as u8),
            _ => {
                let span = w.item_span(i);
                w.report(Rule::K005, span, "`toc_levels` must be 1 to 6");
            }
        }
    }
    out.restrict_adopted = t
        .get("restrict_adopted")
        .and_then(|i| w.boolean(i, Rule::K005, "`restrict_adopted`"))
        .unwrap_or(false);
    out
}

#[cfg(test)]
mod tests {
    use crate::parse_config;
    use kiln_model::{ExportFormat, Finding, Flavour, ItemStyle, Layout};

    fn parse(extra: &str) -> (kiln_model::Config, Vec<String>) {
        let text = format!("{}{extra}", kiln_model::default_config("Flint"));
        let base = kiln_model::default_config("Flint").lines().count() as u32;
        let (config, findings) = parse_config(&text);
        let codes = findings
            .iter()
            .map(|f: &Finding| format!("{}@{}", f.rule, f.line - base))
            .collect();
        (config.unwrap(), codes)
    }

    #[test]
    fn exports_from_export_md_parse_with_defaults() {
        let (config, codes) = parse(
            r#"
[confluence]
url = "https://confluence.example.com/"
space = "FLINT"

[[export]]
name = "spec"
format = "confluence"
title = "Flint — design document"
link_to = ["spec-md"]
[export.confluence]
parent = 123456789

[[export]]
name = "design-tree"
format = "confluence"
layout = "tree"
types = ["design", "decision"]
[export.confluence]
parent = 123456789

[[export]]
name = "spec-md"
format = "markdown"
path = "docs/generated/DESIGN.md"
[export.markdown]
flavour = "github"
"#,
        );
        assert_eq!(codes, Vec::<String>::new());
        assert_eq!(
            config.confluence.as_ref().unwrap().base_url(),
            "https://confluence.example.com"
        );
        let spec = config.export("spec").unwrap();
        assert_eq!(
            (spec.format, spec.layout, spec.item_style()),
            (ExportFormat::Confluence, Layout::Document, ItemStyle::Line)
        );
        assert_eq!(spec.link_to, ["spec-md"]);
        assert_eq!(spec.types.len(), 7);
        assert_eq!(spec.reports, ["traceability", "findings"]);
        assert_eq!(
            config.export("design-tree").unwrap().item_style(),
            ItemStyle::Table
        );
        let md = config.export("spec-md").unwrap();
        assert_eq!(md.title, "Flint");
        assert_eq!(md.markdown.flavour, Flavour::Github);
    }

    #[test]
    fn invalid_targets_are_k005_and_left_out() {
        let (config, codes) = parse(
            r#"
[[export]]
name = "a"
format = "pdf"

[[export]]
name = "b"
format = "markdown"

[[export]]
name = "c"
format = "confluence"
layout = "tree"
link_to = ["nowhere"]
[export.confluence]
page = 5

[[export]]
name = "d"
format = "html"
path = "out/d.html"
reports = ["changes", "gossip"]

[[export]]
name = "e"
format = "html"
path = "out/e.html"
colour = "red"
link_to = ["nowhere"]

[[export]]
name = "e"
format = "html"
path = "out/e2.html"
"#,
        );
        assert_eq!(
            config
                .exports
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>(),
            ["e"]
        );
        assert_eq!(
            codes,
            [
                // `changes` is a report now; only `gossip` on line 22 isn't.
                "K005@4", "K005@7", "K005@15", "K005@11", "K005@22", "K003@28", "K005@32", "K005@29"
            ]
        );
    }
}
