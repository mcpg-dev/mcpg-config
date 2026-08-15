//! `mcpg config secrets` — operator-facing secret-reference scanner.
//!
//! Loads one or more YAML config files (same merge semantics as
//! `mcpg config check` / the runtime), runs the T4-12 secret
//! scanner, and prints the table of every `${env.VAR}` and
//! `<scheme>://...` reference the gateway will resolve at runtime.
//! Useful for compliance reviews and blast-radius analysis ("what
//! gateways consume `GITHUB_TOKEN` and which bindings reference
//! it?").
//!
//! ```text
//! $ mcpg config secrets prod.yaml
//! ENV vars referenced (3):
//!   GH_TOKEN          — bindings[github.user.repos.list].headers.Authorization
//!   REDIS_URL         — cluster.url
//!   ANALYTICS_SECRET  — oauth.providers.analytics.client_secret
//!
//! cred:// refs (2):
//!   vault://db/orders          — bindings[orders.list].url
//!   vault://kms/audit-key      — audit.sinks[0].config.encryption_key
//! ```
//!
//! Exit codes:
//!   0 — scan succeeded (regardless of whether any refs were found)
//!   2 — usage / I/O error
//!
//! Does NOT actually resolve any secret — it only surfaces what
//! the gateway will read at boot. Resolution failures (missing
//! env var, vault timeout) surface at gateway boot, not here.

use std::path::PathBuf;
use std::process::ExitCode;

use mcpg::config::secret_scan::{SecretRef, SecretRefKind, scan_value};

const USAGE: &str = "\
mcpg config secrets — list every secret reference a config will resolve at boot

USAGE:
    mcpg config secrets <config.yaml> [<override.yaml> ...]
    mcpg config secrets --json <config.yaml> [...]

OPTIONS:
    --json    Emit machine-readable JSON instead of the pretty table.
    -h, --help  Print this message.

NOTES:
    Multiple files merge in argument order with later-wins semantics
    (matches `MCPG_CONFIG=a.yaml:b.yaml` at runtime). Does NOT apply
    `MCPG_*` env-var overrides — the scanner inspects the files as
    written.

EXIT CODES:
    0 — scan succeeded
    2 — usage or I/O error
";

pub fn run(args: Vec<String>) -> ExitCode {
    if args.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let mut json_mode = false;
    let mut paths: Vec<PathBuf> = Vec::new();
    for arg in &args {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--json" => json_mode = true,
            other if other.starts_with('-') => {
                eprintln!("error: unknown option: {other}\n\n{USAGE}");
                return ExitCode::from(2);
            }
            other => paths.push(PathBuf::from(other)),
        }
    }
    if paths.is_empty() {
        eprintln!("error: no config files specified\n\n{USAGE}");
        return ExitCode::from(2);
    }

    let mut yamls: Vec<String> = Vec::with_capacity(paths.len());
    for path in &paths {
        if !path.exists() {
            eprintln!("error: config file not found: {}", path.display());
            return ExitCode::from(2);
        }
        match std::fs::read_to_string(path) {
            Ok(s) => yamls.push(s),
            Err(e) => {
                eprintln!("error: failed to read {}: {}", path.display(), e);
                return ExitCode::from(2);
            }
        }
    }

    // Merge YAMLs into a single serde_json::Value so the scanner can
    // walk the operator-as-written shape, even if the merged result
    // wouldn't typecheck against AppConfig (the scanner is purely
    // string-based, so partial / lenient configs still surface refs).
    let mut merged = serde_json::Value::Object(serde_json::Map::new());
    for (path, yaml) in paths.iter().zip(yamls.iter()) {
        let parsed: serde_json::Value = match serde_yaml::from_str(yaml) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("error: failed to parse YAML in {}: {}", path.display(), e);
                return ExitCode::from(2);
            }
        };
        deep_merge(&mut merged, parsed);
    }

    let mut refs = Vec::new();
    scan_value(&merged, "", &mut refs);
    refs.sort();
    refs.dedup();

    if json_mode {
        match serde_json::to_string_pretty(&refs) {
            Ok(s) => {
                println!("{s}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: failed to serialise refs: {e}");
                ExitCode::from(2)
            }
        }
    } else {
        print_pretty_table(&refs);
        ExitCode::SUCCESS
    }
}

/// Pretty-print: env-vars first, then secret-URIs. Within each
/// group the entries are name-sorted so a rotation review groups
/// every reference of the same secret together.
fn print_pretty_table(refs: &[SecretRef]) {
    let env_refs: Vec<&SecretRef> = refs
        .iter()
        .filter(|r| r.kind == SecretRefKind::EnvVar)
        .collect();
    let uri_refs: Vec<&SecretRef> = refs
        .iter()
        .filter(|r| r.kind == SecretRefKind::SecretUri)
        .collect();

    if env_refs.is_empty() && uri_refs.is_empty() {
        println!("(no secret references found)");
        return;
    }

    let env_name_width = env_refs.iter().map(|r| r.name.len()).max().unwrap_or(0);
    let uri_name_width = uri_refs.iter().map(|r| r.name.len()).max().unwrap_or(0);

    if !env_refs.is_empty() {
        println!("ENV vars referenced ({}):", env_refs.len());
        for r in env_refs {
            println!(
                "  {:<width$}  \u{2014} {}",
                r.name,
                r.field_path,
                width = env_name_width,
            );
        }
    }

    if !uri_refs.is_empty() {
        if env_name_width > 0 {
            println!();
        }
        println!("Secret URI refs ({}):", uri_refs.len());
        for r in uri_refs {
            println!(
                "  {:<width$}  \u{2014} {}",
                r.name,
                r.field_path,
                width = uri_name_width,
            );
        }
    }
}

/// Recursive deep-merge: objects merge field-wise, arrays + scalars
/// replace wholesale. Mirrors figment's later-wins semantics so the
/// scanner sees the same shape the runtime does.
fn deep_merge(dst: &mut serde_json::Value, src: serde_json::Value) {
    match (dst, src) {
        (serde_json::Value::Object(d), serde_json::Value::Object(s)) => {
            for (k, v) in s {
                match d.get_mut(&k) {
                    Some(existing) => deep_merge(existing, v),
                    None => {
                        d.insert(k, v);
                    }
                }
            }
        }
        (slot, src) => *slot = src,
    }
}
