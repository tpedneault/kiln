+++
id = "REQ-001"
type = "requirement"
title = "The repository is the source of truth"
status = "approved"
+++

Every fact Kiln shows comes from files in the repository, plus git
history and, read-only, Jira. No database, no server: deleting Kiln loses
no information.

From DESIGN.md, goal 1.
