+++
id = "TASK-003"
type = "task"
title = "kiln next and kiln status: ready work and the plan by milestone"
status = "done"

[links]
part-of = ["MS-016"]
implements = ["REQ-020"]
+++

`kiln next [--milestone ID]` lists ready work (ADR-018), earliest
milestone first. `kiln status` shows each open milestone with its
progress, ready and waiting work (and what it waits on), then work in
no milestone and the count of `kiln check` findings.
