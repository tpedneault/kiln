//! A new item's file: the next free ID, the path from the type's folder
//! and the title, front matter (links and tags included) and the type's
//! template as body, or the body given. Shared by
//! `kiln new` and the language server's "New child …" and "Create test
//! verifying this", so both write the same file. Computed, not written:
//! creating it is the caller's.

use kiln_model::{slug, ItemId};
use kiln_parse::{normalize, MAX_TITLE};

use crate::project::Project;
use crate::source::FileSource;

/// File names carry at most this much of the title.
const SLUG_LEN: usize = 50;

/// What to create.
#[derive(Debug, Clone, Default)]
pub struct NewItemSpec<'a> {
    pub type_name: &'a str,
    /// Defaults to "Untitled <type>".
    pub title: Option<&'a str>,
    pub parent: Option<&'a ItemId>,
    /// `[links]` to write: (role, target).
    pub links: &'a [(String, ItemId)],
    /// `tags`, lowercase.
    pub tags: &'a [String],
    /// The body, in place of the type's template: what an agent writes
    /// in the same command that creates the item (REQ-020).
    pub body: Option<&'a str>,
}

/// A new item, ready to be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewItem {
    pub id: ItemId,
    /// Relative to the project root.
    pub path: String,
    pub text: String,
    /// The 1-based line of its title, where an editor opens it.
    pub title_line: u32,
}

/// Plans a new item. Errors are one line, naming what isn't allowed.
pub fn plan_new_item(
    project: &Project,
    source: &dyn FileSource,
    spec: &NewItemSpec,
) -> Result<NewItem, String> {
    let config = &project.config;
    let ty = config.type_named(spec.type_name).ok_or_else(|| {
        let names: Vec<&str> = config.types.iter().map(|t| t.name.as_str()).collect();
        format!("`{}` isn't a type ({})", spec.type_name, names.join(", "))
    })?;
    if let Some(parent) = spec.parent {
        let target = project
            .item(parent)
            .ok_or_else(|| format!("{parent} doesn't exist"))?;
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
    for (role, target) in spec.links {
        let def = config
            .role(role)
            .ok_or_else(|| format!("`{role}` isn't a link role"))?;
        let target_item = project
            .item(target)
            .ok_or_else(|| format!("{target} doesn't exist"))?;
        if !def.allows_from(&ty.name) || !def.allows_to(&target_item.type_name) {
            return Err(format!(
                "a {} can't `{role}` a {}",
                ty.name, target_item.type_name
            ));
        }
    }
    for tag in spec.tags {
        if tag.is_empty() || tag.chars().any(|c| c.is_uppercase() || c.is_control()) {
            return Err(format!("tag `{tag}` must be lowercase and not empty"));
        }
    }
    let default_title = format!("Untitled {}", ty.title.to_lowercase());
    let title = spec.title.unwrap_or(&default_title);
    if title.contains('\n') || title.chars().count() > MAX_TITLE {
        return Err(format!(
            "a title is one line of at most {MAX_TITLE} characters"
        ));
    }
    let body = match (spec.body, &ty.template) {
        (Some(body), _) => body.replace("\r\n", "\n"),
        (None, None) => String::new(),
        (None, Some(template)) => {
            let bytes = source
                .read(template)
                .map_err(|e| format!("template {template} for {}: {e}", ty.name))?;
            normalize(&bytes).ok_or_else(|| format!("template {template} isn't UTF-8"))?
        }
    };
    let items_dir = config
        .paths
        .first()
        .ok_or("kiln.toml has no `paths` to put items in")?;
    let id = project.next_id(&ty.prefix);
    let name = match slug(title, SLUG_LEN) {
        s if s.is_empty() => format!("{id}.md"),
        s => format!("{id}-{s}.md"),
    };
    let dir = match items_dir.as_str() {
        "." => ty.folder(),
        d => format!("{d}/{}", ty.folder()),
    };
    let mut text = format!(
        "+++\nid = \"{id}\"\ntype = {}\ntitle = {}\nstatus = {}\n",
        toml_string(&ty.name),
        toml_string(title),
        toml_string(&ty.initial)
    );
    if let Some(parent) = spec.parent {
        text.push_str(&format!("parent = \"{parent}\"\n"));
    }
    if !spec.tags.is_empty() {
        let tags: Vec<String> = spec.tags.iter().map(|t| toml_string(t)).collect();
        text.push_str(&format!("tags = [{}]\n", tags.join(", ")));
    }
    if !spec.links.is_empty() {
        text.push_str("\n[links]\n");
        // One line per role, in the order roles first appear: a role
        // given twice apart must not become two keys.
        let mut roles: Vec<&str> = Vec::new();
        for (role, _) in spec.links {
            if !roles.contains(&role.as_str()) {
                roles.push(role);
            }
        }
        for role in roles {
            let targets: Vec<String> = spec
                .links
                .iter()
                .filter(|(r, _)| r == role)
                .map(|(_, t)| format!("\"{t}\""))
                .collect();
            text.push_str(&format!("{role} = [{}]\n", targets.join(", ")));
        }
    }
    text.push_str("+++\n");
    if !body.trim().is_empty() {
        text.push('\n');
        text.push_str(body.trim_start_matches('\n'));
        if !body.ends_with('\n') {
            text.push('\n');
        }
    }
    Ok(NewItem {
        id,
        path: format!("{dir}/{name}"),
        text,
        title_line: 4,
    })
}

/// A TOML basic string, quoted and escaped.
pub fn toml_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{id, item};
    use crate::MemorySource;

    #[test]
    fn toml_strings_escape_what_toml_needs() {
        assert_eq!(
            toml_string("a \"b\" \\ c\u{1}"),
            "\"a \\\"b\\\" \\\\ c\\u0001\""
        );
    }

    #[test]
    fn a_new_item_has_the_next_id_its_parent_and_links() {
        let source = MemorySource::new()
            .with("kiln.toml", kiln_model::default_config("T"))
            .with(
                "design/requirements/REQ-001-a.md",
                item("REQ-001", "requirement", "approved", ""),
            );
        let p = Project::load(&source).unwrap();
        let links = [("verifies".to_owned(), id("REQ-001"))];
        let spec = NewItemSpec {
            type_name: "test",
            title: Some("Starts \"fast\""),
            links: &links,
            ..Default::default()
        };
        let new = plan_new_item(&p, &source, &spec).unwrap();
        assert_eq!(new.path, "design/tests/TEST-001-starts-fast.md");
        assert_eq!(
            new.text,
            "+++\nid = \"TEST-001\"\ntype = \"test\"\ntitle = \"Starts \\\"fast\\\"\"\nstatus = \"draft\"\n\n[links]\nverifies = [\"REQ-001\"]\n+++\n"
        );
        let child = NewItemSpec {
            type_name: "requirement",
            parent: Some(&id("REQ-001")),
            ..Default::default()
        };
        let new = plan_new_item(&p, &source, &child).unwrap();
        assert!(
            new.text.contains("parent = \"REQ-001\"\n")
                && new.path.starts_with("design/requirements/REQ-002-"),
            "{new:?}"
        );
        let bad = NewItemSpec {
            type_name: "requirement",
            links: &links,
            ..Default::default()
        };
        assert_eq!(
            plan_new_item(&p, &source, &bad).unwrap_err(),
            "a requirement can't `verifies` a requirement"
        );
    }

    #[test]
    // verifies: REQ-020
    fn a_new_item_takes_tags_a_body_and_a_role_given_twice() {
        let source = MemorySource::new()
            .with("kiln.toml", kiln_model::default_config("T"))
            .with(
                "design/MS-001.md",
                item("MS-001", "milestone", "active", ""),
            )
            .with(
                "design/REQ-001.md",
                item("REQ-001", "requirement", "approved", ""),
            )
            .with("design/TASK-001.md", item("TASK-001", "task", "todo", ""));
        let p = Project::load(&source).unwrap();
        let links = [
            ("depends-on".to_owned(), id("TASK-001")),
            ("implements".to_owned(), id("REQ-001")),
            ("depends-on".to_owned(), id("MS-001")),
        ];
        let tags = ["csv".to_owned()];
        let spec = NewItemSpec {
            type_name: "task",
            title: Some("Write it"),
            links: &links,
            tags: &tags,
            body: Some("\r\nThe writer.\r\n"),
            ..Default::default()
        };
        let new = plan_new_item(&p, &source, &spec).unwrap();
        assert_eq!(
            new.text,
            "+++\nid = \"TASK-002\"\ntype = \"task\"\ntitle = \"Write it\"\nstatus = \"todo\"\ntags = [\"csv\"]\n\n[links]\ndepends-on = [\"TASK-001\", \"MS-001\"]\nimplements = [\"REQ-001\"]\n+++\n\nThe writer.\n"
        );
        let shouting = ["CSV".to_owned()];
        let bad = NewItemSpec {
            type_name: "task",
            tags: &shouting,
            ..Default::default()
        };
        assert!(plan_new_item(&p, &source, &bad)
            .unwrap_err()
            .contains("must be lowercase"));
    }
}
