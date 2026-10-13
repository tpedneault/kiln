+++
id = "ADR-011"
type = "decision"
title = "Word files are written directly, not through docx-rs"
status = "accepted"
+++

EXPORT.md planned the `docx` format on the `docx-rs` crate. Building it
(K8) showed that `docx-rs` numbers bookmarks, hyperlinks, pictures and
headers from process-wide counters, so the same document rendered twice
in one `kiln export` comes out with different bytes, and `kiln export
--check` (determinism, EXPORT.md) can't hold. It also pulls in an image
library Kiln doesn't need, and has no notion of a reference document's
styles.

Kiln writes the package itself: WordprocessingML for what the document
tree holds (headings, paragraphs, lists, tables, links, bookmarks,
pictures, a TOC field), in a zip written by the `zip` crate with its
parts in a fixed order and a fixed time. A reference `.docx` is read with
the same crate, and its styles, theme, numbering and page setup are
copied over, with Kiln's missing styles added. About a thousand lines,
every one of them testable as text.

The cost: Kiln owns the subset of the format it writes, and a construct
Word users expect that the tree doesn't have (footnotes, headers and
footers) needs writing by hand. The tree is small and changes rarely, so
that is the cheaper side.
