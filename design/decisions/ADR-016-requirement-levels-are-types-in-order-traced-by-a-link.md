+++
id = "ADR-016"
type = "decision"
title = "Requirement levels are types in order, traced upward by a link; derived ones are marked"
status = "accepted"
+++

Requirements written at several levels needed Kiln to know which types
are levels, in which order, and how a level answers the one above.

**A type per level, ordered in `[requirements] levels`.** Not a `level`
key on each requirement, nor tags: each level has its own prefix
(`SYS-004` says what it is in a meeting), its own statuses and its own
section in the document, which types already give. Declaring the order
once in `kiln.toml` keeps it in one place. Without the table the one
level is `requirement`, so no project changes behaviour.

**`parent` within a level, a link across levels.** `parent` already means
decomposition and makes the tree and the coverage of children; reusing it
across levels would make one requirement's children a mix of finer
system requirements and the software requirements that implement it. A
link role (`refines` by default, `trace` names another) is written once
on the lower side, as every link is (ADR-007), and can point at several
requirements above, which `parent` can't. A requirement with no trace
link traces through its parent, so a decomposed subtree links upward
once.

**Derived requirements are marked, not inferred.** A lower-level
requirement that answers nothing above is either forgotten or
deliberate (DO-178C's derived requirements, which go to the safety
assessment). `derived = true` in front matter is the one way to say
"deliberate"; without it, K208. A key rather than a tag because it
changes what the check does, and tags never do.

**Each rule one fact.** K208 (nothing above), K209 (the wrong level
above), K210 (nothing below), with K201 and K202 applying to every level
their role's `to` names, so a V-model can verify every level and
implement only the lowest. A link that skips a level still counts where
it points: one wrong link is one finding, not two. The levels report
shows the leaf's coverage per chain; each level's own verification is
the traceability report with its `type`, rather than a matrix too wide
to print.
