+++
id = "REQ-014"
type = "requirement"
title = "No secrets in kiln.toml"
status = "verified"
parent = "REQ-005"
+++

`kiln.toml` is committed, so a key named like a credential is an error
(K004).
