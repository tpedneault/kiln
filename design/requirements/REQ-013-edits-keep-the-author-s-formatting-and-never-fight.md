+++
id = "REQ-013"
type = "requirement"
title = "Edits keep the author's formatting and never fight an editor"
status = "verified"
parent = "REQ-001"
+++

`kiln set`, `link`, `unlink` and `renumber` change only the keys or text
they are about, keep comments and layout, keep the file's line endings,
write atomically, and refuse a file that changed since Kiln read it.
