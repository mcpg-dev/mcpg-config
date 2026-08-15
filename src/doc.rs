//! `mcpg config doc` — emits a Markdown configuration reference
//! generated from the same schemars JSON Schema that
//! `mcpg config schema` exposes.
//!
//! ```bash
//! $ mcpg config doc > apps/gateway/docs/configuration.md
//! ```
//!
//! Output is deterministic — same source tree → byte-identical
//! Markdown — so CI can diff against a committed copy and fail on
//! drift.

use std::process::ExitCode;

use serde_json::Value;

/// Audience-curated walk-through prepended to the alphabetical reference.
/// Three sections in operator-priority order: Quick start (the 20% that
/// solves 80% of cases), Production hardening (compliance + multi-replica),
/// Advanced (pipelines, suspending steps, per-plugin observability).
///
/// Each section links to the matching `examples/deployments/` template
/// and into the alphabetical reference further down the file. Edit this
/// constant + regenerate to update the curated layer; the reference
/// auto-tracks the live `AppConfig` schema so it never drifts.
const CURATED_INTRO: &str = include_str!("../docs/configuration-intro.md");

pub fn run(_args: Vec<String>) -> ExitCode {
    let schema = serde_json::to_value(schemars::schema_for!(mcpg::config::AppConfig))
        .expect("schema serializes");
    let mut out = String::new();
    render(&schema, &mut out);
    // Exactly one trailing newline: gen-config-reference.sh captures this
    // output via command substitution (which strips trailing newlines and
    // re-adds one), and the committed-doc test compares byte-for-byte —
    // the two only agree when the generator is already normalized.
    println!("{}", out.trim_end());
    ExitCode::SUCCESS
}

fn render(schema: &Value, out: &mut String) {
    out.push_str("# MCPG Configuration\n\n");
    out.push_str("> **Generated from `apps/gateway/src/config/` via `mcpg config doc`.**  \n");
    out.push_str(
        "> The audience-facing sections below are curated inside the generator; the per-block reference at the bottom is sourced from `///` rustdoc + `#[serde(...)]` annotations on the live `AppConfig` tree.  \n",
    );
    out.push_str("> Re-generate with: `mcpg config doc > apps/gateway/docs/configuration.md`.\n\n");

    if let Some(desc) = schema.get("description").and_then(|v| v.as_str()) {
        out.push_str("## Overview\n\n");
        out.push_str(desc);
        out.push_str("\n\n");
    }

    out.push_str(CURATED_INTRO);

    let definitions = schema
        .get("definitions")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    out.push_str("## Top-level structure (`AppConfig`)\n\n");
    out.push_str("Every field on the root `AppConfig`, alphabetised. Click a type to jump to its per-block reference below.\n\n");
    if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
        render_field_table(props, &definitions, out);
    }

    out.push_str("\n## Per-block reference\n\n");
    out.push_str("Every type reachable from `AppConfig`, alphabetised. Field tables show type, default, and the field's `///` doc-comment summary.\n\n");
    let mut def_names: Vec<&String> = definitions.keys().collect();
    def_names.sort();
    for name in def_names {
        let def = &definitions[name];
        render_definition(name, def, &definitions, out);
    }
}

fn render_field_table(
    props: &serde_json::Map<String, Value>,
    defs: &serde_json::Map<String, Value>,
    out: &mut String,
) {
    out.push_str("| Field | Type | Default | Summary |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    let mut keys: Vec<&String> = props.keys().collect();
    keys.sort();
    for key in keys {
        let field = &props[key];
        let ty = type_of(field, defs);
        let default = default_of(field);
        let summary = first_line(
            field
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
        );
        out.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            escape_md_cell(key),
            escape_md_cell(&ty),
            escape_md_cell(&default),
            escape_md_cell(&summary),
        ));
    }
    out.push('\n');
}

fn render_definition(
    name: &str,
    def: &Value,
    defs: &serde_json::Map<String, Value>,
    out: &mut String,
) {
    out.push_str(&format!("### `{name}`\n\n"));

    if let Some(desc) = def.get("description").and_then(|v| v.as_str()) {
        out.push_str(desc);
        out.push_str("\n\n");
    }

    // Enum-with-variants (typed/untagged unions)
    if let Some(variants) = def.get("oneOf").and_then(|v| v.as_array()) {
        out.push_str("**Variants:**\n\n");
        for v in variants {
            render_variant(v, defs, out);
        }
        return;
    }

    // Plain string-only enum (e.g. AuditOnFailure: "fail_closed" | "fail_open")
    if let Some(values) = def.get("enum").and_then(|v| v.as_array()) {
        out.push_str("**Allowed values:**\n\n");
        for v in values {
            if let Some(s) = v.as_str() {
                out.push_str(&format!("- `{s}`\n"));
            }
        }
        out.push('\n');
        return;
    }

    if let Some(props) = def.get("properties").and_then(|v| v.as_object()) {
        render_field_table(props, defs, out);
    } else {
        // Transparent newtype around a primitive (e.g. transparent BTreeMap wrappers)
        let ty = type_of(def, defs);
        out.push_str(&format!("Type: {ty}\n\n"));
    }
}

fn render_variant(variant: &Value, defs: &serde_json::Map<String, Value>, out: &mut String) {
    let title = variant
        .get("title")
        .and_then(|v| v.as_str())
        .or_else(|| {
            // Tagged unions have a `properties.type` field with a single `enum` value
            variant
                .get("properties")
                .and_then(|p| p.get("type"))
                .and_then(|t| t.get("enum"))
                .and_then(|e| e.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
        })
        .or_else(|| {
            // String-only enum with per-variant docs: `{type: "string", enum: ["fail_closed"]}`
            variant
                .get("enum")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
        })
        .unwrap_or("(unnamed variant)");
    let desc = first_line(
        variant
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
    );
    if desc.is_empty() {
        out.push_str(&format!("- **`{title}`**\n"));
    } else {
        out.push_str(&format!("- **`{title}`** — {desc}\n"));
    }
    if let Some(props) = variant.get("properties").and_then(|v| v.as_object())
        && props.len() > 1
    {
        let mut keys: Vec<&String> = props.keys().filter(|k| *k != "type").collect();
        keys.sort();
        for key in keys {
            let ty = type_of(&props[key], defs);
            out.push_str(&format!("  - `{key}`: {ty}\n"));
        }
    }
    out.push('\n');
}

fn type_of(field: &Value, defs: &serde_json::Map<String, Value>) -> String {
    if let Some(ref_) = field.get("$ref").and_then(|v| v.as_str())
        && let Some(name) = ref_.strip_prefix("#/definitions/")
    {
        return format!("[`{name}`](#{})", anchor(name));
    }
    // Schemars wraps `default + $ref` fields in `allOf: [{$ref: ...}]`.
    if let Some(arr) = field.get("allOf").and_then(|v| v.as_array())
        && arr.len() == 1
    {
        return type_of(&arr[0], defs);
    }
    if let Some(t) = field.get("type") {
        if let Some(s) = t.as_str() {
            return primitive_to_md(s, field, defs);
        }
        if let Some(arr) = t.as_array() {
            // Nullable: ["string", "null"]
            let types: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str())
                .filter(|s| *s != "null")
                .map(|s| primitive_to_md(s, field, defs))
                .collect();
            if types.is_empty() {
                return "null".to_owned();
            }
            return format!("{} (optional)", types.join(" | "));
        }
    }
    if let Some(any_of) = field.get("anyOf").and_then(|v| v.as_array()) {
        let types: Vec<String> = any_of
            .iter()
            .filter(|v| v.get("type").and_then(|t| t.as_str()) != Some("null"))
            .map(|v| type_of(v, defs))
            .collect();
        if types.len() == 1 && any_of.len() == 2 {
            return format!("{} (optional)", types[0]);
        }
        return types.join(" | ");
    }
    if field.get("oneOf").is_some() {
        return "(tagged union — see variant list)".to_owned();
    }
    "any".to_owned()
}

fn primitive_to_md(t: &str, field: &Value, defs: &serde_json::Map<String, Value>) -> String {
    match t {
        "object" => {
            if let Some(add) = field.get("additionalProperties")
                && add != &Value::Bool(false)
            {
                let inner = type_of(add, defs);
                // HTML-escaped so markdown pipelines that parse inline HTML
                // (the docs site renders with rehype-raw) keep the angle
                // brackets as text instead of eating them as a bogus tag.
                return format!("map&lt;string, {inner}&gt;");
            }
            "object".to_owned()
        }
        "array" => {
            if let Some(items) = field.get("items") {
                let inner = type_of(items, defs);
                return format!("array&lt;{inner}&gt;");
            }
            "array".to_owned()
        }
        other => other.to_owned(),
    }
}

fn default_of(field: &Value) -> String {
    match field.get("default") {
        Some(Value::Null) | None => String::new(),
        Some(v) => match v {
            Value::String(s) => format!("`\"{s}\"`"),
            Value::Bool(b) => format!("`{b}`"),
            Value::Number(n) => format!("`{n}`"),
            Value::Array(a) if a.is_empty() => "`[]`".to_owned(),
            Value::Array(_) => "(see type)".to_owned(),
            Value::Object(o) if o.is_empty() => "`{}`".to_owned(),
            // Verbose object defaults clutter the table; the per-block
            // section below shows each field's own default.
            Value::Object(_) => "(see type)".to_owned(),
            Value::Null => String::new(),
        },
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").trim().to_owned()
}

fn escape_md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn anchor(name: &str) -> String {
    name.to_lowercase()
}
