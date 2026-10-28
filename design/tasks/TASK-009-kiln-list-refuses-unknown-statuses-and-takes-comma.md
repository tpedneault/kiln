+++
id = "TASK-009"
type = "task"
title = "kiln list refuses unknown statuses and takes comma lists"
status = "todo"

[links]
part-of = ["MS-017"]
implements = ["REQ-021"]
+++

`--status`, `--type` and `--tag` take comma-separated lists; a status
no listed type has is a usage error (exit 2), as an unknown type already
is.
