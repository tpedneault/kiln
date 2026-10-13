//! Talking to a running Fenix through its control channel
//! (`fenix-control`): open a file at a line, look up agenda tasks, clock
//! in. When Fenix isn't running it starts `fenix <path>:<line>`; when
//! Fenix isn't installed it falls back to the OS default editor and says
//! so (docs/INTEGRATION.md, "Opening items in Fenix").
//!
//! Capabilities, not versions: it asks Fenix what it can do (`fenix/hello`)
//! and disables an action with the reason when something is missing, so a
//! newer or older Fenix degrades instead of failing.
//!
//! It deliberately never writes Fenix's files (agenda, notebook,
//! settings); anything Kiln needs that the channel lacks is a G-item in
//! INTEGRATION.md, not a workaround here.
//!
//! Arrives with `kiln new --open` in K1 and grows in K5; a shell until
//! then.
