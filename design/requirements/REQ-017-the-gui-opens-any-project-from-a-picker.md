+++
id = "REQ-017"
type = "requirement"
title = "The GUI opens any project from a picker and switches between them"
status = "draft"
+++

Today `kiln gui` (and `kilnw`) finds its project by walking up from the
current folder to a `kiln.toml`, and stops with "no kiln.toml in … or any
folder above it" when there is none. So the window can't be started from
a shortcut, the Start menu or Flint without first being in a project,
and looking at a second project means a second window from a second
folder.

The GUI should start anywhere: outside a project it opens on a picker of
the projects opened before (most recent first) and a way to open another
folder, and inside one it can switch to another project from the same
picker without restarting. The list of recent projects is the user's,
not the project's, so it lives with the user's settings, never in a
repository.

Noted for later, after the Kiln mark and icon; not yet planned into a
milestone.
