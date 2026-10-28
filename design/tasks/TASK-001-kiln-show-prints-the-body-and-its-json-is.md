+++
id = "TASK-001"
type = "task"
title = "kiln show prints the body, and its JSON is structured"
status = "done"

[links]
part-of = ["MS-016"]
implements = ["REQ-020"]
+++

`kiln show ID` prints the body under the rows. `--format json` gives
links, inbound links, children and parent as objects (`id`, `title`,
`status`; code locations as `path` and `line`), coverage as booleans
by role, progress as numbers, and the body.
