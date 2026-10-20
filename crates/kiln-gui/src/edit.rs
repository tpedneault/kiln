//! The edits the GUI makes (GUI.md, "Editing"): one-line front-matter
//! values, links, a new item, a renumbering. Never an item's body: that is
//! prose, and prose is edited in Fenix.
//!
//! The application decides an [`Edit`] and checks its value against the
//! project it shows; [`apply`] makes it, through `kiln-index`'s
//! `apply_*` functions, the ones `kiln set`, `link`, `unlink`,
//! `renumber` and `new` call. It reads the project again first, as each
//! command does, so a file an editor saved in the meantime is refused
//! rather than overwritten.

use std::path::{Path, PathBuf};

use kiln_index::{DiskSource, Project};
use kiln_model::ItemId;

/// An edit to make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// `kiln set ID KEY=VALUE`; an empty value removes the key.
    Set {
        id: ItemId,
        key: String,
        value: String,
    },
    /// `kiln link` (`add`) or `kiln unlink`.
    Link {
        id: ItemId,
        role: String,
        target: ItemId,
        add: bool,
    },
    /// `kiln new TYPE [--parent ID] [--title TEXT] --open`.
    New {
        type_name: String,
        title: Option<String>,
        parent: Option<ItemId>,
    },
    /// `kiln renumber FROM --to TO [--file PATH]`.
    Renumber {
        from: ItemId,
        to: ItemId,
        file: Option<String>,
    },
}

/// What an edit did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// What the command would have printed.
    pub message: String,
    /// The item the cursor should follow.
    pub item: Option<ItemId>,
    /// A new item's file (absolute) and title line, to open in Fenix.
    pub created: Option<(PathBuf, u32)>,
}

/// Makes `edit` in the project at `root`.
pub fn apply(root: &Path, edit: &Edit) -> Result<Applied, String> {
    let source = DiskSource::new(root);
    let project = Project::load(&source).map_err(|e| format!("couldn't read the project: {e}"))?;
    let fail = |e: kiln_index::EditError| e.to_string();
    match edit {
        Edit::Set { id, key, value } => {
            let pairs = [(key.clone(), value.clone())];
            let changes = kiln_index::apply_set(&project, &source, id, &pairs).map_err(fail)?;
            Ok(Applied {
                message: changes
                    .iter()
                    .map(|c| c.line())
                    .collect::<Vec<_>>()
                    .join("; "),
                item: Some(id.clone()),
                created: None,
            })
        }
        Edit::Link {
            id,
            role,
            target,
            add,
        } => Ok(Applied {
            message: kiln_index::apply_link(&project, &source, id, role, target, *add)
                .map_err(fail)?,
            item: Some(id.clone()),
            created: None,
        }),
        Edit::New {
            type_name,
            title,
            parent,
        } => {
            let spec = kiln_index::NewItemSpec {
                type_name,
                title: title.as_deref(),
                parent: parent.as_ref(),
                links: &[],
            };
            let new = kiln_index::create_item(&project, &source, &spec).map_err(fail)?;
            Ok(Applied {
                message: format!("created {}: {}", new.id, new.path),
                created: Some((source.path(&new.path), new.title_line)),
                item: Some(new.id),
            })
        }
        Edit::Renumber { from, to, file } => {
            let done = kiln_index::apply_renumber(&project, &source, from, to, file.as_deref())
                .map_err(fail)?;
            let mut message = done.summary;
            if !done.left_alone.is_empty() {
                message.push_str(&format!(
                    "; left alone (may mean either): {}",
                    done.left_alone.join(", ")
                ));
            }
            Ok(Applied {
                message,
                item: Some(to.clone()),
                created: None,
            })
        }
    }
}
