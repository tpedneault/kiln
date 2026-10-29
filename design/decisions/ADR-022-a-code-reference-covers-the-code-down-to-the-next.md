+++
id = "ADR-022"
type = "decision"
title = "A code reference covers the code down to the next one"
status = "accepted"
+++

## Context

`kiln affected` turns a git diff into the requirements it touches. A code reference is a comment on the line before the code it is about, and Kiln doesn't parse code (ADR-008), so where that code ends isn't known.

## Decision

A code reference covers its own line down to the line before the next code reference in the same file, or the end of the file. A changed line inside that range touches the reference's targets; a deletion touches the range it was in. An item file that changed touches that item. Each touched requirement is listed with the changed ranges and the references that reached it, and with what verifies it, to re-check.

## Consequences

It over-reports rather than misses: a reference at the top of a file covers the whole file, which is what a file-level `implements` means. Precision comes from placing references close to their code, which the format already asks for. No language support is needed, and the answer is the same on every machine.
