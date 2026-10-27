+++
id = "REQ-019"
type = "requirement"
title = "Coding agents find how this project's design is organised and how to change it"
status = "implemented"
+++

Coding agents now implement much of what a design describes, and change
the design too. Without guidance they edit front matter by hand, choose
IDs, write inverse links and edit generated exports: work `kiln check`
then rejects, or worse, accepts and the next export contradicts.

A Kiln project carries a guide that such agents read where they work: how
to read and change the design through Kiln's commands, what they may and
may not decide, and how this project's design is organised (its types,
folders, statuses, links, levels and exports). The organisation part
always matches `kiln.toml`, and CI says when it doesn't; the rules are
the project's to adapt.

From DESIGN.md, "The agent guide" (K14).
