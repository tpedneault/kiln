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
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};

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

    /// A JUnit XML test report (repeatable): a `verifies` code reference then counts only when its test passed.
    #[arg(long = "results", global = true, value_name = "FILE")]
    results: Vec<PathBuf>,

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

/// `kiln affected`'s outputs.
#[derive(Clone, Copy, ValueEnum)]
pub enum AffectedFormat {
    Text,
    Json,
    /// GitHub workflow annotations: a notice on each changed reference.
    Github,
}

#[derive(Subcommand)]
enum Command {
    /// Start a project: kiln.toml, design/ with body templates, the agent guide and a design-doc export.
    Init {
        /// The project's name in reports (default: the folder's name).
        #[arg(long)]
        name: Option<String>,
        /// Also register `kiln lsp` for Markdown in .fenix/tools.json (merged, never replaced).
        #[arg(long)]
        fenix: bool,
        /// The preset to add (`kiln preset list`): design-doc (the default), arc42, srs, or none.
        #[arg(long, value_name = "NAME")]
        preset: Option<String>,
    },
    /// Write the agent guide (design/AGENTS.md), or bring Kiln's section of it up to date with kiln.toml.
    Agents,
    /// Document templates Kiln ships (arc42, srs, design-doc): list them, or add one.
    Preset {
        #[command(subcommand)]
        what: PresetCommand,
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
        /// A link to write, ROLE=ID (`--link part-of=MS-002`); repeatable.
        #[arg(long = "link", value_name = "ROLE=ID")]
        links: Vec<String>,
        /// A tag; repeatable.
        #[arg(long = "tag", value_name = "TAG")]
        tags: Vec<String>,
        /// The body, from a file or `-` for stdin, in place of the type's template.
        #[arg(long, value_name = "PATH")]
        body_file: Option<PathBuf>,
        /// Open the new file in Fenix at its title.
        #[arg(long)]
        open: bool,
        /// json prints {"id", "path"}.
        #[arg(long, value_enum, default_value_t = TextOrJson::Text)]
        format: TextOrJson,
    },
    /// List items, filtered.
    List {
        /// Only these types, comma-separated (`requirement,design`).
        #[arg(long = "type", value_name = "TYPE")]
        type_name: Option<String>,
        /// Only these statuses, comma-separated (`todo,doing`); one no type has is an error.
        #[arg(long, value_name = "STATUS")]
        status: Option<String>,
        /// Only items with any of these tags, comma-separated.
        #[arg(long, value_name = "TAG")]
        tag: Option<String>,
        /// Each linked Jira ticket's status and assignee (read-only, cached 10 minutes).
        #[arg(long)]
        jira: bool,
        /// With --format json, each item's body too: the whole plan in one call.
        #[arg(long)]
        body: bool,
        #[arg(long, value_enum, default_value_t = TextOrJson::Text)]
        format: TextOrJson,
    },
    /// Show one item: its links in both directions, coverage, progress and body.
    Show {
        id: String,
        #[arg(long, value_enum, default_value_t = TextOrJson::Text)]
        format: TextOrJson,
    },
    /// Work ready to start: open, not blocked, nothing it depends on open; earliest milestone first.
    Next {
        /// Only this milestone's work.
        #[arg(long, value_name = "ID")]
        milestone: Option<String>,
        /// Leave out work someone has claimed (an `owner` is set).
        #[arg(long)]
        unowned: bool,
        #[arg(long, value_enum, default_value_t = TextOrJson::Text)]
        format: TextOrJson,
    },
    /// The plan by milestone: progress, ready and waiting work, and kiln check's findings.
    Status {
        #[arg(long, value_enum, default_value_t = TextOrJson::Text)]
        format: TextOrJson,
    },
    /// Check the project against the rules; exit 1 on any error.
    Check {
        #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
        format: CheckFormat,
    },
    /// Change front-matter values: status=, title=, parent=, owner=, due=, tags=a,b, jira=KEY (empty removes).
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
        /// Follow the requirement levels down from the top: each chain, the derived and the untraced.
        #[arg(long, conflicts_with = "type_name")]
        levels: bool,
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
    /// Jira: push the tasks Kiln manages as sub-tasks of the project issue.
    Jira {
        #[command(subcommand)]
        what: JiraCommand,
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
    /// The items a change reaches: requirements whose code or file changed since REV (default HEAD), and what verifies them.
    Affected {
        /// The revision to compare the working tree with (`origin/main` in a pull request).
        #[arg(default_value = "HEAD")]
        rev: String,
        #[arg(long, value_enum, default_value_t = AffectedFormat::Text)]
        format: AffectedFormat,
    },
    /// An item's history from git: each commit that changed it, with date, author and what changed.
    Log {
        id: String,
        #[arg(long, value_enum, default_value_t = TextOrJson::Text)]
        format: TextOrJson,
    },
    /// Sign-off status changes since REV (default HEAD), with the commit and author of each: K218 findings.
    Signoff {
        /// The revision to compare the working tree with (`origin/main` in a pull request).
        #[arg(default_value = "HEAD")]
        rev: String,
        #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
        format: CheckFormat,
    },
    /// Everything that reaches an item by links or parents, transitively.
    Impact {
        id: String,
        #[arg(long, value_enum, default_value_t = TextOrJson::Text)]
        format: TextOrJson,
    },
}

#[derive(Subcommand)]
enum PresetCommand {
    /// The presets and what each is for.
    List,
    /// Copy a preset's template into this project's templates/ folder; never overwrites.
    Add {
        name: String,
        /// Print what would change; write nothing.
        #[arg(long)]
        dry_run: bool,
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
enum JiraCommand {
    /// Create, update and move the project's tickets to match the items.
    Sync {
        /// Print the plan; change nothing.
        #[arg(long)]
        dry_run: bool,
        /// Sync without asking (needed when not interactive).
        #[arg(long)]
        yes: bool,
    },
    /// Take the status of items' tickets into the items (all synced items without IDs).
    Pull { ids: Vec<String> },
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

/// `kiln --help` lists the commands in these groups, in this order, so a
/// newcomer sees the eight everyday ones before the plan, review, editors
/// and integrations (REQ-021). Each line's text is the command's own
/// `about`, so the groups can't say something the command doesn't; a test
/// keeps every command in exactly one group.
const COMMAND_GROUPS: &[(&str, &[&str])] = &[
    (
        "Everyday",
        &[
            "init", "new", "list", "show", "set", "link", "check", "export",
        ],
    ),
    (
        "The plan and its coverage",
        &["next", "status", "trace", "impact"],
    ),
    (
        "Changing the design",
        &["unlink", "renumber", "preset", "agents"],
    ),
    (
        "Reviewing",
        &["diff", "affected", "signoff", "log", "confluence"],
    ),
    ("The window and editors", &["gui", "lsp", "provider"]),
    ("Integrations", &["jira", "auth"]),
];

/// The root command with its help grouped by [`COMMAND_GROUPS`].
fn command() -> clap::Command {
    let root = Cli::command();
    let width = COMMAND_GROUPS
        .iter()
        .flat_map(|(_, names)| names.iter().map(|n| n.len()))
        .max()
        .unwrap_or(0);
    let mut groups = String::new();
    for (title, names) in COMMAND_GROUPS {
        groups.push_str(&format!("{title}:\n"));
        for name in *names {
            let about = root
                .find_subcommand(name)
                .and_then(|c| c.get_about())
                .map(|a| a.to_string())
                .unwrap_or_default();
            groups.push_str(&format!("  {name:width$}  {about}\n"));
        }
        groups.push('\n');
    }
    groups.push_str("`kiln help COMMAND` or `kiln COMMAND --help` for one command's options; `kiln` alone opens the window.\n");
    root.help_template(format!(
        "{{about-with-newline}}\n{{usage-heading}} {{usage}}\n\n{groups}\nOptions:\n{{options}}"
    ))
}

fn main() -> ExitCode {
    let cli = match command()
        .try_get_matches()
        .and_then(|m| Cli::from_arg_matches(&m))
    {
        Ok(cli) => cli,
        Err(err) => return report(&err),
    };
    project::set_results(cli.results);
    let project = cli.project.as_deref();
    let command = cli.command.unwrap_or(Command::Gui {
        since: None,
        item: None,
    });
    let result = match command {
        Command::Init {
            name,
            fenix,
            preset,
        } => cmd::init::run(project, name, fenix, preset.as_deref()),
        Command::Agents => cmd::agents::run(project),
        Command::Preset {
            what: PresetCommand::List,
        } => cmd::preset::list(),
        Command::Preset {
            what: PresetCommand::Add { name, dry_run },
        } => cmd::preset::add(project, &name, dry_run),
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
            links,
            tags,
            body_file,
            open,
            format,
        } => cmd::new::run(
            project,
            cmd::new::Args {
                type_name: &type_name,
                parent,
                title,
                links: &links,
                tags,
                body_file: body_file.as_deref(),
                open,
                format,
            },
        ),
        Command::List {
            type_name,
            status,
            tag,
            jira,
            body,
            format,
        } => cmd::list::run(
            project,
            cmd::list::Filters {
                type_name,
                status,
                tag,
            },
            jira,
            body,
            format,
        ),
        Command::Show { id, format } => cmd::show::run(project, &id, format),
        Command::Next {
            milestone,
            unowned,
            format,
        } => cmd::plan::next(project, milestone.as_deref(), unowned, format),
        Command::Status { format } => cmd::plan::status(project, format),
        Command::Check { format } => cmd::check::run(project, format),
        Command::Set { id, assignments } => cmd::edit::set(project, &id, &assignments),
        Command::Link { id, role, target } => cmd::edit::link(project, &id, &role, &target, true),
        Command::Unlink { id, role, target } => {
            cmd::edit::link(project, &id, &role, &target, false)
        }
        Command::Renumber { id, to, file } => {
            cmd::renumber::run(project, &id, &to, file.as_deref())
        }
        Command::Trace {
            levels: true,
            format,
            ..
        } => cmd::trace::run_levels(project, format),
        Command::Trace {
            type_name, format, ..
        } => cmd::trace::run(project, type_name, format),
        Command::Impact { id, format } => cmd::impact::run(project, &id, format),
        Command::Affected { rev, format } => cmd::affected::run(project, &rev, format),
        Command::Log { id, format } => cmd::log::run(project, &id, format),
        Command::Signoff { rev, format } => cmd::signoff::run(project, &rev, format),
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
        Command::Jira {
            what: JiraCommand::Sync { dry_run, yes },
        } => cmd::jira::sync(project, dry_run, yes),
        Command::Jira {
            what: JiraCommand::Pull { ids },
        } => cmd::jira::pull(project, &ids),
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

    #[test]
    // verifies: REQ-021
    fn every_command_is_in_one_help_group() {
        let root = Cli::command();
        let mut grouped: Vec<&str> = COMMAND_GROUPS
            .iter()
            .flat_map(|(_, names)| names.iter().copied())
            .collect();
        let mut commands: Vec<&str> = root
            .get_subcommands()
            .map(|c| c.get_name())
            .filter(|n| *n != "help")
            .collect();
        grouped.sort_unstable();
        commands.sort_unstable();
        let before = grouped.len();
        grouped.dedup();
        assert_eq!(grouped.len(), before, "a command is in two groups");
        assert_eq!(grouped, commands);
        let help = command().render_help().to_string();
        assert!(help.contains("Everyday:\n  init "), "{help}");
        assert!(help.contains("--project <DIR>"), "{help}");
    }
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
