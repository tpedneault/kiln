//! Template validation (K112): every directive known, every attribute
//! known, every type, status and ID real (EXPORT.md, "Templates"). Run by
//! `kiln check`, so a broken template fails the merge request, not the
//! publish after it.

use kiln_index::Project;
use regex::Regex;

use crate::body::{render_markdown, BodyContext};
use crate::doc::{Block, Directive};

/// A problem on a template line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateProblem {
    pub line: u32,
    pub message: String,
}

const REPORT_NAMES: &[&str] = &["traceability", "findings", "roadmap", "risks", "changes"];
const VALUES: &[&str] = &["name", "commit", "date", "since"];

/// Problems in a template's text, with lines counted from 1.
pub fn template_problems(text: &str, project: &Project) -> Vec<TemplateProblem> {
    let prefixes = project.config.prefixes();
    let ctx = BodyContext {
        dir: "",
        first_line: 1,
        prefixes: &prefixes,
        link_bare_ids: false,
        template: true,
    };
    let mut out = Vec::new();
    visit(&render_markdown(text, &ctx), project, &mut out);
    let value = Regex::new(r"<!--\s*kiln:value\b(.*?)-->").expect("a valid literal regex");
    for (n, line) in text.lines().enumerate() {
        for caps in value.captures_iter(line) {
            let name = caps[1].trim();
            if !VALUES.contains(&name) {
                out.push(problem(
                    n as u32 + 1,
                    format!("kiln:value `{name}` isn't name, commit, date or since"),
                ));
            }
        }
    }
    out.sort_by_key(|p| p.line);
    out
}

fn problem(line: u32, message: String) -> TemplateProblem {
    TemplateProblem { line, message }
}

fn visit(blocks: &[Block], project: &Project, out: &mut Vec<TemplateProblem>) {
    for block in blocks {
        match block {
            Block::Directive(d) => directive(d, project, out),
            Block::Quote(inner) | Block::Callout { content: inner, .. } => {
                visit(inner, project, out)
            }
            Block::List { items, .. } => items.iter().for_each(|i| visit(&i.content, project, out)),
            _ => {}
        }
    }
}

fn directive(d: &Directive, project: &Project, out: &mut Vec<TemplateProblem>) {
    let config = &project.config;
    let unknown_attrs = |allowed: &[&str], out: &mut Vec<TemplateProblem>| {
        for (k, _) in &d.attrs {
            if !allowed.contains(&k.as_str()) {
                out.push(problem(
                    d.line,
                    format!(
                        "kiln:{} has no attribute `{k}` ({})",
                        d.name,
                        allowed.join(", ")
                    ),
                ));
            }
        }
    };
    let no_positional = |out: &mut Vec<TemplateProblem>| {
        if let Some(p) = d.positional.first() {
            out.push(problem(
                d.line,
                format!(
                    "kiln:{} takes no `{p}`; attributes are written key=\"value\"",
                    d.name
                ),
            ));
        }
    };
    let check_types = |value: &str, out: &mut Vec<TemplateProblem>| {
        for t in value.split(',').map(str::trim) {
            if config.type_named(t).is_none() {
                out.push(problem(d.line, format!("`{t}` isn't a type")));
            }
        }
    };
    match d.name.as_str() {
        "items" => {
            no_positional(out);
            unknown_attrs(
                &["type", "status", "tag", "root", "jira", "style", "split"],
                out,
            );
            if let Some(t) = d.attr("type") {
                check_types(t, out);
            }
            if let Some(statuses) = d.attr("status") {
                for s in statuses
                    .split(',')
                    .map(|s| s.trim().trim_start_matches('!'))
                {
                    if !config
                        .types
                        .iter()
                        .any(|t| t.statuses.iter().any(|x| x == s))
                    {
                        out.push(problem(d.line, format!("`{s}` isn't a status of any type")));
                    }
                }
            }
            if let Some(root) = d.attr("root") {
                match root.parse() {
                    Ok(id) if project.item(&id).is_some() => {}
                    _ => out.push(problem(d.line, format!("root `{root}` isn't an item"))),
                }
            }
            if let Some(style) = d.attr("style") {
                if style != "full" && style != "row" {
                    out.push(problem(
                        d.line,
                        format!("style `{style}` isn't full or row"),
                    ));
                }
            }
            if let Some(split) = d.attr("split") {
                if split != "auto" && split != "no" {
                    out.push(problem(d.line, format!("split `{split}` isn't auto or no")));
                }
            }
        }
        "item" => {
            unknown_attrs(&[], out);
            match d.positional.as_slice() {
                [id] => match id.parse() {
                    Ok(id) if project.item(&id).is_some() => {}
                    _ => out.push(problem(d.line, format!("kiln:item `{id}` isn't an item"))),
                },
                _ => out.push(problem(d.line, "kiln:item takes one item ID".into())),
            }
        }
        "report" => {
            match d.positional.as_slice() {
                [name] if REPORT_NAMES.contains(&name.as_str()) => {}
                _ => out.push(problem(
                    d.line,
                    format!("kiln:report takes one of {}", REPORT_NAMES.join(", ")),
                )),
            }
            if d.positional.first().map(String::as_str) == Some("traceability") {
                unknown_attrs(&["type"], out);
                if let Some(t) = d.attr("type") {
                    check_types(t, out);
                }
            } else {
                unknown_attrs(&[], out);
            }
        }
        "summary" | "toc" => {
            no_positional(out);
            unknown_attrs(&[], out);
        }
        "value" => out.push(problem(
            d.line,
            "kiln:value goes inside a paragraph, not on a line of its own".into(),
        )),
        other => out.push(problem(
            d.line,
            format!("unknown directive kiln:{other} (items, item, report, summary, toc, value)"),
        )),
    }
}
