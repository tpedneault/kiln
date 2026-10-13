+++
id = "REQ-016"
type = "requirement"
title = "Changes since a baseline are shown item by item"
status = "implemented"
parent = "REQ-002"
+++

A reader who saw the document at a release asks what changed since.
`kiln diff v1.0` lists the items added, removed and changed (status,
title, links, body…), and `kiln export --since v1.0` marks each new or
changed item where it stands and lists them in the changes report.

From DESIGN.md, "Baseline" (K6).
