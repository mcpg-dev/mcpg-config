//! `mcpg config explain` — describe a config field by its dotted path.
//!
//! Walks the live `AppConfig` JSON Schema (same one
//! `mcpg config schema` emits) and prints the field's doc-comment,
//! type, default, and any validation hints — sourced directly from
//! `///` rustdoc + `#[serde(...)]` annotations on the gateway's
//! config tree.
//!
//! ```text
//! $ mcpg config explain server.bind_address
//! server.bind_address
//!   Type:    string
//!   Default: "127.0.0.1:8787"
//!
//! $ mcpg config explain audit.on_failure
//! audit.on_failure
//!   Type:    AuditOnFailure (enum)
//!   Default: "fail_closed"
//!   Allowed values:
//!     - fail_closed
//!     - fail_open
//!
//!   Operator policy when an audit-sink emit fails.
//! ```
//!
//! With no path, lists the top-level `AppConfig` fields. Exits 0 on
//! match, 1 on unknown path, 2 on usage error.

use std::process::ExitCode;

use serde_json::Value;

const USAGE: &str = "\
mcpg config explain — describe a config field by its dotted path

USAGE:
    mcpg config explain                      # list top-level fields
    mcpg config explain <path>               # describe one field

EXAMPLES:
    mcpg config explain server.bind_address
    mcpg config explain audit.on_failure
    mcpg config explain bindings[0].name
";

pub fn run(args: Vec<String>) -> ExitCode {
    let path = match args.len() {
        0 => None,
        1 if args[0] == "--help" || args[0] == "-h" => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        1 => Some(args[0].as_str()),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    let schema = serde_json::to_value(schemars::schema_for!(mcpg::config::AppConfig))
        .expect("schema serializes");

    match path {
        None => {
            print_root(&schema);
            ExitCode::SUCCESS
        }
        Some(p) => match walk(&schema, p) {
            Ok(field) => {
                print_field(p, &field, &schema);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
    }
}

fn print_root(schema: &Value) {
    println!("AppConfig — top-level fields:\n");
    if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
        let mut keys: Vec<&String> = props.keys().collect();
        keys.sort();
        for key in keys {
            let summary = props[key]
                .get("description")
                .and_then(|v| v.as_str())
                .map(first_line)
                .unwrap_or_default();
            if summary.is_empty() {
                println!("  {key}");
            } else {
                println!("  {key:24} — {summary}");
            }
        }
    }
    println!("\nUse `mcpg config explain <field>` to drill into a specific block.");
}

fn print_field(path: &str, field: &Value, root: &Value) {
    let resolved = resolve(field, root);
    let ty = type_of(&resolved, root);
    let default = default_of(field).or_else(|| default_of(&resolved));
    let desc = field
        .get("description")
        .and_then(|v| v.as_str())
        .or_else(|| resolved.get("description").and_then(|v| v.as_str()))
        .unwrap_or("");

    println!("{path}");
    println!("  Type:    {ty}");
    if let Some(d) = default {
        println!("  Default: {d}");
    }
    if let Some(values) = enum_values(&resolved) {
        println!("  Allowed values:");
        for v in values {
            println!("    - {v}");
        }
    }
    if let Some(props) = resolved.get("properties").and_then(|v| v.as_object()) {
        let mut keys: Vec<&String> = props.keys().collect();
        keys.sort();
        if !keys.is_empty() {
            println!("  Fields:");
            for key in keys {
                let inner_ty = type_of(&props[key], root);
                println!("    - {key}: {inner_ty}");
            }
        }
    }
    if !desc.is_empty() {
        println!();
        for line in desc.lines() {
            println!("  {line}");
        }
    }
}

/// Walk a dotted path (`a.b.c[0].d`) against the schema, following
/// `$ref` / `allOf` indirection at each hop.
fn walk(root: &Value, path: &str) -> Result<Value, String> {
    let mut cursor: Value = root.clone();
    for token in tokenize(path) {
        cursor = resolve(&cursor, root);
        match token {
            Token::Field(name) => {
                let props = cursor
                    .get("properties")
                    .and_then(|v| v.as_object())
                    .ok_or_else(|| {
                        format!(
                            "error: cannot descend into '{}' — not an object at this point in path '{path}'",
                            name,
                        )
                    })?;
                cursor = props
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("error: unknown field '{name}' in path '{path}'"))?;
            }
            Token::Index => {
                cursor = cursor.get("items").cloned().ok_or_else(|| {
                    format!("error: '{path}' uses [N] indexing but element is not an array")
                })?;
            }
        }
    }
    Ok(cursor)
}

#[derive(Debug)]
enum Token<'a> {
    Field(&'a str),
    Index,
}

fn tokenize(path: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    for segment in path.split('.') {
        // segment may be `name` or `name[0]` or `name[0][1]`
        let mut rest = segment;
        if let Some(idx) = rest.find('[') {
            let (head, tail) = rest.split_at(idx);
            if !head.is_empty() {
                out.push(Token::Field(head));
            }
            rest = tail;
            while !rest.is_empty() {
                if let Some(end) = rest.find(']') {
                    out.push(Token::Index);
                    rest = &rest[end + 1..];
                } else {
                    break;
                }
            }
        } else if !segment.is_empty() {
            out.push(Token::Field(segment));
        }
    }
    out
}

/// Follow `$ref` and `allOf: [{$ref}]` indirection one level so the
/// caller sees the underlying object schema.
fn resolve(node: &Value, root: &Value) -> Value {
    if let Some(ref_) = node.get("$ref").and_then(|v| v.as_str())
        && let Some(name) = ref_.strip_prefix("#/definitions/")
        && let Some(def) = root.get("definitions").and_then(|v| v.get(name))
    {
        return def.clone();
    }
    if let Some(arr) = node.get("allOf").and_then(|v| v.as_array())
        && arr.len() == 1
    {
        return resolve(&arr[0], root);
    }
    node.clone()
}

fn type_of(node: &Value, root: &Value) -> String {
    if let Some(ref_) = node.get("$ref").and_then(|v| v.as_str())
        && let Some(name) = ref_.strip_prefix("#/definitions/")
    {
        return format!("{name} (see definition)");
    }
    if let Some(arr) = node.get("allOf").and_then(|v| v.as_array())
        && arr.len() == 1
    {
        return type_of(&arr[0], root);
    }
    if let Some(t) = node.get("type") {
        if let Some(s) = t.as_str() {
            return primitive_to_str(s, node, root);
        }
        if let Some(arr) = t.as_array() {
            let types: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str())
                .filter(|s| *s != "null")
                .map(|s| primitive_to_str(s, node, root))
                .collect();
            if types.is_empty() {
                return "null".to_owned();
            }
            return format!("{} (optional)", types.join(" | "));
        }
    }
    if let Some(any_of) = node.get("anyOf").and_then(|v| v.as_array()) {
        let types: Vec<String> = any_of
            .iter()
            .filter(|v| v.get("type").and_then(|t| t.as_str()) != Some("null"))
            .map(|v| type_of(v, root))
            .collect();
        if types.len() == 1 && any_of.len() == 2 {
            return format!("{} (optional)", types[0]);
        }
        return types.join(" | ");
    }
    if let Some(arr) = node.get("oneOf").and_then(|v| v.as_array()) {
        // String-only enum with per-variant docs (schemars 0.8 shape):
        // every entry is `{type: "string", enum: [literal]}`.
        let all_string_enums = arr.iter().all(|v| {
            v.get("type").and_then(|t| t.as_str()) == Some("string")
                && v.get("enum").and_then(|e| e.as_array()).map(|a| a.len()) == Some(1)
        });
        if all_string_enums && !arr.is_empty() {
            return "string (enum)".to_owned();
        }
        return "tagged union".to_owned();
    }
    if node.get("enum").is_some() {
        return "string (enum)".to_owned();
    }
    "any".to_owned()
}

fn primitive_to_str(t: &str, node: &Value, root: &Value) -> String {
    match t {
        "object" => {
            if let Some(add) = node.get("additionalProperties")
                && add != &Value::Bool(false)
            {
                return format!("map<string, {}>", type_of(add, root));
            }
            "object".to_owned()
        }
        "array" => {
            if let Some(items) = node.get("items") {
                return format!("array<{}>", type_of(items, root));
            }
            "array".to_owned()
        }
        other => other.to_owned(),
    }
}

fn default_of(node: &Value) -> Option<String> {
    match node.get("default")? {
        Value::Null => None,
        Value::String(s) => Some(format!("\"{s}\"")),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Array(a) if a.is_empty() => Some("[]".to_owned()),
        Value::Object(o) if o.is_empty() => Some("{}".to_owned()),
        v => serde_json::to_string(v).ok(),
    }
}

fn enum_values(node: &Value) -> Option<Vec<String>> {
    // String-only enum (`#[serde(rename_all = "snake_case")]`).
    if let Some(arr) = node.get("enum").and_then(|v| v.as_array()) {
        return Some(
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect(),
        );
    }
    // schemars 0.8 emits per-variant docs as oneOf with each entry's
    // own `enum: [literal]`. Aggregate them.
    if let Some(arr) = node.get("oneOf").and_then(|v| v.as_array()) {
        let mut values = Vec::new();
        for entry in arr {
            if let Some(lit) = entry
                .get("enum")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
            {
                values.push(lit.to_owned());
            }
        }
        if !values.is_empty() {
            return Some(values);
        }
    }
    None
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").trim().to_owned()
}
