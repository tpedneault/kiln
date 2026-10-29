+++
id = "TASK-021"
type = "task"
title = "kiln signoff: sign-off changes since a revision, with their authors"
status = "todo"

[links]
part-of = ["MS-020"]
implements = ["REQ-024"]
+++

**What:** `kiln signoff [REV]` lists every change to a sign-off status (`[signoff]` in kiln.toml, defaults after the agent guide's rule) between `REV` and the working tree, with the commit and author; text, JSON, GitHub or GitLab annotations.

**Done when:** a pull request that approves a requirement shows a warning annotation naming the commit and its author.
