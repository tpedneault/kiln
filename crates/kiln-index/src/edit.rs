//! The editing commands as functions: validating a front-matter value
//! (`kiln set`), a link (`kiln link`), and applying them, a renumbering
//! or a new item to the files on disk. Shared by the command line, the
//! GUI and the Flint provider, so a value one accepts the others accept,
//! and a file changed under any of them is refused the same way.
//!
//! Validation ([`plan_set`], [`check_link`], [`unique`]) is pure; the
//! `apply_*` functions and [`create_item`] are the I/O, through
//! [`crate::write_back`] (and, for a new file, an atomic create). They
//! print nothing: each returns what it did, and the caller words it for
//! its own output. They write only after every check passed.

use kiln_model::{Date, Item, ItemId};
use kiln_parse::{add_link, remove_link, set_field, splice, FieldValue};

use crate::new_item::{plan_new_item, NewItem, NewItemSpec};
use crate::project::Project;
use crate::renumber::plan_renumber;
use crate::source::{DiskSource, Stamp};
use crate::write::{read_for_edit, write_back, WriteError};

/// The keys [`plan_set`] changes, for messages.
pub const SETTABLE: &str = "status, title, parent, owner, due, tags or jira";

/// Why an edit wasn't made. Nothing was written in any case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// The value, ID or file isn't one the edit can take.
    Invalid(String),
    /// A file changed on disk since it was read.
    Changed(String),
    /// Reading or writing failed.
    Io(String),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::Invalid(m) | EditError::Changed(m) | EditError::Io(m) => f.write_str(m),
        }
    }
}

impl From<WriteError> for EditError {
    fn from(e: WriteError) -> EditError {
        match e {
            WriteError::Changed(m) => EditError::Changed(m),
            WriteError::Io(m) => EditError::Io(m),
        }
    }
}

fn invalid(m: impl Into<String>) -> EditError {
    EditError::Invalid(m.into())
}

/// The item with `id`, which must exist once: an edit refuses a
/// duplicated ID, since it couldn't say which file it changed.
pub fn unique<'p>(project: &'p Project, id: &ItemId) -> Result<&'p Item, String> {
    let item = project
        .item(id)
        .ok_or_else(|| format!("{id} doesn't exist"))?;
    let copies = project.indices(id);
    if copies.len() > 1 {
        let paths: Vec<&str> = copies
            .iter()
            .map(|&i| project.items[i].path.as_str())
            .collect();
        return Err(format!(
            "{id} is in {} files ({}); `kiln renumber {id} --to {} --file …` first",
            paths.len(),
            paths.join(", "),
            project.next_id(id.prefix())
        ));
    }
    Ok(item)
}

/// The value to write for `key` on `item`, or why not. `None` removes the
/// key. An empty value removes `parent`, `owner`, `due`, `tags` and
/// `jira`. `tags` and `jira` take comma-separated lists and replace the
/// whole list: a command that adds one tag would need a second to remove
/// one, and an agent can read the list first (`kiln show`).
pub fn plan_set(
    p: &Project,
    item: &Item,
    key: &str,
    value: &str,
) -> Result<Option<FieldValue>, String> {
    let ty = p.type_of(item);
    match key {
        "status" => {
            let ty = ty
                .ok_or_else(|| format!("{}'s type `{}` isn't declared", item.id, item.type_name))?;
            if !ty.statuses.iter().any(|s| s == value) {
                return Err(format!(
                    "`{value}` isn't a {} status ({})",
                    ty.name,
                    ty.statuses.join(", ")
                ));
            }
            Ok(Some(FieldValue::String(value.to_owned())))
        }
        "title" => {
            if value.is_empty() || value.chars().count() > kiln_parse::MAX_TITLE {
                return Err(format!(
                    "a title is one line of 1 to {} characters",
                    kiln_parse::MAX_TITLE
                ));
            }
            Ok(Some(FieldValue::String(value.to_owned())))
        }
        "parent" if value.is_empty() => Ok(None),
        "parent" => {
            let parent: ItemId = value.parse().map_err(|e| format!("`{value}`: {e}"))?;
            let target = p
                .item(&parent)
                .ok_or_else(|| format!("{parent} doesn't exist"))?;
            if let Some(ty) = ty {
                if !ty.parents.contains(&target.type_name) {
                    let allowed = if ty.parents.is_empty() {
                        "none".to_owned()
                    } else {
                        ty.parents.join(", ")
                    };
                    return Err(format!(
                        "a {} can't have a {} as parent (allowed: {allowed})",
                        ty.name, target.type_name
                    ));
                }
            }
            if parent == item.id || p.ancestors(&parent).iter().any(|a| a.id == item.id) {
                return Err(format!(
                    "{parent} is {} or below it; that would be a parent cycle",
                    item.id
                ));
            }
            Ok(Some(FieldValue::String(parent.to_string())))
        }
        "owner" if value.is_empty() => Ok(None),
        "owner" => Ok(Some(FieldValue::String(value.to_owned()))),
        "due" if value.is_empty() => Ok(None),
        "due" => parse_date(value)
            .map(|d| Some(FieldValue::Date(d)))
            .ok_or_else(|| format!("`{value}` isn't a date (2026-11-30)")),
        "tags" | "jira" => {
            let list: Vec<String> = value
                .split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .collect();
            for v in &list {
                if key == "tags" && v.chars().any(|c| c.is_uppercase() || c.is_control()) {
                    return Err(format!("tag `{v}` must be lowercase"));
                }
                if key == "jira" && !is_jira_key(v) {
                    return Err(format!("`{v}` isn't a Jira issue key (PROJ-123)"));
                }
            }
            Ok(match (key, list.len()) {
                (_, 0) => None,
                ("jira", 1) => Some(FieldValue::String(list[0].clone())),
                _ => Some(FieldValue::Strings(list)),
            })
        }
        _ => Err(format!("kiln set changes {SETTABLE}, not `{key}`")),
    }
}

/// `PROJ-123`: a project key (a capital letter, then capitals, digits or
/// underscores) and a number.
fn is_jira_key(s: &str) -> bool {
    let Some((project, number)) = s.split_once('-') else {
        return false;
    };
    project.starts_with(|c: char| c.is_ascii_uppercase())
        && project
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
}

/// `YYYY-MM-DD`, a real day.
pub fn parse_date(s: &str) -> Option<Date> {
    let mut parts = s.split('-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let date = Date {
        year: y.parse().ok()?,
        month: m.parse().ok()?,
        day: d.parse().ok()?,
    };
    let leap = date.year % 4 == 0 && (date.year % 100 != 0 || date.year % 400 == 0);
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let valid = (1..=12).contains(&date.month)
        && date.day >= 1
        && date.day <= days[usize::from(date.month) - 1];
    valid.then_some(date)
}

/// Whether `item` may link to `target` by `role`: the role exists, is
/// written on its source side, and allows both types.
pub fn check_link(p: &Project, item: &Item, role: &str, target: &ItemId) -> Result<(), String> {
    let r = p.config.role(role).ok_or_else(|| {
        let hint = match p.config.role_by_inverse_key(role) {
            Some(inverse) => format!(
                "; write it from the other side: kiln link {target} {} {}",
                inverse.name, item.id
            ),
            None => {
                let names: Vec<&str> = p.config.links.iter().map(|r| r.name.as_str()).collect();
                format!(" ({})", names.join(", "))
            }
        };
        format!("`{role}` isn't a link role{hint}")
    })?;
    if !r.allows_from(&item.type_name) {
        return Err(format!(
            "a {} can't be the source of `{role}` (from: {})",
            item.type_name,
            r.from.join(", ")
        ));
    }
    let target_item = p
        .item(target)
        .ok_or_else(|| format!("{target} doesn't exist"))?;
    if !r.allows_to(&target_item.type_name) {
        return Err(format!(
            "`{role}` can't point at a {} (to: {})",
            target_item.type_name,
            r.to.join(", ")
        ));
    }
    Ok(())
}

/// One key [`apply_set`] changed: the value before and after, as written
/// (empty for none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetChange {
    pub id: ItemId,
    pub key: String,
    pub old: String,
    pub new: String,
}

impl SetChange {
    /// `REQ-014: status draft → approved`, what `kiln set` prints.
    pub fn line(&self) -> String {
        let show = |s: &str| {
            if s.is_empty() {
                "(none)".to_owned()
            } else {
                s.to_owned()
            }
        };
        format!(
            "{}: {} {} → {}",
            self.id,
            self.key,
            show(&self.old),
            show(&self.new)
        )
    }
}

fn current(item: &Item, key: &str) -> String {
    match key {
        "status" => item.status.clone(),
        "title" => item.title.clone(),
        "parent" => item
            .parent
            .as_ref()
            .map(|r| r.raw.clone())
            .unwrap_or_default(),
        "owner" => item.owner.clone().unwrap_or_default(),
        "tags" => item.tags.join(","),
        "jira" => item.jira.join(","),
        _ => item.due.map(|d| d.to_string()).unwrap_or_default(),
    }
}

/// Reads an item file for editing as normalised text.
fn read_item(source: &DiskSource, rel: &str) -> Result<(String, Vec<u8>, Stamp), EditError> {
    let file = read_for_edit(&source.path(rel))?;
    let text =
        kiln_parse::normalize(&file.bytes).ok_or_else(|| invalid(format!("{rel} isn't UTF-8")))?;
    Ok((text, file.bytes, file.stamp))
}

/// `kiln set ID KEY=VALUE…`: every value checked first, then one write.
pub fn apply_set(
    p: &Project,
    source: &DiskSource,
    id: &ItemId,
    assignments: &[(String, String)],
) -> Result<Vec<SetChange>, EditError> {
    let item = unique(p, id).map_err(invalid)?;
    let mut planned = Vec::new();
    for (key, value) in assignments {
        let new = plan_set(p, item, key, value).map_err(invalid)?;
        planned.push((key.clone(), new, value.clone()));
    }
    let (mut text, bytes, stamp) = read_item(source, &item.path)?;
    for (key, value, _) in &planned {
        text = set_field(&text, key, value.clone())
            .map_err(|e| invalid(format!("{}: {e}", item.path)))?;
    }
    write_back(
        &source.path(&item.path),
        &item.path,
        &bytes,
        stamp,
        &text,
        true,
    )?;
    Ok(planned
        .into_iter()
        .map(|(key, _, value)| SetChange {
            id: id.clone(),
            old: current(item, &key),
            key,
            new: value,
        })
        .collect())
}

/// `kiln link` (`add`) or `kiln unlink`; the line `kiln` prints.
pub fn apply_link(
    p: &Project,
    source: &DiskSource,
    id: &ItemId,
    role: &str,
    target: &ItemId,
    add: bool,
) -> Result<String, EditError> {
    let item = unique(p, id).map_err(invalid)?;
    if add {
        check_link(p, item, role, target).map_err(invalid)?;
    }
    let (text, bytes, stamp) = read_item(source, &item.path)?;
    let target_text = target.to_string();
    let new = if add {
        add_link(&text, role, &target_text)
    } else {
        remove_link(&text, role, &target_text)
    }
    .map_err(|e| invalid(format!("{}: {e}", item.path)))?;
    if new == text {
        return Ok(format!("{id} {role} {target} (already there)"));
    }
    write_back(
        &source.path(&item.path),
        &item.path,
        &bytes,
        stamp,
        &new,
        true,
    )?;
    Ok(format!(
        "{id} {}{role} {target}",
        if add { "" } else { "no longer " }
    ))
}

/// Records `key` as `id`'s Jira ticket (`jira = "DEMO-102"`): what `kiln
/// jira sync` writes after creating the ticket, so the next sync updates
/// it rather than creating another.
pub fn apply_jira_key(
    p: &Project,
    source: &DiskSource,
    id: &ItemId,
    key: &str,
) -> Result<(), EditError> {
    let item = unique(p, id).map_err(invalid)?;
    let (text, bytes, stamp) = read_item(source, &item.path)?;
    let new = set_field(&text, "jira", Some(FieldValue::String(key.to_owned())))
        .map_err(|e| invalid(format!("{}: {e}", item.path)))?;
    write_back(
        &source.path(&item.path),
        &item.path,
        &bytes,
        stamp,
        &new,
        true,
    )?;
    Ok(())
}

/// Sets `key = "value"` in `kiln.toml`'s `[table]` (`[jira] parent`),
/// keeping the rest of the file as written.
pub fn apply_config_key(
    source: &DiskSource,
    table: &str,
    key: &str,
    value: &str,
) -> Result<(), EditError> {
    let rel = kiln_parse::CONFIG_FILE;
    let (text, bytes, stamp) = read_item(source, rel)?;
    let new = kiln_parse::set_config_key(&text, table, key, value)
        .map_err(|e| invalid(format!("{rel}: {e}")))?;
    write_back(&source.path(rel), rel, &bytes, stamp, &new, true)?;
    Ok(())
}

/// What [`apply_renumber`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renumbered {
    /// `REQ-015 → REQ-016: 2 references in 1 other file; a.md → b.md`.
    pub summary: String,
    /// References left alone because they may mean either item.
    pub left_alone: Vec<String>,
}

/// `kiln renumber`: every file checked unchanged since `source` read it
/// before any is written, so a refusal leaves the project as it was; the
/// item's file is renamed last.
pub fn apply_renumber(
    p: &Project,
    source: &DiskSource,
    from: &ItemId,
    to: &ItemId,
    file: Option<&str>,
) -> Result<Renumbered, EditError> {
    let plan = plan_renumber(p, from, to, file).map_err(invalid)?;
    if let Some(new_path) = &plan.new_path {
        if source.path(new_path).exists() {
            return Err(invalid(format!("{new_path} already exists")));
        }
    }
    let mut writes = Vec::new();
    for (rel, edits) in &plan.files {
        let path = source.path(rel);
        let stamp = source
            .stamp(rel)
            .ok_or_else(|| invalid(format!("{rel} wasn't read while loading")))?;
        let io = |e: std::io::Error| EditError::Io(format!("{rel}: {e}"));
        let bytes = std::fs::read(&path).map_err(io)?;
        if Stamp::of(&path).map_err(io)? != stamp {
            return Err(EditError::Changed(format!(
                "{rel} changed on disk while renumbering; nothing was written, run it again"
            )));
        }
        let text = if edits.code {
            String::from_utf8(bytes.clone()).map_err(|_| invalid(format!("{rel} isn't UTF-8")))?
        } else {
            kiln_parse::normalize(&bytes).ok_or_else(|| invalid(format!("{rel} isn't UTF-8")))?
        };
        let in_bounds = edits.edits.iter().all(|(r, _)| {
            r.end <= text.len()
                && text.is_char_boundary(r.start)
                && text.is_char_boundary(r.end)
                && text[r.clone()] == plan.from.to_string()
        });
        if !in_bounds {
            return Err(EditError::Changed(format!(
                "{rel} doesn't match what was indexed; nothing was written, run it again"
            )));
        }
        let new = splice(&text, &edits.edits);
        writes.push((
            path,
            rel.clone(),
            bytes,
            stamp,
            new,
            !edits.code,
            edits.edits.len(),
        ));
    }
    let mut references = 0;
    for (path, rel, bytes, stamp, new, normalised, count) in &writes {
        write_back(path, rel, bytes, *stamp, new, *normalised)?;
        if *rel != plan.item_path {
            references += count;
        }
    }
    let files = writes.iter().filter(|w| w.1 != plan.item_path).count();
    let mut summary = format!(
        "{} → {}: {references} reference{} in {files} other file{}",
        plan.from,
        plan.to,
        if references == 1 { "" } else { "s" },
        if files == 1 { "" } else { "s" }
    );
    if let Some(new_path) = &plan.new_path {
        std::fs::rename(source.path(&plan.item_path), source.path(new_path)).map_err(|e| {
            EditError::Io(format!(
                "renumbered, but couldn't rename {} to {new_path}: {e}",
                plan.item_path
            ))
        })?;
        summary.push_str(&format!("; {} → {new_path}", plan.item_path));
    }
    Ok(Renumbered {
        summary,
        left_alone: plan.left_alone,
    })
}

/// `kiln new`: plans the item and creates its file, never over an
/// existing one.
pub fn create_item(
    p: &Project,
    source: &DiskSource,
    spec: &NewItemSpec,
) -> Result<NewItem, EditError> {
    let new = plan_new_item(p, source, spec).map_err(invalid)?;
    let path = source.path(&new.path);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| EditError::Io(format!("{}: {e}", dir.display())))?;
    }
    fenix_storage::atomic_write_new(&path, |w| std::io::Write::write_all(w, new.text.as_bytes()))
        .map_err(|e| EditError::Io(format!("{}: can't create it: {e}", new.path)))?;
    Ok(new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemorySource;

    #[test]
    fn dates_are_checked() {
        assert_eq!(
            parse_date("2026-11-30").map(|d| d.to_string()).as_deref(),
            Some("2026-11-30")
        );
        assert_eq!(parse_date("2024-02-29").map(|d| d.day), Some(29));
        for bad in [
            "2026-02-29",
            "2026-13-01",
            "2026-1-01",
            "26-01-01",
            "2026-01-01-01",
            "soon",
        ] {
            assert!(parse_date(bad).is_none(), "{bad}");
        }
    }

    fn project() -> Project {
        let item = |id: &str, ty: &str, rest: &str| {
            format!("+++\nid = \"{id}\"\ntype = \"{ty}\"\ntitle = \"T\"\nstatus = \"draft\"\n{rest}+++\n")
        };
        let source = MemorySource::new()
            .with("kiln.toml", kiln_model::default_config("Demo"))
            .with("design/REQ-001.md", item("REQ-001", "requirement", ""))
            .with(
                "design/REQ-002.md",
                item("REQ-002", "requirement", "parent = \"REQ-001\"\n"),
            )
            .with("design/TEST-001.md", item("TEST-001", "test", ""));
        Project::load(&source).unwrap()
    }

    #[test]
    fn values_are_checked_against_the_configuration() {
        let p = project();
        let req1 = p.item(&"REQ-001".parse().unwrap()).unwrap();
        assert!(plan_set(&p, req1, "status", "approved").is_ok());
        assert!(plan_set(&p, req1, "status", "nonsense")
            .unwrap_err()
            .starts_with("`nonsense` isn't a requirement status"));
        assert_eq!(plan_set(&p, req1, "parent", "").unwrap(), None);
        assert!(plan_set(&p, req1, "parent", "REQ-002")
            .unwrap_err()
            .contains("parent cycle"));
        assert!(plan_set(&p, req1, "parent", "TEST-001")
            .unwrap_err()
            .contains("can't have a test as parent"));
        assert!(plan_set(&p, req1, "due", "soon").is_err());
        assert_eq!(
            plan_set(&p, req1, "tags", "perf, ux").unwrap(),
            Some(FieldValue::Strings(vec!["perf".into(), "ux".into()]))
        );
        assert_eq!(plan_set(&p, req1, "tags", "").unwrap(), None);
        assert!(plan_set(&p, req1, "tags", "Perf").is_err());
        assert_eq!(
            plan_set(&p, req1, "jira", "DEMO-12").unwrap(),
            Some(FieldValue::String("DEMO-12".into()))
        );
        assert!(plan_set(&p, req1, "jira", "demo-12").is_err());
        assert!(plan_set(&p, req1, "colour", "red")
            .unwrap_err()
            .contains("not `colour`"));
    }

    #[test]
    fn links_are_checked_by_role_and_type() {
        let p = project();
        let test = p.item(&"TEST-001".parse().unwrap()).unwrap();
        let req1 = p.item(&"REQ-001".parse().unwrap()).unwrap();
        let r1: ItemId = "REQ-001".parse().unwrap();
        let t1: ItemId = "TEST-001".parse().unwrap();
        assert!(check_link(&p, test, "verifies", &r1).is_ok());
        assert!(check_link(&p, req1, "verifies", &t1)
            .unwrap_err()
            .contains("can't be the source of `verifies`"));
        assert!(check_link(&p, req1, "verified-by", &t1)
            .unwrap_err()
            .contains("write it from the other side: kiln link TEST-001 verifies REQ-001"));
    }
}
