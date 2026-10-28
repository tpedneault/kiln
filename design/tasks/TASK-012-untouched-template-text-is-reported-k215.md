+++
id = "TASK-012"
type = "task"
title = "Untouched template text is reported (K215)"
status = "done"

[links]
part-of = ["MS-018"]
implements = ["REQ-022"]
+++

An item past its first status whose body still has a line of its type's template, word for word (not a heading), gets a warning on that line, so prompts like "**What:** the change…" don't reach the exported document.
