+++
id = "TASK-019"
type = "task"
title = "kiln affected: the requirements a change reaches, and what to re-check"
status = "done"

[links]
part-of = ["MS-019"]
implements = ["REQ-023"]
+++

**What:** `kiln affected [REV]` reads `git diff` from `REV` to the working tree and lists the items whose code or file changed (ADR-022), with what verifies them; text, JSON or GitHub annotations.

**Done when:** changing one line under an `implements:` comment lists that requirement and its tests.
