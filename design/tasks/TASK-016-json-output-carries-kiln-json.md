+++
id = "TASK-016"
type = "task"
title = "JSON output carries kiln_json"
status = "done"

[links]
part-of = ["MS-018"]
implements = ["REQ-022"]
+++

Every JSON object Kiln prints starts with `"kiln_json": 1`, raised when a shape changes incompatibly; arrays only gain fields.
