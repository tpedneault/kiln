//! Talking to a running Fenix through its control channel
//! (`fenix-control`). K1: open a file at a line, for `kiln new --open`.
//! When Fenix isn't running it starts `fenix <path>:<line>`; when Fenix
//! isn't installed it falls back to the OS default editor and says so
//! (docs/INTEGRATION.md, "Opening items in Fenix"). Agenda lookups and
//! clock-in arrive with K5.
//!
//! Capabilities, not versions: it asks Fenix what it can do (`fenix/hello`)
//! and refuses with the reason when `fenix/open` is missing, so a newer
//! or older Fenix degrades instead of failing obscurely.
//!
//! It deliberately never writes Fenix's files (agenda, notebook,
//! settings); anything Kiln needs that the channel lacks is a G-item in
//! INTEGRATION.md, not a workaround here.

use std::path::Path;
use std::process::{Command, Stdio};

use fenix_control::protocol::method;
use fenix_control::{Client, Endpoint, OpenParams, OpenPath};

/// How the file was opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opened {
    /// In the running Fenix.
    Fenix,
    /// Fenix wasn't running; started it with the file.
    StartedFenix,
    /// Fenix isn't installed; handed to the OS default editor.
    DefaultEditor,
}

impl Opened {
    /// One line for the user, naming the fallback when there was one.
    pub fn describe(&self) -> &'static str {
        match self {
            Opened::Fenix => "opened in Fenix",
            Opened::StartedFenix => "started Fenix with it",
            Opened::DefaultEditor => "Fenix isn't installed; opened with the default editor",
        }
    }
}

/// Opens `path` at `line` (1-based): the running Fenix, else a new
/// Fenix, else the OS default editor.
pub fn open(path: &Path, line: usize) -> Result<Opened, String> {
    if open_in_running(&Endpoint::current(), path, line)? {
        return Ok(Opened::Fenix);
    }
    let spawn = Command::new("fenix")
        .arg(format!("{}:{line}", path.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    match spawn {
        Ok(_) => Ok(Opened::StartedFenix),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            default_editor(path).map(|()| Opened::DefaultEditor)
        }
        Err(e) => Err(format!("couldn't start fenix: {e}")),
    }
}

/// `fenix/open` on the Fenix serving `endpoint`. `Ok(false)` when none
/// is; an error when one is but can't or won't open the file.
pub fn open_in_running(endpoint: &Endpoint, path: &Path, line: usize) -> Result<bool, String> {
    let client = Client::connect(endpoint)
        .map_err(|e| format!("couldn't reach Fenix's control channel: {e}"))?;
    let Some(mut client) = client else {
        return Ok(false);
    };
    let hello = client
        .hello("kiln", env!("CARGO_PKG_VERSION"))
        .map_err(|e| format!("Fenix's control channel: {e}"))?;
    if !hello.capabilities.iter().any(|c| c == method::OPEN) {
        return Err(format!(
            "Fenix {} can't open files over its control channel (no {})",
            hello.version,
            method::OPEN
        ));
    }
    let params = OpenParams {
        paths: vec![OpenPath {
            path: path.to_string_lossy().into_owned(),
            line: Some(line),
            column: Some(1),
        }],
        new_window: false,
        focus: true,
    };
    client
        .open(&params)
        .map_err(|e| format!("Fenix couldn't open {}: {e}", path.display()))?;
    Ok(true)
}

fn default_editor(path: &Path) -> Result<(), String> {
    let mut command = if cfg!(windows) {
        let mut c = Command::new("cmd");
        // `start` treats its first quoted argument as a window title.
        c.args(["/C", "start", ""]).arg(path);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(path);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(path);
        c
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
        .map_err(|e| format!("Fenix isn't installed and no default editor could be started: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fenix_control::{serve, Call, ServerInfo};
    use serde_json::Value;
    use std::sync::mpsc;

    fn endpoint(name: &str) -> Endpoint {
        Endpoint::instance(&format!("kiln-test-{name}-{}", std::process::id()))
    }

    #[test]
    fn nobody_listening_is_not_an_error() {
        assert_eq!(
            open_in_running(&endpoint("none"), Path::new("a.md"), 3),
            Ok(false)
        );
    }

    #[test]
    fn a_running_fenix_gets_the_path_and_line() {
        let endpoint = endpoint("open");
        let mut listener = endpoint.bind().unwrap();
        let (tx, rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let stream = listener.accept().unwrap();
            let writer = stream.try_clone().unwrap();
            serve(stream, writer, &ServerInfo::new("9.9.9", 1), |call| {
                if let Call::Open(params) = call {
                    tx.send(params).unwrap();
                }
                Ok(Value::Null)
            })
            .unwrap();
        });
        assert_eq!(
            open_in_running(&endpoint, Path::new("design/REQ-001.md"), 4),
            Ok(true)
        );
        let params = rx.recv().unwrap();
        assert_eq!(params.paths[0].path, "design/REQ-001.md");
        assert_eq!(params.paths[0].line, Some(4));
        server.join().unwrap();
    }

    #[test]
    fn a_fenix_without_open_is_refused_with_the_reason() {
        let endpoint = endpoint("noopen");
        let mut listener = endpoint.bind().unwrap();
        let server = std::thread::spawn(move || {
            let stream = listener.accept().unwrap();
            let writer = stream.try_clone().unwrap();
            let mut info = ServerInfo::new("0.1.0", 1);
            info.capabilities.retain(|c| c != method::OPEN);
            serve(stream, writer, &info, |_| Ok(Value::Null)).unwrap();
        });
        let err = open_in_running(&endpoint, Path::new("a.md"), 1).unwrap_err();
        assert!(err.contains("can't open files"), "{err}");
        server.join().unwrap();
    }
}
