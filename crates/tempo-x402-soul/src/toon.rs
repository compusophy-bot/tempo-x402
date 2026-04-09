//! TOON — Token-Oriented Object Notation.
//!
//! Compact text encoding for structured data in LLM prompts.
//! Saves 30-60% tokens vs JSON by eliminating redundant keys, quotes, and braces.
//!
//! ## Format
//!
//! **Array of objects** (tabular):
//! ```text
//! slug | price | description
//! hello | $0.10 | A greeting endpoint
//! math | $0.25 | Arithmetic API
//! ```
//!
//! **Single object** (key=value):
//! ```text
//! uptime = 3600
//! endpoints = 5
//! revenue = $12.50
//! ```
//!
//! **Nested objects** flatten with dot notation: `identity.address = 0x1234`

use serde_json::Value;

/// Encode a JSON value as TOON.
///
/// - Arrays of objects → pipe-separated table (header + rows)
/// - Objects → key = value lines (nested keys use dot notation)
/// - Primitives → string representation
pub fn to_toon(value: &Value) -> String {
    match value {
        Value::Array(arr) if !arr.is_empty() && arr[0].is_object() => array_to_table(arr),
        Value::Array(arr) => arr
            .iter()
            .map(|v| value_to_string(v))
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(map) => object_to_kv(map, ""),
        other => value_to_string(other),
    }
}

/// Encode a NodeSnapshot as compact TOON for chat context.
/// Much more token-efficient than serde_json::to_string.
pub fn snapshot_to_toon(value: &Value) -> String {
    let mut lines = Vec::new();

    if let Value::Object(map) = value {
        // Scalar fields first
        for (key, val) in map {
            match val {
                Value::Array(arr) if !arr.is_empty() && arr[0].is_object() => {
                    // Will handle tables below
                }
                Value::Array(arr) if arr.is_empty() => {} // Skip empty arrays
                Value::Object(inner) => {
                    // Flatten nested object with dot notation
                    for (k2, v2) in inner {
                        if !is_empty_value(v2) {
                            lines.push(format!("{key}.{k2} = {}", value_to_string(v2)));
                        }
                    }
                }
                val if !is_empty_value(val) => {
                    lines.push(format!("{key} = {}", value_to_string(val)));
                }
                _ => {}
            }
        }

        // Then tables for arrays of objects
        for (key, val) in map {
            if let Value::Array(arr) = val {
                if !arr.is_empty() && arr[0].is_object() {
                    lines.push(String::new());
                    lines.push(format!("[{key}]"));
                    lines.push(array_to_table(arr));
                }
            }
        }
    }

    lines.join("\n")
}

/// Array of objects → pipe-separated table.
fn array_to_table(arr: &[Value]) -> String {
    // Collect all keys from all objects (preserving order from first object)
    let mut keys = Vec::new();
    if let Some(Value::Object(first)) = arr.first() {
        for key in first.keys() {
            keys.push(key.clone());
        }
    }
    // Add any keys from later objects that weren't in the first
    for item in arr.iter().skip(1) {
        if let Value::Object(map) = item {
            for key in map.keys() {
                if !keys.contains(key) {
                    keys.push(key.clone());
                }
            }
        }
    }

    // Skip columns that are empty/null in ALL rows
    let keys: Vec<String> = keys
        .into_iter()
        .filter(|key| {
            arr.iter()
                .any(|item| !is_empty_value(item.get(key).unwrap_or(&Value::Null)))
        })
        .collect();

    if keys.is_empty() {
        return String::new();
    }

    let mut lines = Vec::with_capacity(arr.len() + 1);

    // Header
    lines.push(keys.join(" | "));

    // Rows
    for item in arr {
        let row: Vec<String> = keys
            .iter()
            .map(|key| {
                item.get(key)
                    .map(|v| value_to_string(v))
                    .unwrap_or_else(|| "-".to_string())
            })
            .collect();
        lines.push(row.join(" | "));
    }

    lines.join("\n")
}

/// Object → key = value lines with optional prefix for nesting.
fn object_to_kv(map: &serde_json::Map<String, Value>, prefix: &str) -> String {
    let mut lines = Vec::new();

    for (key, val) in map {
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };

        match val {
            Value::Object(inner) => {
                lines.push(object_to_kv(inner, &full_key));
            }
            val if !is_empty_value(val) => {
                lines.push(format!("{full_key} = {}", value_to_string(val)));
            }
            _ => {}
        }
    }

    lines.join("\n")
}

/// Convert a JSON value to a compact string representation.
fn value_to_string(val: &Value) -> String {
    match val {
        Value::Null => "-".to_string(),
        Value::Bool(b) => if *b { "yes" } else { "no" }.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .iter()
            .map(|v| value_to_string(v))
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(_) => "[obj]".to_string(),
    }
}

/// Check if a value is "empty" (null, empty string, empty array).
fn is_empty_value(val: &Value) -> bool {
    match val {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(arr) => arr.is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_array_to_table() {
        let data = json!([
            {"slug": "hello", "price": "$0.10", "desc": "Greeting"},
            {"slug": "math", "price": "$0.25", "desc": "Arithmetic"}
        ]);
        let result = to_toon(&data);
        // BTreeMap orders keys alphabetically: desc, price, slug
        assert!(result.contains("desc | price | slug"));
        assert!(result.contains("Greeting | $0.10 | hello"));
        assert!(result.contains("Arithmetic | $0.25 | math"));
    }

    #[test]
    fn test_object_to_kv() {
        let data = json!({
            "uptime": 3600,
            "endpoints": 5,
            "revenue": "$12.50"
        });
        let result = to_toon(&data);
        assert!(result.contains("uptime = 3600"));
        assert!(result.contains("endpoints = 5"));
        assert!(result.contains("revenue = $12.50"));
    }

    #[test]
    fn test_snapshot_compact() {
        let snapshot = json!({
            "uptime_secs": 7200,
            "endpoint_count": 3,
            "total_revenue": "$5.00",
            "endpoints": [
                {"slug": "info", "price": "$0.01", "requests": 42},
                {"slug": "chat", "price": "$0.05", "requests": 10}
            ],
            "peers": []
        });
        let result = snapshot_to_toon(&snapshot);
        assert!(result.contains("uptime_secs = 7200"));
        // BTreeMap orders keys alphabetically
        assert!(result.contains("price | requests | slug"));
        assert!(!result.contains("peers")); // empty array skipped
    }

    #[test]
    fn test_skips_empty() {
        let data = json!({
            "name": "test",
            "empty": "",
            "null_val": null,
            "empty_arr": []
        });
        let result = to_toon(&data);
        assert!(result.contains("name = test"));
        assert!(!result.contains("empty"));
        assert!(!result.contains("null_val"));
    }
}
