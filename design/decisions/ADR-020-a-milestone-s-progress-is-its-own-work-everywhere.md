+++
id = "ADR-020"
type = "decision"
title = "A milestone's progress is its own work, everywhere"
status = "accepted"
+++

## Context

Milestone progress counted the work `part-of` a milestone and of every milestone it `depends-on`, transitively. K15's `kiln status` counted only a milestone's own work, so the GUI's Roadmap, the exported roadmap and `kiln show` disagreed with `kiln status` on the same milestone. The wiki already warned that the transitive count misleads whenever `depends-on` only meant the order: in this repository, MS-017 reported K15's tasks as its own.

## Decision

A milestone's progress is its own work: done out of non-closed, over the work `part-of` it directly or through its parent (a sub-task counts where its task does). One function computes it, and `kiln show`, `kiln status`, the GUI and the roadmap report all use it. `depends-on` between milestones orders them and nothing else, as ADR-018 already says for readiness.

## Consequences

Every view shows the same numbers. A project that relied on a later milestone counting the earlier one's work sees smaller totals; it can make the later milestone include that work, or read the earlier milestone's own line. The roadmap report's numbers change for such projects, so their committed exports change once.
