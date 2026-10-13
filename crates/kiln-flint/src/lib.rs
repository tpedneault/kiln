//! `kiln provider`: a Flint provider over stdio, built on
//! `flint-protocol`'s types, that registers a project's items as Flint
//! static items and answers the `status`, `impact` and `all` drill-downs
//! (docs/INTEGRATION.md, "Kiln as a Flint provider").
//!
//! Static items rather than answering every query: Flint matches and
//! ranks them itself, so ordinary typing costs no round trip to Kiln. The
//! price is re-sending the item list when the project changes, capped at
//! one `provider/setItems` per 250 ms.
//!
//! It deliberately never writes Flint's or Fenix's files; actions reach
//! Fenix through its control channel (`kiln-fenix`).
//!
//! Arrives in K5; a shell until then.
