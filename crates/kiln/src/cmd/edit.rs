//! `kiln set`, `link` and `unlink`: comment-preserving front-matter edits,
//! validated against the configuration before anything is written.

use std::path::Path;

use kiln_index::Project;
use kiln_model::{Date, Item};
use kiln_parse::{add_link, remove_link, set_field, FieldValue};

use super::{existing, parse_id, unique};
use crate::project::{load, read_for_edit, write_back};
use crate::Fail;

const SETTABLE: &str = "status, title, parent, owner or due";

pub fn set(project: Option<&Path>, id: &str, assignments: &[String]) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let id = parse_id(id)?;
    let item = unique(p, &id)?;
    let mut changes: Vec<(String, Option<FieldValue>, String)> = Vec::new();
    for assignment in assignments {
        let (key, value) = assignment
            .split_once('=')
            .ok_or_else(|| Fail::usage(format!("`{assignment}`: write KEY=VALUE ({SETTABLE})")))?;
        let (key, value) = (key.trim(), value.trim());
        let new = validate(p, item, key, value)?;
        changes.push((key.to_owned(), new, value.to_owned()));
    }

    let path = loaded.source.path(&item.path);
    let file = read_for_edit(&path)?;
    let mut text = kiln_parse::normalize(&file.bytes)
        .ok_or_else(|| Fail::usage(format!("{} isn't UTF-8", item.path)))?;
    for (key, value, _) in &changes {
        text = set_field(&text, key, value.clone())
            .map_err(|e| Fail::usage(format!("{}: {e}", item.path)))?;
    }
    write_back(&path, &item.path, &file.bytes, file.stamp, &text, true)?;
    for (key, _, value) in changes {
        let old = match key.as_str() {
            "status" => item.status.clone(),
            "title" => item.title.clone(),
            "parent" => item
                .parent
                .as_ref()
                .map(|r| r.raw.clone())
                .unwrap_or_default(),
            "owner" => item.owner.clone().unwrap_or_default(),
            _ => item.due.map(|d| d.to_string()).unwrap_or_default(),
        };
        let show = |s: &str| {
            if s.is_empty() {
                "(none)".to_owned()
            } else {
                s.to_owned()
            }
        };
        println!("{id}: {key} {} → {}", show(&old), show(&value));
    }
    Ok(0)
}

/// The value to write for `key`, or why not. `None` removes the key.
fn validate(p: &Project, item: &Item, key: &str, value: &str) -> Result<Option<FieldValue>, Fail> {
    let ty = p.type_of(item);
    match key {
        "status" => {
            let ty = ty.ok_or_else(|| {
                Fail::usage(format!(
                    "{}'s type `{}` isn't declared",
                    item.id, item.type_name
                ))
            })?;
            if !ty.statuses.iter().any(|s| s == value) {
                return Err(Fail::usage(format!(
                    "`{value}` isn't a {} status ({})",
                    ty.name,
                    ty.statuses.join(", ")
                )));
            }
            Ok(Some(FieldValue::String(value.to_owned())))
        }
        "title" => {
            if value.is_empty() || value.chars().count() > kiln_parse::MAX_TITLE {
                return Err(Fail::usage(format!(
                    "a title is one line of 1 to {} characters",
                    kiln_parse::MAX_TITLE
                )));
            }
            Ok(Some(FieldValue::String(value.to_owned())))
        }
        "parent" if value.is_empty() => Ok(None),
        "parent" => {
            let parent = parse_id(value)?;
            let target = existing(p, &parent)?;
            if let Some(ty) = ty {
                if !ty.parents.contains(&target.type_name) {
                    let allowed = if ty.parents.is_empty() {
                        "none".to_owned()
                    } else {
                        ty.parents.join(", ")
                    };
                    return Err(Fail::usage(format!(
                        "a {} can't have a {} as parent (allowed: {allowed})",
                        ty.name, target.type_name
                    )));
                }
            }
            if parent == item.id || p.ancestors(&parent).iter().any(|a| a.id == item.id) {
                return Err(Fail::usage(format!(
                    "{parent} is {} or below it; that would be a parent cycle",
                    item.id
                )));
            }
            Ok(Some(FieldValue::String(parent.to_string())))
        }
        "owner" if value.is_empty() => Ok(None),
        "owner" => Ok(Some(FieldValue::String(value.to_owned()))),
        "due" if value.is_empty() => Ok(None),
        "due" => parse_date(value)
            .map(|d| Some(FieldValue::Date(d)))
            .ok_or_else(|| Fail::usage(format!("`{value}` isn't a date (2026-11-30)"))),
        _ => Err(Fail::usage(format!(
            "kiln set changes {SETTABLE}, not `{key}`"
        ))),
    }
}

fn parse_date(s: &str) -> Option<Date> {
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

pub fn link(
    project: Option<&Path>,
    id: &str,
    role: &str,
    target: &str,
    add: bool,
) -> Result<u8, Fail> {
    let loaded = load(project)?;
    let p = &loaded.project;
    let id = parse_id(id)?;
    let target_id = parse_id(target)?;
    let item = unique(p, &id)?;
    if add {
        let r = p.config.role(role).ok_or_else(|| {
            let hint = match p.config.role_by_inverse_key(role) {
                Some(inverse) => format!(
                    "; write it from the other side: kiln link {target_id} {} {id}",
                    inverse.name
                ),
                None => {
                    let names: Vec<&str> = p.config.links.iter().map(|r| r.name.as_str()).collect();
                    format!(" ({})", names.join(", "))
                }
            };
            Fail::usage(format!("`{role}` isn't a link role{hint}"))
        })?;
        if !r.allows_from(&item.type_name) {
            return Err(Fail::usage(format!(
                "a {} can't be the source of `{role}` (from: {})",
                item.type_name,
                r.from.join(", ")
            )));
        }
        let target_item = existing(p, &target_id)?;
        if !r.allows_to(&target_item.type_name) {
            return Err(Fail::usage(format!(
                "`{role}` can't point at a {} (to: {})",
                target_item.type_name,
                r.to.join(", ")
            )));
        }
    }
    let path = loaded.source.path(&item.path);
    let file = read_for_edit(&path)?;
    let text = kiln_parse::normalize(&file.bytes)
        .ok_or_else(|| Fail::usage(format!("{} isn't UTF-8", item.path)))?;
    let target_text = target_id.to_string();
    let new = if add {
        add_link(&text, role, &target_text)
    } else {
        remove_link(&text, role, &target_text)
    }
    .map_err(|e| Fail::usage(format!("{}: {e}", item.path)))?;
    if new == text {
        println!("{id} {role} {target_id} (already there)");
        return Ok(0);
    }
    write_back(&path, &item.path, &file.bytes, file.stamp, &new, true)?;
    println!(
        "{id} {}{role} {target_id}",
        if add { "" } else { "no longer " }
    );
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
