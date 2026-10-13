+++
id = "ADR-006"
type = "decision"
title = "One spelling per ID number, at least three digits"
status = "accepted"
+++

FORMAT.md said "no leading zeros" while every example used three digits.
Found while writing this folder (K1): IDs are written with at least three
digits, zero-padded, and exactly one spelling per number, so `grep` finds
an ID as typed and IDs sort as text up to 999. Another spelling is a
finding that names the right one.
