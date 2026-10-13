//! The rules that need the configuration and the whole project. Rules a
//! single file can decide (K100, K104 for keys and value types, K001–K004)
//! are reported by `kiln-parse` while loading and arrive here in
//! `Project::findings`.
//!
//! Some rules name statuses and types from FORMAT.md's defaults on
//! purpose (`requirement`, `verified`, `passing`, `superseded`,
//! `milestone`): they are what the rule is about, and a project with other
//! names simply doesn't trigger them.

use std::collections::HashSet;

use kiln_index::{FileSource, Project, Source};
use kiln_model::{Date, Finding, IdError, Item, ItemId, Ref, Rule, CODE};

/// Every finding for `project`, at default severities (overrides are
/// applied by the caller), unsorted.
pub fn findings(project: &Project, files: &dyn FileSource, today: Date) -> Vec<Finding> {
    let mut out = project.findings.clone();
    duplicates(project, &mut out);
    for item in &project.items {
        item_rules(project, files, item, &mut out);
    }
    parent_cycles(project, &mut out);
    code_rules(project, &mut out);
    coverage_rules(project, &mut out);
    status_rules(project, today, &mut out);
    diagram_rules(project, &mut out);
    template_rules(project, files, &mut out);
    out
}

/// K111: a Mermaid block that doesn't draw, on its own line.
fn diagram_rules(project: &Project, out: &mut Vec<Finding>) {
    for item in &project.items {
        for (source, span) in &item.diagrams {
            if let Some((line, column, message)) = kiln_render::diagram_error(source) {
                let at = kiln_model::Span {
                    start: span.start,
                    end: span.end,
                    line: span.line + line,
                    column: column + 1,
                };
                report(
                    out,
                    Rule::K111,
                    &item.path,
                    at,
                    format!("diagram: {message}"),
                );
            }
        }
    }
}

/// K112: each export target's template exists and its directives are
/// valid, and a Word target's reference document exists. A template shared by several targets is checked once.
fn template_rules(project: &Project, files: &dyn FileSource, out: &mut Vec<Finding>) {
    let mut seen = HashSet::new();
    for target in &project.config.exports {
        // A Word reference document is a template too: it must exist and
        // be a zip (a `.docx` is one); its styles are read at export.
        if let Some(path) = &target.docx.reference {
            if seen.insert(path.clone())
                && !files.read(path).is_ok_and(|b| b.starts_with(b"PK\x03\x04"))
            {
                report(
                    out,
                    Rule::K112,
                    kiln_parse::CONFIG_FILE,
                    kiln_model::Span {
                        start: 0,
                        end: 0,
                        line: 1,
                        column: 1,
                    },
                    format!(
                        "export `{}`: reference {path} doesn't exist or isn't a .docx",
                        target.name
                    ),
                );
            }
        }
        let Some(path) = &target.template else {
            continue;
        };
        if !seen.insert(path.clone()) {
            continue;
        }
        let at_line = |line: u32| kiln_model::Span {
            start: 0,
            end: 0,
            line,
            column: 1,
        };
        let text = match files
            .read(path)
            .ok()
            .and_then(|b| kiln_parse::normalize(&b))
        {
            Some(text) => text,
            None => {
                report(
                    out,
                    Rule::K112,
                    kiln_parse::CONFIG_FILE,
                    at_line(1),
                    format!(
                        "export `{}`: template {path} doesn't exist or isn't UTF-8",
                        target.name
                    ),
                );
                continue;
            }
        };
        for problem in kiln_render::template_problems(&text, project) {
            report(
                out,
                Rule::K112,
                path,
                at_line(problem.line),
                problem.message,
            );
        }
    }
}

fn report(out: &mut Vec<Finding>, rule: Rule, path: &str, span: kiln_model::Span, message: String) {
    out.push(Finding::new(rule, path, span, message));
}

/// K102 for a reference that isn't an ID or names nothing; `None` then,
/// else the target item.
fn resolve<'p>(
    project: &'p Project,
    path: &str,
    r: &Ref,
    what: &str,
    out: &mut Vec<Finding>,
) -> Option<&'p Item> {
    match &r.id {
        Err(IdError::Spelling(id)) => {
            report(
                out,
                Rule::K102,
                path,
                r.span,
                format!("{what} `{}` isn't how IDs are written: {id}", r.raw),
            );
            None
        }
        Err(e) => {
            report(
                out,
                Rule::K102,
                path,
                r.span,
                format!("{what} `{}` isn't an ID: {e}", r.raw),
            );
            None
        }
        Ok(id) => {
            let found = project.item(id);
            if found.is_none() {
                report(
                    out,
                    Rule::K102,
                    path,
                    r.span,
                    format!("{what} {id} doesn't exist"),
                );
            }
            found
        }
    }
}

/// K101, on every copy, naming the other files.
// implements: REQ-015
fn duplicates(project: &Project, out: &mut Vec<Finding>) {
    let mut ids: Vec<&ItemId> = project.ids().collect();
    ids.sort();
    for id in ids {
        let copies = project.indices(id);
        if copies.len() < 2 {
            continue;
        }
        for &i in copies {
            let item = &project.items[i];
            let others: Vec<&str> = copies
                .iter()
                .filter(|&&j| j != i)
                .map(|&j| project.items[j].path.as_str())
                .collect();
            report(
                out,
                Rule::K101,
                &item.path,
                item.id_span,
                format!("{id} is also the ID of {}; `kiln renumber {id} --to {} --file {}` moves this one", others.join(", "), project.next_id(id.prefix()), item.path),
            );
        }
    }
}

fn item_rules(project: &Project, files: &dyn FileSource, item: &Item, out: &mut Vec<Finding>) {
    let config = &project.config;
    let path = item.path.as_str();
    let file_name = path.rsplit('/').next().unwrap_or(path);
    if !file_name.starts_with(&item.id.to_string()) {
        report(
            out,
            Rule::K105,
            path,
            item.id_span,
            format!("the file name doesn't start with {}", item.id),
        );
    }

    let ty = if item.type_name.is_empty() {
        None
    } else {
        let ty = config.type_named(&item.type_name);
        if ty.is_none() {
            let names: Vec<&str> = config.types.iter().map(|t| t.name.as_str()).collect();
            report(
                out,
                Rule::K104,
                path,
                item.type_span,
                format!(
                    "type `{}` isn't declared in kiln.toml ({})",
                    item.type_name,
                    names.join(", ")
                ),
            );
        }
        ty
    };
    if let Some(ty) = ty {
        if item.id.prefix() != ty.prefix {
            report(
                out,
                Rule::K106,
                path,
                item.id_span,
                format!(
                    "{} has prefix {}, but {} items use {}",
                    item.id,
                    item.id.prefix(),
                    ty.name,
                    ty.prefix
                ),
            );
        }
        if !item.status.is_empty() && !ty.statuses.contains(&item.status) {
            report(
                out,
                Rule::K107,
                path,
                item.status_span,
                format!(
                    "status `{}` isn't one of {}'s: {}",
                    item.status,
                    ty.name,
                    ty.statuses.join(", ")
                ),
            );
        }
    }

    if let Some(parent) = &item.parent {
        if let Some(target) = resolve(project, path, parent, "parent", out) {
            if let Some(ty) = ty {
                if !target.type_name.is_empty() && !ty.parents.contains(&target.type_name) {
                    let allowed = if ty.parents.is_empty() {
                        "none".to_owned()
                    } else {
                        ty.parents.join(", ")
                    };
                    report(
                        out,
                        Rule::K108,
                        path,
                        parent.span,
                        format!(
                            "a {} can't have a {} as parent (allowed: {allowed})",
                            ty.name, target.type_name
                        ),
                    );
                }
            }
        }
    }

    let closed = project.is_closed(item);
    let mut reported_roles = HashSet::new();
    for link in &item.links {
        let Some(role) = config.role(&link.role) else {
            if reported_roles.insert(link.role.as_str()) {
                match config.role_by_inverse_key(&link.role) {
                    Some(inverse) => report(out, Rule::K109, path, link.role_span, format!("`{}` is how `{}` reads from the other side; write `{} = [\"{}\"]` in the other item", link.role, inverse.name, inverse.name, item.id)),
                    None => report(out, Rule::K108, path, link.role_span, format!("`{}` isn't a link role in kiln.toml", link.role)),
                }
            }
            continue;
        };
        if let Some(ty) = ty {
            if !role.allows_from(&ty.name) {
                if reported_roles.insert(link.role.as_str()) {
                    report(
                        out,
                        Rule::K108,
                        path,
                        link.role_span,
                        format!(
                            "a {} can't be the source of `{}` (from: {})",
                            ty.name,
                            role.name,
                            role.from.join(", ")
                        ),
                    );
                }
                continue;
            }
        }
        let Some(target) = resolve(project, path, &link.target, &role.name, out) else {
            continue;
        };
        if !target.type_name.is_empty() && !role.allows_to(&target.type_name) {
            report(
                out,
                Rule::K108,
                path,
                link.target.span,
                format!(
                    "`{}` can't point at a {} ({}; to: {})",
                    role.name,
                    target.type_name,
                    target.id,
                    role.to.join(", ")
                ),
            );
        } else if !closed && project.is_closed(target) {
            report(
                out,
                Rule::K203,
                path,
                link.target.span,
                format!(
                    "{} links to {}, which is {}",
                    item.id, target.id, target.status
                ),
            );
        }
    }

    for mention in &item.mentions {
        resolve(project, path, &mention.target, "reference", out);
    }

    let dir = path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    for link in &item.relative_links {
        match resolve_relative(dir, &link.dest) {
            None => report(
                out,
                Rule::K110,
                path,
                link.span,
                format!("`{}` points outside the project", link.dest),
            ),
            Some(target) if !files.exists(&target) => report(
                out,
                Rule::K110,
                path,
                link.span,
                format!("`{}` doesn't exist ({target})", link.dest),
            ),
            Some(_) => {}
        }
    }
}

/// `dest` from a file in `dir`, as a project-relative path; `/x` is from
/// the project root. `None` when it climbs out of the project.
pub fn resolve_relative(dir: &str, dest: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    let (base, dest) = match dest.strip_prefix('/') {
        Some(rest) => ("", rest),
        None => (dir, dest),
    };
    for part in base.split('/').chain(dest.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    Some(if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    })
}

/// K103, on each item whose parent chain comes back to it.
fn parent_cycles(project: &Project, out: &mut Vec<Finding>) {
    for item in &project.items {
        let Some(parent) = &item.parent else { continue };
        let mut chain = vec![item.id.to_string()];
        let mut seen = HashSet::from([item.id.clone()]);
        let mut at = parent.id.as_ref().ok().cloned();
        while let Some(id) = at {
            chain.push(id.to_string());
            if id == item.id {
                report(
                    out,
                    Rule::K103,
                    &item.path,
                    parent.span,
                    format!("parent cycle: {}", chain.join(" → ")),
                );
                break;
            }
            if !seen.insert(id.clone()) {
                break;
            }
            at = project
                .item(&id)
                .and_then(|p| p.parent.as_ref())
                .and_then(|p| p.id.as_ref().ok().cloned());
        }
    }
}

fn code_rules(project: &Project, out: &mut Vec<Finding>) {
    let config = &project.config;
    let code_roles: Vec<&str> = config
        .links
        .iter()
        .filter(|r| r.allows_from(CODE))
        .map(|r| r.name.as_str())
        .collect();
    for code in &project.code_refs {
        let role = config.role(&code.role).filter(|r| r.allows_from(CODE));
        let Some(role) = role else {
            report(
                out,
                Rule::K108,
                &code.path,
                code.role_span,
                format!(
                    "`{}` isn't a link role code can use ({})",
                    code.role,
                    code_roles.join(", ")
                ),
            );
            continue;
        };
        let Some(target) = resolve(project, &code.path, &code.target, &role.name, out) else {
            continue;
        };
        if !target.type_name.is_empty() && !role.allows_to(&target.type_name) {
            report(
                out,
                Rule::K108,
                &code.path,
                code.target.span,
                format!(
                    "`{}` can't point at a {} ({}; to: {})",
                    role.name,
                    target.type_name,
                    target.id,
                    role.to.join(", ")
                ),
            );
        }
    }
}

/// K201, K202: requirements past their initial status that nothing
/// verifies or implements.
fn coverage_rules(project: &Project, out: &mut Vec<Finding>) {
    let Some(ty) = project.config.type_named("requirement") else {
        return;
    };
    for item in project.items.iter().filter(|i| i.type_name == ty.name) {
        if item.status == ty.initial || ty.is_closed(&item.status) || item.status.is_empty() {
            continue;
        }
        for (rule, role, verb) in [
            (Rule::K201, "verifies", "verifies"),
            (Rule::K202, "implements", "implements"),
        ] {
            if project.config.role(role).is_some() && !project.covered(&item.id, role) {
                report(
                    out,
                    rule,
                    &item.path,
                    item.id_span,
                    format!("{} is {} but nothing {verb} it", item.id, item.status),
                );
            }
        }
    }
}

fn status_rules(project: &Project, today: Date, out: &mut Vec<Finding>) {
    for (i, item) in project.items.iter().enumerate() {
        let inbound = project.inbound(&item.id);
        if item.type_name == "decision"
            && item.status == "superseded"
            && !inbound.iter().any(|l| l.role == "supersedes")
        {
            report(
                out,
                Rule::K204,
                &item.path,
                item.status_span,
                format!("{} is superseded, but no decision `supersedes` it", item.id),
            );
        }
        if item.status == "verified" {
            let passing = inbound.iter().any(|l| {
                l.role == "verifies"
                    && match l.source {
                        Source::Code(_) => true,
                        Source::Item(j) => {
                            let s = &project.items[j];
                            j != i && s.type_name == "test" && s.status == "passing"
                        }
                    }
            });
            if !passing {
                report(
                    out,
                    Rule::K205,
                    &item.path,
                    item.status_span,
                    format!(
                        "{} is verified, but no passing test or code reference verifies it",
                        item.id
                    ),
                );
            }
        }
        if item.type_name == "milestone" {
            if let Some(due) = item.due {
                if due < today && !project.is_done(item) && !project.is_closed(item) {
                    report(
                        out,
                        Rule::K206,
                        &item.path,
                        item.id_span,
                        format!("{} was due {due} and is {}", item.id, item.status),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_resolve_inside_the_project() {
        assert_eq!(
            resolve_relative("design/req", "../img/a.png").as_deref(),
            Some("design/img/a.png")
        );
        assert_eq!(
            resolve_relative("design", "/docs/x.md").as_deref(),
            Some("docs/x.md")
        );
        assert_eq!(
            resolve_relative("design", "./a.md").as_deref(),
            Some("design/a.md")
        );
        assert_eq!(resolve_relative("design", "../../x"), None);
    }
}
