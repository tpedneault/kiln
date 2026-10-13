+++
id = "REQ-007"
type = "requirement"
title = "kiln check on 2,000 items and 50k source files in 2 s"
status = "verified"
parent = "REQ-004"
+++

Cold, on a laptop. The benchmark in `crates/kiln-check/examples/bench.rs`
generates such a project and gates CI at 50% over the budget.
