//! Hover on any ID: what `kiln show` prints for it (title, type, status,
//! where it is, parent, children, links both ways, mentions, coverage,
//! progress), as plain text, since that is what every client shows the
//! same way (Fenix flattens hover to text).

use kiln_index::{labelled, Project};
use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};

use super::id_at;
use crate::text::Doc;

pub fn hover(project: &Project, doc: &Doc, offset: usize) -> Option<Hover> {
    let (start, end, id) = id_at(&doc.text, offset, &project.config.prefixes())?;
    let text = match project.item(&id) {
        None => format!(
            "{id} doesn't exist (the next free {} is {})",
            id.prefix(),
            project.next_id(id.prefix())
        ),
        Some(item) => {
            let mut lines = vec![
                labelled(item),
                format!(
                    "{} · {} · {}:{}",
                    item.type_name, item.status, item.path, item.id_span.line
                ),
            ];
            let rows = project.facts(item);
            let width = rows
                .iter()
                .map(|(k, _)| k.chars().count())
                .max()
                .unwrap_or(0);
            for (key, values) in rows {
                lines.push(format!("{key:width$}  {}", values.join(", ")));
            }
            lines.join("\n")
        }
    };
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::PlainText,
            value: text,
        }),
        range: Some(doc.range(start, end)),
    })
}
