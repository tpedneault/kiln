+++
id = "TASK-010"
type = "task"
title = "KILN_EDITOR, VISUAL and EDITOR open items outside Fenix"
status = "todo"

[links]
part-of = ["MS-017"]
implements = ["REQ-021"]
+++

`kiln new --open` and the GUI open an item with `KILN_EDITOR` when it
is set, then Fenix, then `VISUAL` or `EDITOR`, then the system's
default; `{path}` and `{line}` in the command are replaced.
