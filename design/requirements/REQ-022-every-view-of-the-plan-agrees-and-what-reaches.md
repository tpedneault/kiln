+++
id = "REQ-022"
type = "requirement"
title = "Every view of the plan agrees, and what reaches readers is finished"
status = "verified"
+++

**Rationale:** after K15 and K16, a person on the GUI and an agent on the command line could see different progress for the same milestone, two agents could take the same task, template prompts could reach the exported document, the default document left the plan out, and a script couldn't tell which JSON shape it was reading.

**Acceptance:** the GUI, `kiln show`, `kiln status` and the roadmap report give one progress per milestone; the GUI marks ready and waiting work as `kiln next` does; `kiln next --unowned` leaves claimed work out; `kiln check` reports template text left in an item past its first status (K215); the `design-doc` export places tasks and tests; every JSON object Kiln prints carries `kiln_json`.

From DESIGN.md, "Milestones", K17.
