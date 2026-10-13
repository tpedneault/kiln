+++
id = "ADR-002"
type = "decision"
title = "Sequential IDs, renumbered on collision"
status = "accepted"
+++

IDs are the type's prefix and the next number, readable and said aloud in
meetings, over collision-free IDs nobody can remember. Two branches can
allocate the same number; `kiln check` finds it and `kiln renumber` fixes
it.
