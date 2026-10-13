+++
id = "REQ-015"
type = "requirement"
title = "Duplicate IDs are found and fixed by one command"
status = "verified"
parent = "REQ-003"

[links]
mitigates = ["RISK-003"]
+++

Two branches can allocate the same number. `kiln check` reports both
files (K101) and `kiln renumber` moves one, rewriting every reference
Kiln counts.
