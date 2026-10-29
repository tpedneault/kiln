+++
id = "ADR-013"
type = "decision"
title = "Items are a flat list, with one line per relationship"
status = "superseded"
+++

Exports nested items by `parent`: a child was a heading one level below
its parent. Read in Confluence, that hid most requirements from the table
of contents, which stops at three levels (`toc_levels`): a section is
level 1, its items level 2, and every child below that was missing. And
an item's properties were one line (`approved · under REQ-002 · verified
by TEST-031, TEST-032 · implemented by …`), which is compact but hard to
scan once an item has more than two relationships.

Now the items of a `kiln:items` are a flat list of headings at one
level, in tree order, each parent followed by its children, so the
table of contents lists every item and related items still sit
together; the parent shows in the item's properties. `nest="parent"`
keeps the old nesting where a document wants it, and `tree` nests by
default, because there nesting is what makes the page tree. Properties
default to `item_style = "list"`: the status, then one line per
relationship with its label in bold, broken inside one paragraph so the
block stays short. `line` remains for documents that want the compact
form.
