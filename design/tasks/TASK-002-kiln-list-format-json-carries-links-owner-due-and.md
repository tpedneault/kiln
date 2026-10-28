+++
id = "TASK-002"
type = "task"
title = "kiln list --format json carries links, owner, due and, with --body, the body"
status = "done"

[links]
part-of = ["MS-016"]
implements = ["REQ-020"]
+++

Each item in `kiln list --format json` gains `owner`, `due`, `links`
(role → IDs) and `done`/`closed`; `--body` adds the body, so the whole
plan loads in one call.
