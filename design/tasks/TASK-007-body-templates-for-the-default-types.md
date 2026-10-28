+++
id = "TASK-007"
type = "task"
title = "Body templates for the default types"
status = "done"

[links]
part-of = ["MS-017"]
implements = ["REQ-021"]
+++

`kiln init` writes a short body template for each default type
(requirement, design, decision, milestone, task, test, risk) under
`design/templates/` and names it in `kiln.toml`.
