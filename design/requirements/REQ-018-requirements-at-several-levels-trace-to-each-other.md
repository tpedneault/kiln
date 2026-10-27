+++
id = "REQ-018"
type = "requirement"
title = "Requirements at several levels trace to each other, and derived ones say so"
status = "implemented"
+++

Projects at work write requirements at several levels: what the customer
or the stakeholders need, what the system must do, what each piece of
software must do; for avionics, DO-178C's high- and low-level
requirements. Before K13 Kiln checked one type, `requirement`: a project
could declare a type per level and link them, but nothing checked that a
system requirement answers a stakeholder requirement, or that every
stakeholder requirement was refined, and no report showed the chain from
the top level to the code and tests.

Kiln knows the levels and their order. It reports a requirement that
answers nothing above it unless it is marked derived, one that nothing
below refines, and a link that skips a level. It shows the chains as a
traceability matrix, on the command line and in exports, with the
derived requirements apart, because that is what an assessor asks for. A
project with one level is checked as before.

From FORMAT.md, "Requirement levels" (K13).
