//! The Confluence back end (docs/CONFLUENCE.md): the storage-format
//! emitter, the normaliser used for drift detection, desired pages with
//! their `renderHash`, the planner, snapshots (live or from a file), and
//! the `Confluence` client trait with its Cloud and Data Center
//! implementations (`ureq`, native certificates, the system proxy).
//!
//! The emitter and planner are pure: the plan is a function of desired
//! pages and a snapshot of remote ones, tested table-driven, so the
//! decisions (create, update, unchanged, retire, refuse on drift) are
//! checked without a server. Only the client does I/O, and it is tested
//! against an in-process fake Confluence of both flavours. Every endpoint
//! Kiln calls is listed in CONFLUENCE.md; nothing else is used.
//!
//! Apply carries a plan out in CONFLUENCE.md's order (creates
//! parent-first, then bodies with their attachments, labels and property,
//! then retires and prunes, the root page last), reacting to 409, 413 and
//! drift found mid-way. It never touches a page without its own target's
//! `kiln` content property, and never puts a token in a log, a plan or an
//! error.

mod apply;
mod cache;
mod client;
mod cloud;
mod dc;
mod desired;
mod model;
mod normalise;
mod plan;
mod snapshot;
mod storage;

pub use apply::{apply, Applied, ApplyOptions};
pub use cache::{CachedPage, PageCache};
pub use client::{connect, ClientError, Confluence, Created, ErrorKind, Http, Identity};
pub use desired::{desired_pages, titles, DesiredContext, DesiredPage, MAX_TITLE};
pub use model::{KilnProperty, RemotePage, Snapshot};
pub use normalise::{body_hash, hash, normalise};
pub use plan::{plan, Op, OpKind, Plan, PlanOptions};
pub use snapshot::take_snapshot;
pub use storage::{emit, esc, Attachment, AttachmentSource, EmitContext, Emitted};

/// The fake server, for this crate's tests and, with the `fake` feature,
/// for the `kiln` binary's end-to-end tests. Not for anything else.
#[cfg(any(test, feature = "fake"))]
pub mod fake;
#[cfg(test)]
mod tests;
