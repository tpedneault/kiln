+++
id = "MS-020"
type = "milestone"
title = "K19: sign-off and history from git"
status = "active"

[links]
depends-on = ["MS-019"]
+++

**Delivers:** K19: `kiln log` (an item's history from git) and `kiln signoff` (sign-off status changes since a revision, with their authors, as CI annotations), with `[signoff]` in kiln.toml.

**Done when:** each task below is done and `kiln check` is clean on this repository.
