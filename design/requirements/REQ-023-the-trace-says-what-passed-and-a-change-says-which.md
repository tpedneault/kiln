+++
id = "REQ-023"
type = "requirement"
title = "The trace says what passed, and a change says which requirements it touches"
status = "verified"
+++

**Rationale:** a `verifies:` comment counted as verification whether its test passed or not, and a reviewer of a code change had to work out by hand which requirements it touched. At work the trace is evidence; it should say what passed in this run, and a pull request should say which requirements its code changes reach.

**Acceptance:** with `--results` and the CI's JUnit report, `kiln trace` and `kiln check` count a code reference that verifies only when its test passed, and report a failed or missing test (K216, K217); `kiln affected REV` lists the requirements whose implementing or verifying code changed since `REV`, with what to re-check, as text, JSON or CI annotations.

From DESIGN.md, "Milestones", K18.
