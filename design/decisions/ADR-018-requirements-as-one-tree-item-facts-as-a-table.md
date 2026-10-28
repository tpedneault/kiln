+++
id = "ADR-018"
type = "decision"
title = "Requirements are one tree on the page, and an item's facts are a table"
status = "accepted"

[links]
supersedes = ["ADR-013"]
+++

ADR-013 made items a flat list and their properties lines of bold
labels, so that a three-level table of contents listed every item and a
short item took little room. With requirement levels (ADR-016) that
left a design document of separate chapters per level and a page where
the relationship readers care most about, which requirement refines
which, showed only as links (REQ-020).

**The requirements tree.** `<!-- kiln:requirements -->`, and the
default layout, place the requirement levels as one tree: each top-level
requirement, the requirements of the level below that refine it one
heading deeper, and so on, then the derived requirements and those that
trace to nothing, each under a heading of their own. Within a level
ADR-013 still holds: a child follows its parent at the same depth, so
the tree is as deep as there are levels, not as deep as the
decomposition. A requirement that refines several is placed under the
first and named, in one line, under the others: a heading in two places
would be two anchors for one item, and Confluence allows one page per
item in `tree`. The table of contents is as deep as the deepest item
(three levels at least, six at most) unless `toc_levels` says, so
ADR-013's reason for flatness holds without the flatness.

**Properties as a table.** Each item's facts are a two-column table
(status first, then relationships, Jira, tags, owner, due) with related
items by ID alone, linked to their heading. Rows line up across items
and an item with five relationships stays five short rows, where the
lines grew long with titles; the title is one click away. The type and
the source file appear only in `tree`, where an item is a page alone;
in a document its section already says both. `list` and `line` remain
for documents read as prose.

**Requirements first.** Without a template, and in the `design-doc`
preset, the requirements come first, then the other types in
`kiln.toml`'s order, so a project that declares its levels last still
reads top-down; `traceability` without `type` is the chain matrix when
there are several levels.
