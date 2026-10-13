+++
id = "ADR-005"
type = "decision"
title = "Shell out to git"
status = "accepted"
+++

`kiln-git` runs the user's `git` rather than linking a git library, as
`fenix-git` does: it already honours their credentials, configuration
and repository format, and CI images have it.
