+++
id = "ADR-021"
type = "decision"
title = "Test results are matched to code references by the name of the test that follows them"
status = "accepted"
+++

## Context

`--results` reads a JUnit XML report, the format every test runner can write (cargo-nextest, pytest, JUnit, Jest, go-junit-report), and must tell which code reference each test result belongs to. Code references are matched on text, not parsed (ADR-008), so Kiln doesn't know where a test function starts.

## Decision

A `verifies` code reference's test is named by the first declaration after the comment: the name after `fn`, `def`, `func`, `function` or `sub`; else the string in `it("…")`, `test("…")` or `describe("…")`; else the identifier before the first `(`. Attribute and decorator lines (`#[…]`, `@…`) and blank lines are skipped. A JUnit `testcase` matches when its name, or its last `::`, `.` or `/` segment, equals that name; when the report gives a `file`, it must also end with the reference's path. A reference all of whose matches passed counts; one with a failure or error doesn't (K216); one with none, or only skipped ones, didn't run (K217).

## Consequences

It works across languages and runners without a parser, and an explicit name is never needed for the usual layout (the comment on the line before the test). Two tests of the same name in different files share their results unless the report names files; the safer reading wins (any failure is a failure). Without `--results`, nothing changes: a code reference counts as before.
