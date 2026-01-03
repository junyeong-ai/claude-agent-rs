//! Schema transformation utilities for structured outputs.

use serde_json::Value;

/// Transform a schema for strict mode compatibility.
/// Adds `additionalProperties: false` to all objects and ensures `required` fields are present.
pub fn transform_for_strict(schema: Value) -> Value {
    transform_object(schema)
}

fn transform_object(mut value: Value) -> Value {
    if let Value::Object(ref mut map) = value {
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
