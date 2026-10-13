//! Definition, references and rename on an ID, wherever it is written:
//! front matter, a body, or (as a target) a code reference.
//!
//! Rename is `kiln renumber`: the same plan (`kiln_index::plan_renumber`),
//! so the editor and the command line rewrite exactly the same places.
//! Renaming the file as well needs a client that applies file renames in
//! a workspace edit; without one (Fenix, until its G7) the file keeps its
//! name and `kiln check` says so (K105).

use std::collections::HashMap;

use kiln_index::{plan_renumber, Project};
use kiln_model::{ItemId, Span};
use lsp_types::{
    DocumentChangeOperation, DocumentChanges, Location, OneOf,
    OptionalVersionedTextDocumentIdentifier, PrepareRenameResponse, RenameFile, ResourceOp,
    TextDocumentEdit, TextEdit, Uri, WorkspaceEdit,
};

use super::id_at;
use crate::text::{self, Doc};
use crate::world::World;

/// A span in an item file (normalised text) as a location.
fn item_location(world: &World, path: &str, span: Span) -> Option<Location> {
    let doc = world.doc(path)?;
    Some(Location::new(
        world.uri(path)?,
        doc.range(span.start, span.end),
    ))
}

/// A span in a code file (raw text) as a location.
fn code_location(world: &World, path: &str, span: Span) -> Option<Location> {
    let raw = world.raw(path)?;
    let range = lsp_types::Range::new(
        text::position(&raw, span.start),
        text::position(&raw, span.end),
    );
    Some(Location::new(world.uri(path)?, range))
}

/// The ID at `offset` in `doc`, if it is one.
fn target(project: &Project, doc: &Doc, offset: usize) -> Option<(usize, usize, ItemId)> {
    id_at(&doc.text, offset, &project.config.prefixes())
}

/// The item's `id` line, in every file that claims it.
pub fn definition(world: &World, project: &Project, doc: &Doc, offset: usize) -> Vec<Location> {
    let Some((_, _, id)) = target(project, doc, offset) else {
        return Vec::new();
    };
    project
        .indices(&id)
        .iter()
        .filter_map(|&i| {
            let item = &project.items[i];
            item_location(world, &item.path, item.id_span)
        })
        .collect()
}

/// Every place that refers to the item: links, `parent`, mentions, code
/// references; and its own `id` when `declaration`.
pub fn references(
    world: &World,
    project: &Project,
    doc: &Doc,
    offset: usize,
    declaration: bool,
) -> Vec<Location> {
    let Some((_, _, id)) = target(project, doc, offset) else {
        return Vec::new();
    };
    let is = |r: &kiln_model::Ref| r.id.as_ref().is_ok_and(|x| *x == id);
    let mut spans: Vec<(&str, Span, bool)> = Vec::new();
    for item in &project.items {
        if declaration && item.id == id {
            spans.push((&item.path, item.id_span, false));
        }
        if let Some(parent) = item.parent.as_ref().filter(|p| is(p)) {
            spans.push((&item.path, parent.span, false));
        }
        for link in item.links.iter().filter(|l| is(&l.target)) {
            spans.push((&item.path, link.target.span, false));
        }
        for mention in item.mentions.iter().filter(|m| is(&m.target)) {
            spans.push((&item.path, mention.target.span, false));
        }
    }
    for code in project.code_refs.iter().filter(|c| is(&c.target)) {
        spans.push((&code.path, code.target.span, true));
    }
    spans
        .into_iter()
        .filter_map(|(path, span, code)| {
            if code {
                code_location(world, path, span)
            } else {
                item_location(world, path, span)
            }
        })
        .collect()
}

pub fn prepare_rename(
    project: &Project,
    doc: &Doc,
    offset: usize,
) -> Option<PrepareRenameResponse> {
    let (start, end, id) = target(project, doc, offset)?;
    project.item(&id)?;
    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range: doc.range(start, end),
        placeholder: id.to_string(),
    })
}

/// The workspace edit renumbering the ID at `offset` to `new_name`.
/// `rel` picks the item when the ID is duplicated and this is one of its
/// files. `renames` says the client applies file renames.
pub fn rename(
    world: &World,
    project: &Project,
    doc: &Doc,
    rel: Option<&str>,
    offset: usize,
    new_name: &str,
    renames: bool,
) -> Result<Option<WorkspaceEdit>, String> {
    let Some((_, _, from)) = target(project, doc, offset) else {
        return Ok(None);
    };
    let to: ItemId = new_name
        .trim()
        .parse()
        .map_err(|e| format!("`{new_name}`: {e}"))?;
    let file = rel.filter(|r| {
        project
            .indices(&from)
            .iter()
            .any(|&i| project.items[i].path == *r)
    });
    let plan = plan_renumber(project, &from, &to, file)?;
    let mut changes: Vec<(Uri, Vec<TextEdit>)> = Vec::new();
    for (path, edits) in &plan.files {
        let uri = world
            .uri(path)
            .ok_or_else(|| format!("{path}: not a file URI"))?;
        let text_edits: Vec<TextEdit> = if edits.code {
            let raw = world
                .raw(path)
                .ok_or_else(|| format!("{path}: can't read it"))?;
            edits
                .edits
                .iter()
                .map(|(r, new)| TextEdit {
                    range: lsp_types::Range::new(
                        text::position(&raw, r.start),
                        text::position(&raw, r.end),
                    ),
                    new_text: new.clone(),
                })
                .collect()
        } else {
            let doc = world
                .doc(path)
                .ok_or_else(|| format!("{path}: can't read it"))?;
            edits
                .edits
                .iter()
                .map(|(r, new)| TextEdit {
                    range: doc.range(r.start, r.end),
                    new_text: doc.outgoing(new),
                })
                .collect()
        };
        changes.push((uri, text_edits));
    }
    let rename_file = match (&plan.new_path, renames) {
        (Some(new_path), true) => Some(RenameFile {
            old_uri: world
                .uri(&plan.item_path)
                .ok_or("the item's file has no URI")?,
            new_uri: world.uri(new_path).ok_or("the new file name has no URI")?,
            options: None,
            annotation_id: None,
        }),
        _ => None,
    };
    let edit = match rename_file {
        None => WorkspaceEdit {
            changes: Some(changes.into_iter().collect::<HashMap<_, _>>()),
            ..Default::default()
        },
        Some(rename) => {
            let mut ops: Vec<DocumentChangeOperation> = changes
                .into_iter()
                .map(|(uri, edits)| {
                    DocumentChangeOperation::Edit(TextDocumentEdit {
                        text_document: OptionalVersionedTextDocumentIdentifier {
                            uri,
                            version: None,
                        },
                        edits: edits.into_iter().map(OneOf::Left).collect(),
                    })
                })
                .collect();
            ops.push(DocumentChangeOperation::Op(ResourceOp::Rename(rename)));
            WorkspaceEdit {
                document_changes: Some(DocumentChanges::Operations(ops)),
                ..Default::default()
            }
        }
    };
    Ok(Some(edit))
}
