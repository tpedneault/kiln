//! The binary's contract as a process sees it: output streams and exit
//! codes (DESIGN.md, "Command line"). Spawns the built `kiln`, so it also
//! proves the binary starts headless, with no display or console.

use std::process::{Command, Output};

fn kiln(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(args)
        .output()
        .expect("the kiln binary runs")
}

#[test]
fn version_prints_name_and_version_and_exits_0() {
    let out = kiln(&["--version"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        stdout.trim_end(),
        format!("kiln {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn help_exits_0() {
    let out = kiln(&["--help"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8(out.stdout)
        .unwrap()
        .contains("Usage: kiln"));
}

#[test]
fn no_arguments_prints_help_and_exits_2() {
    let out = kiln(&[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8(out.stderr)
        .unwrap()
        .contains("Usage: kiln"));
}

#[test]
fn unknown_command_is_one_line_on_stderr_and_exits_2() {
    let out = kiln(&["frobnicate"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1, "{stderr:?}");
    assert!(stderr.starts_with("kiln: "), "{stderr:?}");
    assert!(stderr.contains("frobnicate"), "{stderr:?}");
}
