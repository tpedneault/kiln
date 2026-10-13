//! The Confluence back end (docs/CONFLUENCE.md): the storage-format
//! emitter, the normaliser used for drift detection, the planner, the
//! `Confluence` client trait with its Cloud and Data Center
//! implementations (`ureq`, native certificates, the system proxy), and
//! apply.
//!
//! The emitter and planner are pure: the plan is a function of desired
//! pages and a snapshot of remote ones, tested table-driven, so the
//! decisions (create, update, unchanged, retire, refuse on drift) are
//! checked without a server. Only the client does I/O, and apply is
//! tested against an in-process fake Confluence of both flavours. Every
//! endpoint Kiln calls is listed in CONFLUENCE.md; nothing else is used.
//!
//! One-way by design: it never merges edits made in Confluence back into
//! the repository, only reports them. It never touches a page without its
//! own target's `kiln` content property, and never puts a token in a log,
//! a plan or an error.
//!
//! Arrives in K2 (emitter, planner, read-only calls, `--dry-run`) and K3
//! (writes); a shell until then.
