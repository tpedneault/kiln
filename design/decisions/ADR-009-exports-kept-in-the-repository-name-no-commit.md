+++
id = "ADR-009"
type = "decision"
title = "Exports kept in the repository name no commit"
status = "accepted"
+++

EXPORT.md wanted a committed Markdown export kept current by
`kiln export --check`, but also had every export name the exported commit
in its summary and link repository files at the last commit that changed
them. A file committed into the repository can't name the commit that
contains it, so the check could never pass. Found while making this
repository's own design document (K2).

A file export inside the repository and not ignored by git is in-tree: it
links repository files with relative paths, which GitLab and GitHub
resolve at the revision being viewed, and its summary names no commit.
Exports outside the repository (Confluence, a file under `target/`) keep
naming commits.
