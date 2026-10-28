+++
id = "TASK-006"
type = "task"
title = "The agent guide describes the plan and how to work through it"
status = "done"

[links]
part-of = ["MS-016"]
implements = ["REQ-020"]
depends-on = ["TASK-003", "TASK-005"]
+++

The generated section gains "Planning": the work types, which statuses
are done, `kiln next` and `kiln status`, and the plan checks. The rules
`kiln init` writes say how to take and finish a task; `kiln init` and
`kiln agents` print the line to add to the repository's own guide.
