+++
id = "ADR-017"
type = "decision"
title = "The agent guide lives in the design folder, its rules written once and its description generated"
status = "accepted"
+++

A Kiln project needed a guide for coding agents (REQ-019).

**`design/AGENTS.md`, not beside `kiln.toml`.** `kiln.toml` is usually at
the repository root, where `AGENTS.md` is the repository's guide to its
code: not Kiln's to write, and Kiln never overwrites a file. In the
first item folder the guide never collides, sits beside the files it
describes, and is read by agents that honour nested `AGENTS.md` files
when they work there. The cost: an agent working only on the code may not
see it, so the wiki suggests one line in the repository's own guide
pointing at it.

**Two parts.** The rules (use Kiln's commands, link from the source side,
leave sign-off to people…) are written once and are the project's from
then on, like a preset's template (ADR-015): teams differ on what an agent
may approve, and a newer Kiln rewriting their words would be a surprise.
The description of the project (types, folders, statuses, roles, levels,
exports) is generated between two marker lines from `kiln.toml` alone,
because a hand-kept copy goes stale the first time someone adds a type,
and a stale guide misleads an agent more than none. `kiln agents`
rewrites only that section, `kiln preset add` refreshes it, and `kiln
check` warns when it no longer matches (K113), as `kiln export --check`
does for exports. Generating it from the configuration alone, not the
items, keeps it stable across ordinary work and exact to compare.

**Optional.** A project that deletes the section, or the file, has opted
out, and K113 says nothing; a warning by default, since a stale guide
doesn't break the design.
