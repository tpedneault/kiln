+++
id = "ADR-023"
type = "decision"
title = "Sign-off and history are read from git, never stored in items"
status = "accepted"
+++

## Context

The agent guide leaves sign-off (approving a requirement, accepting a decision, closing a risk, finishing a milestone) to people, and K19 makes it visible (REQ-024). Kiln could store approvals in the front matter (`approved_by`, a date), or read them from git.

## Decision

Kiln stores nothing new: git is the record. `kiln log` reads an item's history from `git log --follow` and the file at each commit; `kiln signoff` compares a revision with the working tree and names, for each sign-off status change, the last commit in between that touched the file and its author ("uncommitted" when it isn't committed yet). Which statuses are sign-off is `kiln.toml`'s `[signoff]`, type → statuses, with defaults that follow the guide's rule.

## Consequences

Nothing for an agent to forge in a file and nothing to keep in step; the evidence is as good as the repository's history, and a squash merge credits its author. A shallow clone has less history, which `kiln log` says. Who may sign off stays the repository's protected-branch and review rules: Kiln shows the change, it doesn't block it.
