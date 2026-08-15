//! `mcpg-config` — the gateway configuration tool.
//!
//! One binary, seven subcommands:
//! scaffold (`init`), validate (`check`), document (`doc`), inspect
//! (`explain`/`schema`), and audit (`secrets`/`wiring`) the gateway's
//! `AppConfig`.
//!
//! Reached as `mcpg config <sub>` through the gateway's front-door
//! dispatch, or invoked directly as `mcpg-config <sub>`.
//!
//! clap owns the top level (subcommand routing, `--help`, `--version`,
//! exit 2 on usage errors); each subcommand keeps its own argument
//! contract — including the positional `<config.yaml> [override.yaml …]`
//! later-wins merge semantics of `check`/`secrets`/`wiring`, mirroring
//! `MCPG_CONFIG` — and renders its own usage for `-h`/`--help` after the
//! subcommand word.

use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod check;
mod doc;
mod explain;
mod init;
mod schema;
mod secrets;
mod wiring;

#[derive(Parser, Debug)]
#[command(
    name = "mcpg-config",
    version,
    about = "Author, validate, and inspect MCPG gateway configuration",
    long_about = "Author, validate, and inspect the MCPG gateway configuration \
                  (AppConfig): scaffold a starter config, validate before boot, \
                  render the reference docs, explain fields, emit the JSON Schema, \
                  and audit secret references + binding wiring."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

// Each variant hands its raw arguments to the subcommand module: the
// modules own their argument contracts (and their `-h`/`--help`
// rendering), so help interception is disabled at this level and hyphen
// values flow through.
#[derive(Subcommand, Debug)]
enum Command {
    /// Scaffold a validated starter config from a template
    #[command(disable_help_flag = true)]
    Init {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Validate a config (deny_unknown_fields) before booting
    #[command(disable_help_flag = true)]
    Check {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Render the configuration reference from the live schema
    #[command(disable_help_flag = true)]
    Doc {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Explain a config field / resolve env + cred references
    #[command(disable_help_flag = true)]
    Explain {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Emit the JSON Schema for the AppConfig
    #[command(disable_help_flag = true)]
    Schema {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Inspect cred:// + ${env} references
    #[command(disable_help_flag = true)]
    Secrets {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Show how bindings wire to backends + plugins
    #[command(disable_help_flag = true)]
    Wiring {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Init { args } => init::run(args),
        Command::Check { args } => check::run(args),
        Command::Doc { args } => doc::run(args),
        Command::Explain { args } => explain::run(args),
        Command::Schema { args } => schema::run(args),
        Command::Secrets { args } => secrets::run(args),
        Command::Wiring { args } => wiring::run(args),
    }
}
