+++
id = "TASK-008"
type = "task"
title = "kiln init adds the design-doc export"
status = "done"

[links]
part-of = ["MS-017"]
implements = ["REQ-021"]
+++

`kiln init` adds the `design-doc` preset unless `--preset` names
another, so `kiln export` writes a design document straight away.
