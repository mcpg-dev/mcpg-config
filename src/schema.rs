//! `mcpg config schema` — emits a JSON Schema describing the full
//! `AppConfig` shape.
//!
//! Operators commit `config.schema.json` next to their `config.yaml`
//! and add the YAML schema reference comment so VS Code / IntelliJ /
//! vim with `yaml-language-server` provide autocomplete + per-field
//! hover docs.
//!
//! ```bash
//! $ mcpg config schema > config.schema.json
//! ```
//!
//! ```yaml
//! # yaml-language-server: $schema=./config.schema.json
//! server:
//!   bind_address: "127.0.0.1:8787"
//!   # ↑ IDE autocompletes here, with `///` doc-comment as hover.
//! ```
//!
//! The schema is derived from `schemars::JsonSchema` impls on every
//! gateway config type (under `apps/gateway/src/config/`). Output is
//! deterministic — same source tree → byte-identical schema — so CI
//! can `diff` against a committed `config.schema.json`.
//!
//! Determinism note: object-key order in serde_json depends on the
//! `preserve_order` feature, which is subject to Cargo feature
//! unification — a plain `cargo build` and a `cargo test`/`nextest`
//! build of the workspace can resolve it differently, flipping the
//! emitted key order between insertion-order (IndexMap) and sorted
//! (BTreeMap). To stay byte-identical regardless, we recursively sort
//! every object's keys before serializing. Operators + CI then always
//! get the same bytes.

use std::process::ExitCode;

/// Recursively sort all object keys so output is independent of the
/// serde_json `preserve_order` feature (see module docs).
fn sort_keys(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(String, serde_json::Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut sorted = serde_json::Map::new();
            for (k, v) in entries {
                sorted.insert(k, sort_keys(v));
            }
            serde_json::Value::Object(sorted)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sort_keys).collect())
        }
        other => other,
    }
}

pub fn run(_args: Vec<String>) -> ExitCode {
    let schema = schemars::schema_for!(mcpg::config::AppConfig);
    let value = match serde_json::to_value(&schema) {
        Ok(v) => sort_keys(v),
        Err(e) => {
            eprintln!("error: failed to convert schema to value: {e}");
            return ExitCode::from(1);
        }
    };
    match serde_json::to_string_pretty(&value) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: failed to serialize schema: {e}");
            ExitCode::from(1)
        }
    }
}
