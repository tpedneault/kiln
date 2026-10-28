//! Code actions in an item file, and formatting.
//!
//! - "Set status → …", for each other status of the item's type;
//! - on an ID in the file: "Add `<role>` link to <ID>", for each role
//!   allowed from this item to that one and not already there;
//! - "New child <type>", for each type that may have this one as parent;
//! - "Create <type> that <role> <ID>", for each coverage role that can
//!   reach this item (a test that verifies a requirement, …);
//! - "Rename file to match the ID", when its name doesn't start with it
//!   (K105).
//!
//! The last three create or rename a file, which a client applies only
//! when it says it can (`workspaceEdit.resourceOperations`). For a client
//! that can't (Fenix, until its G7), they come back disabled, with the
//! command that does the same from a terminal as the reason: the action
//! isn't hidden, so the client can say why it is missing.

use std::collections::HashMap;

use kiln_index::{plan_new_item, NewItemSpec, Project};
use kiln_model::slug;
use kiln_parse::{add_link, format_front_matter, set_field, FieldValue};
use lsp_types::{
    CodeAction, CodeActionDisabled, CodeActionKind, CreateFile, DocumentChangeOperation,
    DocumentChanges, OneOf, OptionalVersionedTextDocumentIdentifier, Position, Range, RenameFile,
    ResourceOp, TextDocumentEdit, TextEdit, Uri, WorkspaceEdit,
};

use super::{edit_between, id_at, item_at};
use crate::text::Doc;
use crate::world::World;

/// What the client can apply.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClientCan {
    pub create: bool,
    pub rename: bool,
}

const G7: &str = "the editor can't create or rename files from a code action (Fenix G7)";

pub fn actions(
    world: &World,
    project: &Project,
    uri: &Uri,
    doc: &Doc,
    rel: Option<&str>,
    offset: usize,
    can: ClientCan,
) -> Vec<CodeAction> {
    let Some(item) = item_at(project, rel) else {
        return Vec::new();
    };
    let Some(ty) = project.config.type_named(&item.type_name) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let text_action = |title: String, new_text: Result<String, kiln_parse::EditError>| {
        let new_text = new_text.ok()?;
        let edit = edit_between(doc, &doc.text, &new_text)?;
        Some(CodeAction {
            title,
            kind: Some(CodeActionKind::REFACTOR),
            edit: Some(WorkspaceEdit {
                changes: Some(HashMap::from([(uri.clone(), vec![edit])])),
                ..Default::default()
            }),
            ..Default::default()
        })
    };

    // A link to the ID under the cursor.
    if let Some((_, _, id)) = id_at(&doc.text, offset, &project.config.prefixes()) {
        if let Some(other) = project.item(&id).filter(|o| o.id != item.id) {
            for role in &project.config.links {
                let linked = item
                    .links
                    .iter()
                    .any(|l| l.role == role.name && l.target.id.as_ref() == Ok(&id));
                if role.allows_from(&ty.name) && role.allows_to(&other.type_name) && !linked {
                    out.extend(text_action(
                        format!("Add `{}` link to {id}", role.name),
                        add_link(&doc.text, &role.name, &id.to_string()),
                    ));
                }
            }
        }
    }

    for status in ty.statuses.iter().filter(|s| **s != item.status) {
        out.extend(text_action(
            format!("Set status → {status}"),
            set_field(
                &doc.text,
                "status",
                Some(FieldValue::String(status.clone())),
            ),
        ));
    }

    // New items: a child, or one covering this item.
    let mut new_items: Vec<NewAction> = Vec::new();
    for child in project
        .config
        .types
        .iter()
        .filter(|t| t.parents.contains(&ty.name))
    {
        new_items.push((
            format!("New child {}", child.name),
            child.name.clone(),
            Some(item.id.clone()),
            Vec::new(),
            format!("kiln new {} --parent {} --open", child.name, item.id),
        ));
    }
    for role_name in &project.config.coverage_roles {
        let Some(role) = project
            .config
            .role(role_name)
            .filter(|r| r.allows_to(&ty.name))
        else {
            continue;
        };
        for from in role
            .from
            .iter()
            .filter(|f| project.config.type_named(f).is_some())
        {
            new_items.push((
                format!("Create {from} that {} {}", role.name, item.id),
                from.clone(),
                None,
                vec![(role.name.clone(), item.id.clone())],
                format!(
                    "kiln new {from} --open, then kiln link <new ID> {} {}",
                    role.name, item.id
                ),
            ));
        }
    }
    for (title, type_name, parent, links, command) in new_items {
        let spec = NewItemSpec {
            type_name: &type_name,
            title: None,
            parent: parent.as_ref(),
            links: &links,
            ..Default::default()
        };
        let Some(source) = world.source() else {
            continue;
        };
        let Ok(new) = plan_new_item(project, &source, &spec) else {
            continue;
        };
        if !can.create {
            out.push(disabled(title, format!("{G7}; run `{command}`")));
            continue;
        }
        let Some(new_uri) = world.uri(&new.path) else {
            continue;
        };
        out.push(CodeAction {
            title,
            kind: Some(CodeActionKind::REFACTOR),
            edit: Some(WorkspaceEdit {
                document_changes: Some(DocumentChanges::Operations(vec![
                    DocumentChangeOperation::Op(ResourceOp::Create(CreateFile {
                        uri: new_uri.clone(),
                        options: None,
                        annotation_id: None,
                    })),
                    DocumentChangeOperation::Edit(TextDocumentEdit {
                        text_document: OptionalVersionedTextDocumentIdentifier {
                            uri: new_uri,
                            version: None,
                        },
                        edits: vec![OneOf::Left(TextEdit {
                            range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                            new_text: new.text,
                        })],
                    }),
                ])),
                ..Default::default()
            }),
            ..Default::default()
        });
    }

    // K105: the file name doesn't start with the ID.
    let (dir, name) = item.path.rsplit_once('/').unwrap_or(("", &item.path));
    if !name.starts_with(&item.id.to_string()) {
        let new_name = match slug(&item.title, 50) {
            s if s.is_empty() => format!("{}.md", item.id),
            s => format!("{}-{s}.md", item.id),
        };
        let new_path = if dir.is_empty() {
            new_name.clone()
        } else {
            format!("{dir}/{new_name}")
        };
        let title = format!("Rename file to {new_name}");
        match (can.rename, world.uri(&new_path)) {
            (true, Some(new_uri)) => out.push(CodeAction {
                title,
                kind: Some(CodeActionKind::QUICKFIX),
                edit: Some(WorkspaceEdit {
                    document_changes: Some(DocumentChanges::Operations(vec![
                        DocumentChangeOperation::Op(ResourceOp::Rename(RenameFile {
                            old_uri: uri.clone(),
                            new_uri,
                            options: None,
                            annotation_id: None,
                        })),
                    ])),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            _ => out.push(disabled(
                title,
                format!("{G7}; rename it to {new_path} by hand"),
            )),
        }
    }
    out
}

/// A code action creating an item: title, type, parent, links, and the
/// command doing the same from a terminal.
type NewAction = (
    String,
    String,
    Option<kiln_model::ItemId>,
    Vec<(String, kiln_model::ItemId)>,
    String,
);

fn disabled(title: String, reason: String) -> CodeAction {
    CodeAction {
        title,
        kind: Some(CodeActionKind::REFACTOR),
        disabled: Some(CodeActionDisabled { reason }),
        ..Default::default()
    }
}

/// The front matter in FORMAT.md's key order and quoting; the body as is.
pub fn format(doc: &Doc) -> Vec<TextEdit> {
    format_front_matter(&doc.text)
        .ok()
        .and_then(|new| edit_between(doc, &doc.text, &new))
        .into_iter()
        .collect()
}
