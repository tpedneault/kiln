//! What Kiln needs from git: the current commit and whether the tree is
//! dirty, the last commit that changed each file (one
//! `git log --format=%H --name-only` pass), whether the clone is shallow,
//! file contents at a revision, and tags for baselines.
//!
//! Shells out to `git`, as `fenix-git` does, rather than linking a git
//! library: the user's own git already honours their credentials,
//! configuration and repository format, and CI images have it. The cost
//! is a process per query, which is why queries are batched.
//!
//! I/O, read-only: it never commits, checks out or changes a ref.
//!
//! Arrives in K1 (commit id only), grows in K2 (last commit per file,
//! shallow detection) and K6 (revisions, baselines); a shell until then.
