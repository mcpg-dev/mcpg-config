//! `mcpg config wiring` — print the wiring report for a config.
//!
//! Layout D'' P11. Operators using point-of-use wiring lose the
//! pre-D'' "single grep target for all wiring" property; this
//! helper restores it on demand. Walks the resolved config tree
//! and prints a tabular report of every consumer slot, what
//! `kind:` it resolved to, plus the loaded plugin list and any
//! plugin scheme claims (when scheme-routing is wired through —
//! see backlog).
//!
//! ```text
//! $ mcpg config wiring config.yaml
//!
//! Cluster
//!   kind:                     single_node
//!   provides:                 cache, kv, bus
//!
//! Consumer slots
//!   mcp.configurations.sessions.store              kind: cluster
//!   mcp.configurations.pipelines.store             kind: cluster
//!   mcp.configurations.delivery.bus                kind: cluster
//!   governance.quotas.store                        kind: cluster
//!   governance.audit.sinks[0]                      kind: file
//!   observability.logs.sinks[0]                    kind: stderr
//!
//! Loaded plugins (3)
//!   dev.mcpg.cache.redis           class: cache              enabled
//!   dev.mcpg.secret.vault          class: secret_provider    enabled
//!   dev.mcpg.identity.oidc         class: identity_provider  enabled
//!
//! Plugin scheme claims
//!   (none — scheme-routing wires through in P8 follow-up)
//!
//! Loaded but unwired (warnings)
//!   (none)
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use mcpg::config::{AppConfig, ResolvedKind, SlotClass, cluster_provides_for_kind, resolve_kind};

const USAGE: &str = "\
mcpg config wiring — print the wiring report for an MCPG config

USAGE:
    mcpg config wiring <config.yaml> [<override.yaml> ...]

NOTES:
    Multiple files merge in argument order with later-wins semantics
    (same as `MCPG_CONFIG=a.yaml:b.yaml` at runtime). Object fields
    deep-merge; arrays and scalars replace wholesale. The report
    reflects the post-merge / post-validation tree the gateway
    would actually run with.
";

pub fn run(args: Vec<String>) -> ExitCode {
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("{USAGE}");
        return if args.is_empty() {
            ExitCode::from(2)
        } else {
            ExitCode::SUCCESS
        };
    }

    let paths: Vec<PathBuf> = args.iter().map(PathBuf::from).collect();
    for p in &paths {
        if !p.exists() {
            eprintln!("error: config file not found: {}", p.display());
            return ExitCode::from(2);
        }
    }
    let path_refs: Vec<&std::path::Path> = paths.iter().map(PathBuf::as_path).collect();

    let config = match AppConfig::load_many(&path_refs) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: failed to load config: {e}");
            return ExitCode::from(1);
        }
    };

    print_report(&config);
    ExitCode::SUCCESS
}

fn print_report(config: &AppConfig) {
    println!();

    // Cluster section.
    let cluster_kind = config.cluster.kind.as_str();
    let provides = cluster_provides_for_kind(cluster_kind);
    println!("Cluster");
    println!("  kind:                     {cluster_kind}");
    println!(
        "  provides:                 {}",
        if provides.is_empty() {
            "(none)".to_owned()
        } else {
            provides.iter().copied().collect::<Vec<_>>().join(", ")
        }
    );
    println!();

    // Consumer slots — known slots that today carry a `kind:`
    // discriminator. As more slots route through `resolve_kind`
    // (P7.e backlog), expand the table here.
    println!("Consumer slots");
    let mut printed = 0;
    let configurations = &config.mcp.configurations;
    printed += print_capability_slot(
        "mcp.configurations.sessions.store",
        configurations
            .sessions
            .store
            .as_ref()
            .map(|s| s.kind.as_str()),
        SlotClass::Kv,
        cluster_kind,
        &config.plugins,
    );
    printed += print_capability_slot(
        "mcp.configurations.pipelines.store",
        configurations
            .pipelines
            .store
            .as_ref()
            .map(|s| s.kind.as_str()),
        SlotClass::Kv,
        cluster_kind,
        &config.plugins,
    );
    printed += print_capability_slot(
        "mcp.configurations.subscriptions.store",
        configurations
            .subscriptions
            .store
            .as_ref()
            .map(|s| s.kind.as_str()),
        SlotClass::Kv,
        cluster_kind,
        &config.plugins,
    );
    printed += print_capability_slot(
        "mcp.configurations.delivery.bus",
        configurations
            .delivery
            .bus
            .as_ref()
            .map(|b| b.kind.as_str()),
        SlotClass::Bus,
        cluster_kind,
        &config.plugins,
    );
    printed += print_capability_slot(
        "mcp.configurations.cancellation.bus",
        configurations
            .cancellation
            .bus
            .as_ref()
            .map(|b| b.kind.as_str()),
        SlotClass::Bus,
        cluster_kind,
        &config.plugins,
    );

    // governance.quotas.store (only print when quotas declared).
    if !config.governance.quotas.is_empty() {
        let kref = &config.governance.quotas.store;
        if !kref.kind.is_empty() {
            print_kind_ref_slot(
                "governance.quotas.store",
                &kref.kind,
                SlotClass::Kv,
                cluster_kind,
                &config.plugins,
            );
            printed += 1;
        }
    }

    // governance.audit.sinks[].
    for (i, sink) in config.governance.audit.sinks.iter().enumerate() {
        let path = format!("governance.audit.sinks[{i}]");
        print_simple_slot(&path, &sink.kind);
        printed += 1;
    }

    // observability.{logs,metrics,traces}.sinks[].
    for (i, sink) in config.observability.logs.sinks.iter().enumerate() {
        let path = format!("observability.logs.sinks[{i}]");
        print_simple_slot(&path, &sink.kind);
        printed += 1;
    }
    for (i, sink) in config.observability.metrics.sinks.iter().enumerate() {
        let path = format!("observability.metrics.sinks[{i}]");
        print_simple_slot(&path, &sink.kind);
        printed += 1;
    }
    for (i, sink) in config.observability.traces.sinks.iter().enumerate() {
        let path = format!("observability.traces.sinks[{i}]");
        print_simple_slot(&path, &sink.kind);
        printed += 1;
    }

    if printed == 0 {
        println!("  (no consumer slots declared)");
    }
    println!();

    // Loaded plugins.
    println!("Loaded plugins ({})", config.plugins.len());
    if config.plugins.is_empty() {
        println!("  (none)");
    } else {
        for entry in &config.plugins {
            let status = if entry.disabled {
                "DISABLED"
            } else {
                "enabled "
            };
            println!("  {:<32} class: {:<15} {status}", entry.id, entry.class);
        }
    }
    println!();

    // Scheme routing. The live `scheme → plugin_id` table is built at
    // gateway boot from each secret/config provider's runtime
    // `supported_schemes()` (auto-bind); this standalone validator does
    // not load plugins, so it can only list which entries WOULD claim a
    // scheme by class, not the concrete schemes themselves.
    println!("Plugin scheme claims");
    let mut scheme_printed = false;
    for entry in &config.plugins {
        if entry.disabled {
            continue;
        }
        if matches!(entry.class.as_str(), "secret" | "config") {
            println!(
                "  {:<32} (schemes resolved at gateway boot from supported_schemes())",
                entry.id
            );
            scheme_printed = true;
        }
    }
    if !scheme_printed {
        println!("  (none — no secret / config plugins loaded)");
    }
    println!();

    // Loaded but unwired warnings.
    println!("Loaded but unwired (warnings)");
    let referenced = referenced_plugin_ids(config);
    let mut warned = false;
    for entry in &config.plugins {
        if entry.disabled {
            continue;
        }
        if matches!(
            entry.class.as_str(),
            "http_route" | "identity_provider" | "tool_gate" | "transform" | "backend"
        ) {
            continue; // implicitly registered
        }
        if !referenced.contains(entry.id.as_str()) {
            println!(
                "  {:<32} class: {:<15} no consumer slot references its id",
                entry.id, entry.class
            );
            warned = true;
        }
    }
    if !warned {
        println!("  (none)");
    }
    println!();
}

/// Helper for `mcp.configurations.*.store/bus` slots that wrap a
/// `kind:` value inside a typed `StoreOverrideConfig` /
/// `BusOverrideConfig`. Returns 1 if printed, 0 if absent.
fn print_capability_slot(
    label: &str,
    kind: Option<&str>,
    slot: SlotClass,
    cluster_kind: &str,
    plugins: &[mcpg::config::PluginEntryConfig],
) -> usize {
    match kind {
        None => {
            println!("  {label:<46}  (default)");
            0
        }
        Some(kind) => {
            print_kind_ref_slot(label, kind, slot, cluster_kind, plugins);
            1
        }
    }
}

fn print_kind_ref_slot(
    label: &str,
    kind: &str,
    slot: SlotClass,
    cluster_kind: &str,
    plugins: &[mcpg::config::PluginEntryConfig],
) {
    let kref = mcpg::config::KindRef {
        kind: kind.to_owned(),
        config: serde_json::Value::Null,
    };
    let resolved = match resolve_kind(slot, &kref, plugins, cluster_kind) {
        Ok(ResolvedKind::Builtin(name)) => format!("builtin: {name}"),
        Ok(ResolvedKind::Plugin(id)) => format!("plugin: {id}"),
        Ok(ResolvedKind::Cluster) => format!("cluster ({cluster_kind})"),
        Err(e) => format!("UNRESOLVED: {e}"),
    };
    println!("  {label:<46}  kind: {kind:<20} → {resolved}");
}

fn print_simple_slot(label: &str, kind: &str) {
    println!("  {label:<46}  kind: {kind}");
}

/// Plugin ids referenced by any consumer slot's `kind:` value.
/// Used for the loaded-but-unwired warning.
fn referenced_plugin_ids(config: &AppConfig) -> std::collections::BTreeSet<&str> {
    let mut set = std::collections::BTreeSet::new();
    let configurations = &config.mcp.configurations;
    for k in [
        configurations
            .sessions
            .store
            .as_ref()
            .map(|s| s.kind.as_str()),
        configurations
            .pipelines
            .store
            .as_ref()
            .map(|s| s.kind.as_str()),
        configurations
            .subscriptions
            .store
            .as_ref()
            .map(|s| s.kind.as_str()),
        configurations
            .delivery
            .bus
            .as_ref()
            .map(|b| b.kind.as_str()),
        configurations
            .cancellation
            .bus
            .as_ref()
            .map(|b| b.kind.as_str()),
    ]
    .into_iter()
    .flatten()
    {
        if k.contains('.') {
            set.insert(k);
        }
    }
    if !config.governance.quotas.is_empty() {
        let k = config.governance.quotas.store.kind.as_str();
        if k.contains('.') {
            set.insert(k);
        }
    }
    for sink in &config.governance.audit.sinks {
        if sink.kind.contains('.') {
            set.insert(sink.kind.as_str());
        }
    }
    for sinks in [
        &config.observability.logs.sinks,
        &config.observability.metrics.sinks,
        &config.observability.traces.sinks,
    ] {
        for sink in sinks {
            if sink.kind.contains('.') {
                set.insert(sink.kind.as_str());
            }
        }
    }
    set
}
