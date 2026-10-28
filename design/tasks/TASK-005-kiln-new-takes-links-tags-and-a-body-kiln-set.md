+++
id = "TASK-005"
type = "task"
title = "kiln new takes links, tags and a body; kiln set takes tags and jira"
status = "done"

[links]
part-of = ["MS-016"]
implements = ["REQ-020"]
+++

`kiln new TYPE --link ROLE=ID… --tag TAG… --body-file PATH|-` and
`--format json` (`{"id", "path"}`); `kiln set ID tags=a,b jira=KEY`.
