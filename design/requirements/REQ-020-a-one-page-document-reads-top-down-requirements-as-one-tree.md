+++
id = "REQ-020"
type = "requirement"
title = "A one-page document reads top-down, its requirements as one tree"
status = "implemented"
+++

Exported to one Confluence page, a project with several requirement
levels read badly: each level was a chapter of its own, after whatever
types `kiln.toml` declared first, and nothing on the page showed which
system requirement answers which stakeholder requirement except a line
of links under each. Readers at work asked for the requirements as one
tree, on one page, and for each item's facts to be quick to scan.

The document reads top-down: requirements first, as one tree (each
requirement with those of the level below that refine it), then design,
decisions, verification, plan and risks; derived requirements apart.
Every item is in the table of contents, however deep the tree goes, and
each item's facts are a small table whose rows line up from item to
item. Several pages only when the template or the layout says so.

From EXPORT.md, "The requirements tree" (K15).
