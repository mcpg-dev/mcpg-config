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

/// The `AppConfig` schema every subcommand renders from.
///
/// Draft-07 keeps `definitions` and the shape the doc and explain renderers
/// read (schemars' default moved to 2020-12). Descriptions come from `///`
/// comments with their source line breaks; a summary is one paragraph, so
/// the breaks inside a paragraph become spaces and only blank lines remain.
pub fn app_config_schema() -> Result<serde_json::Value, serde_json::Error> {
    let schema = schemars::generate::SchemaSettings::draft07()
        .into_generator()
        .into_root_schema_for::<mcpg::config::AppConfig>();
    let mut value = serde_json::to_value(&schema)?;
    join_description_lines(&mut value);
    Ok(value)
}

/// The string literal a schema node names: `const: "x"`, or the first entry
/// of its `enum`. A variant carrying a whole `enum` list is named by its
/// first value, which is what the reference has always shown for it.
pub fn first_literal(node: &serde_json::Value) -> Option<&str> {
    node.get("const").and_then(|v| v.as_str()).or_else(|| {
        node.get("enum")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
    })
}

fn join_description_lines(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, v) in map.iter_mut() {
                if key == "description"
                    && let serde_json::Value::String(text) = v
                {
                    *text = text
                        .split("\n\n")
                        .map(|paragraph| {
                            paragraph
                                .lines()
                                .map(str::trim)
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n");
                } else {
                    join_description_lines(v);
                }
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(join_description_lines),
        _ => {}
    }
}

pub fn run(_args: Vec<String>) -> ExitCode {
    let value = match app_config_schema() {
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
