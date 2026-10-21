+++
id = "REQ-017"
type = "requirement"
title = "The GUI opens any project from a picker and switches between them"
status = "implemented"
+++

Before K12, `kiln gui` (and `kilnw`) found its project by walking up from
the current folder to a `kiln.toml`, and stopped with "no kiln.toml in …
or any folder above it" when there was none. So the window couldn't be
started from a shortcut, the Start menu or Flint without first being in
a project, and looking at a second project meant a second window from a
second folder.

The GUI starts anywhere: outside a project it opens on Home, a picker of
the projects opened before (most recent first) with a way to add another
folder, and inside one `SPC h` goes back to Home to switch to another
project without restarting. The list of projects is the user's, not the
project's, so it is never in a repository. It lives in Kiln's state
folder rather than beside `settings.toml`: its paths are this machine's,
and on Windows the settings roam.

From GUI.md, "Home" (K12).
