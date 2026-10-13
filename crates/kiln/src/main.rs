//! The `kiln` binary: the command line (`clap`) over the library crates.
//! It owns what only a process has: arguments, stdout and stderr, exit
//! codes, finding the project by walking up to `kiln.toml`. Everything
//! else is a library call, so commands stay thin and their logic stays
//! testable without spawning a process.
//!
//! Conventions every command keeps (DESIGN.md, "Command line"): errors
//! are one line on stderr naming what failed and why, with file and line
//! when there is one; exit codes are 0 fine, 1 findings or a refused plan,
//! 2 usage or configuration error, 3 network or server error.
//!
//! K0 ships only `--help` and `--version`; the commands arrive from K1
//! (docs/CLI.md). With no arguments it prints help and exits 2, until K7
//! gives that invocation the GUI.

use std::process::ExitCode;

use clap::error::ErrorKind;
use clap::Parser;

/// Usage or configuration error.
const EXIT_USAGE: u8 = 2;

#[derive(Parser)]
#[command(
    name = "kiln",
    version,
    about = "Requirements, design decisions and project progress, kept as Markdown in the repository they describe",
    arg_required_else_help = true
)]
struct Cli {}

fn main() -> ExitCode {
    match Cli::try_parse() {
        // Unreachable until the first command exists: no arguments asks
        // for help, and any argument is unknown.
        Ok(Cli {}) => ExitCode::SUCCESS,
        Err(err) => report(&err),
    }
}

/// Help and version go out as clap renders them. A usage error is cut to
/// its first line: clap's own rendering adds a usage block and tips over
/// several lines, and every Kiln error is one line, so CI logs and
/// wrappers can rely on that from the first command on.
fn report(err: &clap::Error) -> ExitCode {
    match err.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            // Nothing useful to do if stdout is gone.
            let _ = err.print();
            ExitCode::SUCCESS
        }
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            let _ = err.print();
            ExitCode::from(EXIT_USAGE)
        }
        _ => {
            eprintln!("kiln: {} (see `kiln --help`)", first_line(err));
            ExitCode::from(EXIT_USAGE)
        }
    }
}

/// The first line of clap's plain-text rendering, without its `error: `
/// prefix (Kiln names itself instead).
fn first_line(err: &clap::Error) -> String {
    let rendered = err.render().to_string();
    let line = rendered.lines().next().unwrap_or_default();
    line.strip_prefix("error: ").unwrap_or(line).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn command_definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn usage_error_is_one_line_without_clap_prefix() {
        let err = Cli::try_parse_from(["kiln", "frobnicate"])
            .err()
            .expect("an unknown subcommand is a usage error");
        let line = first_line(&err);
        assert!(!line.contains('\n'), "{line:?}");
        assert!(!line.starts_with("error:"), "{line:?}");
        assert!(line.contains("frobnicate"), "{line:?}");
    }
}
