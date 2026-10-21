+++
id = "ADR-014"
type = "decision"
title = "Kiln pushes the tasks it manages to Jira"
status = "accepted"
+++

Through K8 Kiln only read Jira: a task changed in Kiln left its ticket
behind, and every change was made twice, once in the repository and
once in Jira. K207 noticed the difference but couldn't remove it.

From K10, with `[jira] project` set, `kiln jira sync` keeps Jira in step
with the items, one way, as publishing keeps Confluence in step
(ADR-003). One Jira issue stands for the Kiln project (`[jira] parent`,
created by the first sync), and each synced item, the tasks by default,
is a sub-task of it. Kiln creates those sub-tasks and keeps their
summary, due date and status: the status by the workflow transition that
reaches the mapped status (`[jira.status]`, else Jira's status category).
A ticket created for an item is recorded in the item (`jira = "…"`), and
the project issue in `kiln.toml`, so the commit records the link and no
ticket is ever created twice; a person runs the sync, not CI, for that
reason.

What stays Jira's: assignees, sprints, comments, worklogs, and every
ticket that isn't a sub-task of the project issue, which Kiln never
writes even when an item links it. Kiln deletes nothing: a sub-task no
item links any more is reported. A ticket moved in Jira is flagged by
K207; `kiln jira pull` takes its status back into the item as a
reviewable edit, or the next sync moves it back. The repository stays
the source of truth (REQ-001), and Kiln still holds no assignments or
sprints (RISK-004).
