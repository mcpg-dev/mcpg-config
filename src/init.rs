//! `mcpg config init` — pick a deployment template and write a starter
//! `config.yaml`.
//!
//! Six templates are baked into the binary at compile time (the same
//! ones that ship under `examples/deployments/` and pass
//! `mcpg config check` in CI). The wizard either prompts interactively
//! for a topology choice or accepts `--template <name>` for scripted
//! use:
//!
//! ```text
//! $ mcpg config init
//! Pick a deployment topology:
//!   1) dev-single-node          — laptop dev, mock binding, stderr logs
//!   2) production-single-redis  — single-instance prod, Redis cluster, OIDC
//!   3) production-redis-cluster — multi-replica via Redis, S3 audit
//!   4) production-nats-cluster  — NATS JetStream variant
//!   5) air-gapped               — zero-outbound, static JWKS, file storage
//!   6) multi-tenant             — per-tenant quota + rate limit + group gate
//!
//! Choice [1]: 2
//! Output path [./config.yaml]:
//! ✓ Wrote config.yaml (production-single-redis).
//!   Validate:  mcpg config check config.yaml
//!   Boot:      MCPG_CONFIG=config.yaml mcpg
//!
//! $ mcpg config init --template multi-tenant --output ./tenant-prod.yaml
//! ```

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
mcpg config init — write a starter MCPG config from a deployment template

USAGE:
    mcpg config init                                # interactive wizard
    mcpg config init --list                         # show template names
    mcpg config init --template <name>              # non-interactive, ./config.yaml
    mcpg config init --template <name> --output <p> # non-interactive, custom path
    mcpg config init --template <name> --force      # overwrite existing file

EXIT CODES:
    0 — config written
    1 — refused to overwrite (pass --force) or unknown --template
    2 — usage error / I/O error
";

struct Template {
    name: &'static str,
    summary: &'static str,
    yaml: &'static str,
}

const TEMPLATES: &[Template] = &[
    Template {
        name: "dev-single-node",
        summary: "laptop dev, mock binding, stderr logs",
        yaml: include_str!("../examples/deployments/dev-single-node.yaml"),
    },
    Template {
        name: "production-single-redis",
        summary: "single-instance prod, Redis cluster, OIDC",
        yaml: include_str!("../examples/deployments/production-single-redis.yaml"),
    },
    Template {
        name: "production-redis-cluster",
        summary: "multi-replica via Redis, S3 audit",
        yaml: include_str!("../examples/deployments/production-redis-cluster.yaml"),
    },
    Template {
        name: "production-nats-cluster",
        summary: "NATS JetStream variant",
        yaml: include_str!("../examples/deployments/production-nats-cluster.yaml"),
    },
    Template {
        name: "air-gapped",
        summary: "zero-outbound, static JWKS, file storage",
        yaml: include_str!("../examples/deployments/air-gapped.yaml"),
    },
    Template {
        name: "multi-tenant",
        summary: "per-tenant quota + rate limit + group-gated tools",
        yaml: include_str!("../examples/deployments/multi-tenant.yaml"),
    },
];

#[derive(Default)]
struct Args {
    list: bool,
    template: Option<String>,
    output: Option<PathBuf>,
    force: bool,
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut args = Args::default();
    let mut iter = raw.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--list" | "-l" => args.list = true,
            "--force" | "-f" => args.force = true,
            "--help" | "-h" => return Err(String::new()), // empty == print help, exit 0
            "--template" | "-t" => {
                args.template = Some(
                    iter.next()
                        .map(String::from)
                        .ok_or_else(|| "--template requires a value".to_owned())?,
                );
            }
            "--output" | "-o" => {
                args.output = Some(
                    iter.next()
                        .map(PathBuf::from)
                        .ok_or_else(|| "--output requires a value".to_owned())?,
                );
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(args)
}

pub fn run(raw: Vec<String>) -> ExitCode {
    let args = match parse_args(&raw) {
        Ok(a) => a,
        Err(msg) if msg.is_empty() => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(msg) => {
            eprintln!("error: {msg}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };

    if args.list {
        for t in TEMPLATES {
            println!("{:24} — {}", t.name, t.summary);
        }
        return ExitCode::SUCCESS;
    }

    let chosen = match args.template.as_deref() {
        Some(name) => match TEMPLATES.iter().find(|t| t.name == name) {
            Some(t) => t,
            None => {
                eprintln!("error: unknown template '{name}'");
                eprintln!();
                eprintln!("Available templates:");
                for t in TEMPLATES {
                    eprintln!("  {} — {}", t.name, t.summary);
                }
                return ExitCode::from(1);
            }
        },
        None => match prompt_for_template() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
        },
    };

    let output = match args.output {
        Some(p) => p,
        None if args.template.is_none() => match prompt_for_output() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
        },
        None => PathBuf::from("./config.yaml"),
    };

    if output.exists() && !args.force {
        eprintln!(
            "error: {} already exists. Pass --force to overwrite.",
            output.display(),
        );
        return ExitCode::from(1);
    }

    if let Err(e) = std::fs::write(&output, chosen.yaml) {
        eprintln!("error: failed to write {}: {}", output.display(), e);
        return ExitCode::from(2);
    }

    println!("\u{2713} Wrote {} ({}).", output.display(), chosen.name,);
    println!("  Validate:  mcpg config check {}", output.display());
    println!("  Boot:      MCPG_CONFIG={} mcpg", output.display());
    ExitCode::SUCCESS
}

fn prompt_for_template() -> io::Result<&'static Template> {
    println!("Pick a deployment topology:");
    for (i, t) in TEMPLATES.iter().enumerate() {
        println!("  {}) {:24} — {}", i + 1, t.name, t.summary);
    }
    println!();

    let stdin = io::stdin();
    let mut handle = stdin.lock();
    loop {
        print!("Choice [1]: ");
        io::stdout().flush()?;
        let mut line = String::new();
        if handle.read_line(&mut line)? == 0 {
            // EOF on stdin (piped + no input) — fall back to default.
            return Ok(&TEMPLATES[0]);
        }
        let trimmed = line.trim();
        let choice = if trimmed.is_empty() {
            1
        } else {
            match trimmed.parse::<usize>() {
                Ok(n) if n >= 1 && n <= TEMPLATES.len() => n,
                _ => {
                    eprintln!("Please enter a number between 1 and {}.", TEMPLATES.len());
                    continue;
                }
            }
        };
        return Ok(&TEMPLATES[choice - 1]);
    }
}

fn prompt_for_output() -> io::Result<PathBuf> {
    print!("Output path [./config.yaml]: ");
    io::stdout().flush()?;
    let stdin = io::stdin();
    let mut line = String::new();
    if stdin.lock().read_line(&mut line)? == 0 {
        return Ok(PathBuf::from("./config.yaml"));
    }
    let trimmed = line.trim();
    Ok(if trimmed.is_empty() {
        PathBuf::from("./config.yaml")
    } else {
        PathBuf::from(trimmed)
    })
}
