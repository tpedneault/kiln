+++
id = "REQ-024"
type = "requirement"
title = "Sign-off is visible: who moved which status, when, in which change"
status = "verified"
+++

**Rationale:** the agent guide leaves sign-off to people, but nothing showed when a status that is someone's call changed, or who changed it. Regulated work asks when a requirement was approved, by whom and in which change; git already knows, and Kiln should say it.

**Acceptance:** `kiln log ID` prints an item's history from git: each commit that changed its status, title, owner, parent, tags, due date or links, with date and author. `kiln signoff REV` lists every sign-off status change since `REV` (requirement approved, decision accepted, risk closed, milestone done by default; `[signoff]` in kiln.toml to change it) with the commit and its author, as text, JSON or CI annotations.

From DESIGN.md, "Milestones", K19.
