+++
id = "TASK-018"
type = "task"
title = "JUnit results count toward verification: --results, K216 and K217"
status = "done"

[links]
part-of = ["MS-019"]
implements = ["REQ-023"]
+++

**What:** `kiln trace`, `kiln check` and `kiln export` take `--results FILE` (JUnit XML, repeatable); a verifying code reference counts only when its test passed (ADR-021); K216 a verifying test failed, K217 one did not run.

**Done when:** a failing test leaves its requirement uncovered in `kiln trace` and K216 names its line.
