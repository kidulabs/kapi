//! Meta-schema management.
//!
//! Provides a hardcoded JSON Schema constant (Draft 2020-12) that defines
//! the shape of valid Schema registration payloads, and a compilation function
//! that returns a `jsonschema::Validator` for use at server startup.

use jsonschema::draft202012;
use jsonschema::Validator;

/// Meta-schema JSON constant defining valid Schema registration shape.
///
/// Uses Draft 2020-12 with `unevaluatedProperties: false` to reject unknown fields.
/// Required fields: targetGroup, targetVersion, targetKind, jsonSchema.
pub const META_SCHEMA_JSON: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "required": ["targetGroup", "targetVersion", "targetKind", "jsonSchema"],
  "properties": {
    "targetGroup": { "type": "string", "minLength": 1 },
    "targetVersion": { "type": "string", "minLength": 1 },
    "targetKind": { "type": "string", "minLength": 1 },
    "jsonSchema": { "type": "object" }
  },
  "unevaluatedProperties": false
}"#;

/// Compiles the meta-schema into a reusable `jsonschema::Validator`.
///
/// Called once at server startup. The resulting validator is injected into
/// `ObjectService` for validating Schema registration payloads.
pub fn compile_meta_schema() -> Result<Validator, anyhow::Error> {
    // Parse the meta-schema JSON constant
    let schema_json: serde_json::Value = serde_json::from_str(META_SCHEMA_JSON)
        .map_err(|e| anyhow::anyhow!("failed to parse meta-schema JSON: {}", e))?;

    // Build a Draft 2020-12 validator
    let validator = draft202012::options()
        .build(&schema_json)
        .map_err(|e| anyhow::anyhow!("failed to compile meta-schema: {}", e))?;

    Ok(validator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // T7: Valid Schema registration payload passes meta-schema validation
    #[test]
    fn valid_schema_passes_meta_schema() {
        let validator = compile_meta_schema().expect("meta-schema should compile");
        let valid_payload = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "jsonSchema": {
                "type": "object",
                "properties": {
                    "color": { "type": "string" }
                }
            }
        });
        assert!(validator.is_valid(&valid_payload));
    }

    // T8: Missing required field fails meta-schema validation
    #[test]
    fn missing_required_field_fails_meta_schema() {
        let validator = compile_meta_schema().expect("meta-schema should compile");
        let missing_target_group = json!({
            "targetVersion": "v1",
            "targetKind": "Widget",
            "jsonSchema": { "type": "object" }
        });
        assert!(!validator.is_valid(&missing_target_group));
        assert!(validator.iter_errors(&missing_target_group).count() > 0);
    }

    // T9: Unknown field fails meta-schema validation (unevaluatedProperties: false)
    #[test]
    fn unknown_field_fails_meta_schema() {
        let validator = compile_meta_schema().expect("meta-schema should compile");
        let with_unknown_field = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "jsonSchema": { "type": "object" },
            "unknownField": "should be rejected"
        });
        assert!(!validator.is_valid(&with_unknown_field));
    }

    // T10: jsonSchema as non-object fails meta-schema validation
    #[test]
    fn json_schema_as_non_object_fails_meta_schema() {
        let validator = compile_meta_schema().expect("meta-schema should compile");
        let json_schema_as_string = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "jsonSchema": "not an object"
        });
        assert!(!validator.is_valid(&json_schema_as_string));
    }

    // T11: compile_meta_schema() returns a working validator
    #[test]
    fn compile_meta_schema_returns_working_validator() {
        let result = compile_meta_schema();
        assert!(result.is_ok());
        let validator = result.unwrap();
        // Verify it can validate both valid and invalid payloads
        let valid = json!({
            "targetGroup": "x",
            "targetVersion": "y",
            "targetKind": "z",
            "jsonSchema": {}
        });
        assert!(validator.is_valid(&valid));
        let invalid = json!({});
        assert!(!validator.is_valid(&invalid));
    }
}
