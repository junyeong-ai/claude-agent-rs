//! Schema transformation utilities for structured outputs.

use serde_json::Value;

/// Properties not supported by Claude API structured outputs.
///
/// Per Claude API docs (2025-11-13):
/// - Numerical constraints: minimum, maximum, exclusiveMinimum, exclusiveMaximum, multipleOf
/// - String constraints: minLength, maxLength
/// - Array constraints: minItems (only 0,1 supported), maxItems
/// - Object constraints: minProperties, maxProperties
///
/// Supported but kept:
/// - default: supported for all types
/// - format: supported for date-time, time, date, duration, email, hostname, uri, ipv4, ipv6, uuid
/// - pattern: supported for simple regex (no backreferences, lookahead, word boundaries)
const UNSUPPORTED_PROPERTIES: &[&str] = &[
    // Numerical constraints
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    // String constraints
    "minLength",
    "maxLength",
    // Array constraints (minItems only supports 0,1; maxItems not supported)
    "minItems",
    "maxItems",
    // Object constraints
    "minProperties",
    "maxProperties",
];

/// Transform a schema for strict mode compatibility.
///
/// This function prepares a JSON schema for Claude's structured outputs:
/// - Adds `additionalProperties: false` to all objects
/// - Auto-generates `required` array if not present (all properties become required)
/// - Removes unsupported constraints (see `UNSUPPORTED_PROPERTIES`)
///
/// Supported properties are preserved: `default`, `format`, `pattern`, `enum`, `const`.
pub fn transform_for_strict(schema: Value) -> Value {
    transform_object(schema)
}

fn transform_object(mut value: Value) -> Value {
    if let Value::Object(ref mut map) = value {
        // Remove unsupported properties
        for prop in UNSUPPORTED_PROPERTIES {
            map.remove(*prop);
        }

        if map.get("type") == Some(&Value::String("object".to_string())) {
            map.insert("additionalProperties".to_string(), Value::Bool(false));

            if !map.contains_key("required")
                && let Some(Value::Object(props)) = map.get("properties")
            {
                let keys: Vec<Value> = props.keys().map(|k| Value::String(k.clone())).collect();
                if !keys.is_empty() {
                    map.insert("required".to_string(), Value::Array(keys));
                }
            }
        }

        for (_, v) in map.iter_mut() {
            *v = transform_object(std::mem::take(v));
        }
    }

    if let Value::Array(ref mut arr) = value {
        for v in arr.iter_mut() {
            *v = transform_object(std::mem::take(v));
        }
    }

    value
}

/// Generate a strict schema from a Rust type using schemars.
pub fn strict_schema<T: schemars::JsonSchema>() -> Value {
    let schema = schemars::schema_for!(T);
    let value = serde_json::to_value(schema).unwrap_or_default();
    transform_for_strict(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_transform_adds_additional_properties() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });

        let result = transform_for_strict(schema);

        assert_eq!(result["additionalProperties"], false);
        assert!(
            result["required"]
                .as_array()
                .unwrap()
                .contains(&json!("name"))
        );
    }

    #[test]
    fn test_transform_nested_objects() {
        let schema = json!({
            "type": "object",
            "properties": {
                "person": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"}
                    }
                }
            }
        });

        let result = transform_for_strict(schema);

        assert_eq!(result["additionalProperties"], false);
        assert_eq!(
            result["properties"]["person"]["additionalProperties"],
            false
        );
    }

    #[test]
    fn test_transform_preserves_existing_required() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            },
            "required": ["name"]
        });

        let result = transform_for_strict(schema);

        assert_eq!(result["required"], json!(["name"]));
    }
}
