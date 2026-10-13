//! `kilnw`: the GUI without a console window, for shortcuts and Flint's
//! "Open in Kiln" (docs/GUI.md, "Windows: two executables").
//!
//! `kiln.exe` stays a console program, so scripts and CI get its output
//! and exit codes. This GUI-subsystem program starts `kiln gui` beside it
//! with its arguments and no console, and waits for it. With no console
//! to print to, a failure to open the window goes to `kilnw.log` in the
//! temporary folder, the one place a person can find it afterwards.

#![cfg_attr(not(test), windows_subsystem = "windows")]

use std::process::{Command, ExitCode, Stdio};

fn main() -> ExitCode {
    let kiln = std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent()
                .map(|d| d.join(format!("kiln{}", std::env::consts::EXE_SUFFIX)))
        })
        .filter(|p| p.is_file())
        .unwrap_or_else(|| "kiln".into());
    let mut command = Command::new(kiln);
    command
        .arg("gui")
        .args(std::env::args_os().skip(1))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let outcome = command.output();
    let failure = match &outcome {
        Ok(out) if out.status.success() => return ExitCode::SUCCESS,
        Ok(out) => String::from_utf8_lossy(&out.stderr).into_owned(),
        Err(e) => format!("kilnw: couldn't start kiln: {e}\n"),
    };
    eprint!("{failure}");
    let _ = std::fs::write(std::env::temp_dir().join("kilnw.log"), &failure);
    ExitCode::FAILURE
}
