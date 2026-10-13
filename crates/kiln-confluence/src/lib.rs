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
//! K2 is read-only: snapshots and `--dry-run`. Applying a plan (creating,
//! updating, labels, attachments, properties, retiring) is K3. It never
//! touches a page without its own target's `kiln` content property, and
//! never puts a token in a log, a plan or an error.

mod client;
mod cloud;
mod dc;
mod desired;
mod model;
mod normalise;
mod plan;
mod snapshot;
mod storage;

pub use client::{connect, ClientError, Confluence, Http};
pub use desired::{desired_pages, titles, DesiredContext, DesiredPage, MAX_TITLE};
pub use model::{KilnProperty, RemotePage, Snapshot};
pub use normalise::{body_hash, hash, normalise};
pub use plan::{plan, Op, OpKind, Plan, PlanOptions};
pub use snapshot::take_snapshot;
pub use storage::{emit, esc, Attachment, AttachmentSource, EmitContext, Emitted};

#[cfg(test)]
mod fake;
#[cfg(test)]
mod tests;
