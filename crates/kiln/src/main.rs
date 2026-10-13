//! The `kiln` binary: the command line (`clap`) over the library crates.
//! It owns what only a process has: arguments, stdout and stderr, exit
//! codes, finding the project by walking up to `kiln.toml`, reading and
//! writing files. Everything else is a library call, so commands stay
//! thin and their logic stays testable without spawning a process.
//!
//! Conventions every command keeps (DESIGN.md, "Command line"): results
//! go to stdout, messages to stderr; errors are one line on stderr naming
//! what failed and why, with file and line when there is one; exit codes
//! are 0 fine, 1 findings or a refused change, 2 usage or configuration
//! error, 3 network or server error. The commands are documented in
//! docs/CLI.md. With no arguments it prints help and exits 2, until K7
//! gives that invocation the GUI.

mod cmd;
mod project;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::error::ErrorKind;
use clap::{Parser, Subcommand, ValueEnum};

/// Findings, or a change refused.
pub const EXIT_FINDINGS: u8 = 1;
/// Usage or configuration error.
pub const EXIT_USAGE: u8 = 2;
/// Network or server error.
pub const EXIT_NETWORK: u8 = 3;

/// Why a command stopped: an exit code and one line for stderr.
#[derive(Debug)]
pub struct Fail {
    pub code: u8,
    pub message: String,
}

impl Fail {
    pub fn new(code: u8, message: impl Into<String>) -> Self {
        Fail {
            code,
            message: message.into(),
        }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Fail::new(EXIT_USAGE, message)
    }
}

#[derive(Parser)]
#[command(
    name = "kiln",
    version,
    about = "Requirements, design decisions and project progress, kept as Markdown in the repository they describe"
)]
struct Cli {
    /// Use the project at or above this folder instead of the current one.
    #[arg(long, global = true, value_name = "DIR")]
    project: Option<PathBuf>,

    /// What to do; none opens the GUI.
    #[command(subcommand)]
    command: Option<Command>,
}

/// Plain text for people, JSON for tools.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TextOrJson {
    Text,
    Json,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CheckFormat {
    Text,
    Json,
    /// GitLab Code Quality report.
    Gitlab,
    /// GitHub Actions workflow commands.
    Github,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TraceFormat {
    Text,
    Json,
    Csv,
}

#[derive(Subcommand)]
enum Command {
    /// Write a kiln.toml with the default types and a design/ folder; never overwrites.
    Init {
        /// The project's name in reports (default: the folder's name).
        #[arg(long)]
        name: Option<String>,
        /// Also register `kiln lsp` for Markdown in .fenix/tools.json (merged, never replaced).
        #[arg(long)]
        fenix: bool,
    },
    /// Create an item with the next free ID and print its path.
    New {
        /// The item's type (requirement, decision, …).
        #[arg(value_name = "TYPE")]
        type_name: String,
        #[arg(long, value_name = "ID")]
        parent: Option<String>,
        #[arg(long, value_name = "TEXT")]
        title: Option<String>,
        /// Open the new file in Fenix at its title.
        #[arg(long)]
        open: bool,
    },
    /// List items, filtered.
    List {
        #[arg(long = "type", value_name = "TYPE")]
        type_name: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        tag: Option<String>,
        /// Each linked Jira ticket's status and assignee (read-only, cached 10 minutes).
        #[arg(long)]
        jira: bool,
        #[arg(long, value_enum, default_value_t = TextOrJson::Text)]
        format: TextOrJson,
    },
    /// Show one item with its links in both directions, coverage and progress.
    Show {
        id: String,
        #[arg(long, value_enum, default_value_t = TextOrJson::Text)]
        format: TextOrJson,
    },
    /// Check the project against the rules; exit 1 on any error.
    Check {
        #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
        format: CheckFormat,
    },
    /// Change front-matter values: status=, title=, parent=, owner=, due= (empty removes).
    Set {
        id: String,
        #[arg(required = true, value_name = "KEY=VALUE")]
        assignments: Vec<String>,
    },
    /// Add a link: `kiln link TEST-004 verifies REQ-014`.
    Link {
        id: String,
        role: String,
        target: String,
    },
    /// Remove a link.
    Unlink {
        id: String,
        role: String,
        target: String,
    },
    /// Move an item to another ID and rewrite every reference to it.
    Renumber {
        id: String,
        #[arg(long, value_name = "ID")]
        to: String,
        /// Which file, when the ID is duplicated.
        #[arg(long, value_name = "PATH")]
        file: Option<String>,
    },
    /// Coverage: what implements and verifies each item, and the gaps.
    Trace {
        #[arg(long = "type", value_name = "TYPE")]
        type_name: Option<String>,
        #[arg(long, value_enum, default_value_t = TraceFormat::Text)]
        format: TraceFormat,
    },
    /// Render export targets: write file targets that changed, publish Confluence targets.
    Export {
        /// Targets to export (default: all).
        names: Vec<String>,
        /// Show what would change; write nothing.
        #[arg(long, conflicts_with = "check")]
        dry_run: bool,
        /// Like --dry-run, and exit 1 if anything would change.
        #[arg(long)]
        check: bool,
        /// Refuse when the working tree has uncommitted changes.
        #[arg(long)]
        require_clean: bool,
        /// Export from a shallow clone, linking older files at the exported commit.
        #[arg(long)]
        allow_shallow: bool,
        /// Plan Confluence targets against a recorded snapshot instead of the live space.
        #[arg(long, value_name = "FILE")]
        snapshot: Option<String>,
        /// Write the live snapshot a Confluence dry run read to FILE.
        #[arg(long, value_name = "FILE")]
        save_snapshot: Option<String>,
        /// Publish Confluence targets without asking (needed when not interactive).
        #[arg(long)]
        yes: bool,
        /// Overwrite pages edited in Confluence.
        #[arg(long)]
        force: bool,
        /// Publish into a `page =` that already has content, replacing it.
        #[arg(long)]
        adopt: bool,
        /// Send pages retired more than DAYS days ago to the trash.
        #[arg(long, value_name = "DAYS")]
        prune_removed: Option<u32>,
        /// Mark what changed since REV (a tag, branch or commit) and fill the changes report.
        #[arg(long, value_name = "REV")]
        since: Option<String>,
        #[arg(long, value_enum, default_value_t = TextOrJson::Text)]
        format: TextOrJson,
    },
    /// The window: Tree, Trace, Impact, Board, Roadmap, Decisions, Changes (also `kiln` alone).
    Gui {
        /// The baseline for the Changes page (default: the most recent tag).
        #[arg(long, value_name = "REV")]
        since: Option<String>,
        /// Put the cursor on this item.
        #[arg(long, value_name = "ID")]
        item: Option<String>,
    },
    /// Language server on stdin and stdout, for Fenix or any LSP editor.
    Lsp,
    /// Flint provider on stdin and stdout: items, drill-downs, actions.
    Provider {
        /// The provider's name in Flint's settings.toml `[[providers]]`.
        #[arg(long, default_value = "kiln")]
        name: String,
        /// The scope keyword to ask Flint for.
        #[arg(long, default_value = "k")]
        prefix: String,
    },
    /// Confluence: read what readers left on the published pages.
    Confluence {
        #[command(subcommand)]
        what: ConfluenceCommand,
    },
    /// Store a token in the OS credential store, prompting without echo.
    Auth {
        #[command(subcommand)]
        service: AuthService,
    },
    /// Items added, removed and changed between two revisions (or a revision and the working tree).
    Diff {
        /// `REV` (to the working tree) or `REV..REV`: a tag, branch or commit.
        range: String,
        #[arg(long = "type", value_name = "TYPE")]
        type_name: Option<String>,
        #[arg(long, value_enum, default_value_t = TextOrJson::Text)]
        format: TextOrJson,
    },
    /// Everything that reaches an item by links or parents, transitively.
    Impact {
        id: String,
        #[arg(long, value_enum, default_value_t = TextOrJson::Text)]
        format: TextOrJson,
    },
}

#[derive(Subcommand)]
enum ConfluenceCommand {
    /// Unresolved footer comments on each target's pages, per item.
    Comments {
        /// Confluence targets (default: all).
        names: Vec<String>,
        #[arg(long, value_enum, default_value_t = TextOrJson::Text)]
        format: TextOrJson,
    },
}

#[derive(Subcommand)]
enum AuthService {
    /// The token for `[confluence] url`, checked against the server first.
    Confluence {
        /// Remove the stored token instead.
        #[arg(long)]
        forget: bool,
    },
    /// The personal access token for `[jira] url`, checked with one search first.
    Jira {
        /// Remove the stored token instead.
        #[arg(long)]
        forget: bool,
    },
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => return report(&err),
    };
    let project = cli.project.as_deref();
    let command = cli.command.unwrap_or(Command::Gui {
        since: None,
        item: None,
    });
    let result = match command {
        Command::Init { name, fenix } => cmd::init::run(project, name, fenix),
        Command::Provider { name, prefix } => kiln_flint::serve_stdio(kiln_flint::Options {
            name,
            prefix,
            pinned: project.map(PathBuf::from),
            start: std::env::current_dir().ok(),
            state: project::folders().map(|f| f.state),
        })
        .map(|()| 0)
        .map_err(|e| Fail::new(EXIT_USAGE, format!("provider: {e}"))),
        Command::Gui { since, item } => cmd::gui::run(project, since, item),
        Command::Lsp => kiln_lsp::serve_stdio(project::today)
            .map(|()| 0)
            .map_err(|e| Fail::new(EXIT_USAGE, format!("lsp: {e}"))),
        Command::New {
            type_name,
            parent,
            title,
            open,
        } => cmd::new::run(project, &type_name, parent, title, open),
        Command::List {
            type_name,
            status,
            tag,
            jira,
            format,
        } => cmd::list::run(project, type_name, status, tag, jira, format),
        Command::Show { id, format } => cmd::show::run(project, &id, format),
        Command::Check { format } => cmd::check::run(project, format),
        Command::Set { id, assignments } => cmd::edit::set(project, &id, &assignments),
        Command::Link { id, role, target } => cmd::edit::link(project, &id, &role, &target, true),
        Command::Unlink { id, role, target } => {
            cmd::edit::link(project, &id, &role, &target, false)
        }
        Command::Renumber { id, to, file } => {
            cmd::renumber::run(project, &id, &to, file.as_deref())
        }
        Command::Trace { type_name, format } => cmd::trace::run(project, type_name, format),
        Command::Impact { id, format } => cmd::impact::run(project, &id, format),
        Command::Diff {
            range,
            type_name,
            format,
        } => cmd::diff::run(project, &range, type_name, format),
        Command::Export {
            names,
            dry_run,
            check,
            require_clean,
            allow_shallow,
            snapshot,
            save_snapshot,
            yes,
            force,
            adopt,
            prune_removed,
            since,
            format,
        } => cmd::export::run(
            project,
            cmd::export::Options {
                names,
                dry_run,
                check,
                require_clean,
                allow_shallow,
                snapshot,
                save_snapshot,
                yes,
                force,
                adopt,
                prune_removed,
                since,
                format,
            },
        ),
        Command::Confluence {
            what: ConfluenceCommand::Comments { names, format },
        } => cmd::confluence::comments(project, names, format),
        Command::Auth {
            service: AuthService::Confluence { forget },
        } => cmd::auth::confluence(project, forget),
        Command::Auth {
            service: AuthService::Jira { forget },
        } => cmd::jira::auth(project, forget),
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(fail) => {
            eprintln!("kiln: {}", fail.message);
            ExitCode::from(fail.code)
        }
    }
}

/// Help and version go out as clap renders them. A usage error is cut to
/// its first line: clap's own rendering adds a usage block and tips over
/// several lines, and every Kiln error is one line, so CI logs and
/// wrappers can rely on that.
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
