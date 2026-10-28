+++
id = "REQ-020"
type = "requirement"
title = "Coding agents read the plan and act on it from the command line"
status = "verified"
+++

An agent asked to "do the next task" or "implement REQ-014" works from
what Kiln prints. It must be able to read an item whole (its body, not
only its title and links), load the plan in one call it can parse, ask
which work is ready to start, and record what it did (create an item
with its links, tags and body; change tags) in one command each, without
editing front matter by hand. When the plan contradicts itself (a done
milestone with open work, a done task whose dependency isn't), `kiln
check` says so, since an agent moving statuses makes that mistake
quietly.

The agent guide says how the plan is organised and how to work through
it, generated from `kiln.toml` like the rest of its section.

From DESIGN.md, "Milestones", K15.
