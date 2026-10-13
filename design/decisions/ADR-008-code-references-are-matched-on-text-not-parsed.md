+++
id = "ADR-008"
type = "decision"
title = "Code references are matched on text, not parsed"
status = "accepted"
+++

Source files are scanned with the configured regexes over whole text, not
parsed per language. A match inside a string literal counts; in exchange
the scanner works for any language and stays within the check budget.
Kiln's own tests build such text at run time for that reason.
