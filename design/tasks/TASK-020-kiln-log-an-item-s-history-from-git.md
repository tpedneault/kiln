+++
id = "TASK-020"
type = "task"
title = "kiln log: an item's history from git"
status = "done"
owner = "claude"

[links]
part-of = ["MS-020"]
implements = ["REQ-024"]
+++

**What:** `kiln log ID` lists each commit that changed the item's status, title, owner, parent, tags, due date or links, oldest first, with date and author, and uncommitted changes last (ADR-023).

**Done when:** an item approved in one commit and linked in another shows both lines with their authors.
