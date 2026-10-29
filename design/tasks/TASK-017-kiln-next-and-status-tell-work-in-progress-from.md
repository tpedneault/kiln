+++
id = "TASK-017"
type = "task"
title = "kiln next and status tell work in progress from work ready to start"
status = "done"
owner = "claude"

[links]
part-of = ["MS-019"]
implements = ["REQ-023"]
+++

**What:** work past its first status is "in progress" in `kiln status` and on the GUI, and `kiln next` lists only work not started yet.

**Done when:** a doing task is under "in progress", not "ready", and `kiln next` leaves it out.
