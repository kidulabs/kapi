//! Meta-schema management.
//!
//! Provides a hardcoded JSON Schema constant (Draft 2020-12) that defines
//! the shape of valid Schema registration payloads, and a `JsonSchemaValidator`
//! wrapper that implements the `SchemaValidator` trait — isolating `ObjectService`
//! from the `jsonschema` crate dependency.

use jsonschema::draft202012;
use serde_json::Value;

/// A structured validation failure carrying the path and message.
pub struct SchemaValidationError {
    pub instance_path: String,
    pub message: String,
}

/// Trait abstracting JSON Schema validation.
///
/// This trait allows `ObjectService` to validate payloads without depending
/// on the `jsonschema` crate directly. It requires `Send + Sync` so it can
/// be stored as `Arc<dyn SchemaValidator>`.
pub trait SchemaValidator: Send + Sync {
    /// Returns `true` if the instance validates against the schema.
    fn is_valid(&self, instance: &Value) -> bool;
    /// Returns a list of validation errors.
    fn validate(&self, instance: &Value) -> Vec<SchemaValidationError>;
}

/// A `jsonschema::Validator` wrapper that implements `SchemaValidator`.
///
/// This is the production implementation. Compilation delegates to
/// `draft202012::options().build()`, and validation maps the
/// `jsonschema` error iterator to domain `SchemaValidationError` values.
pub struct JsonSchemaValidator {
    inner: jsonschema::Validator,
}

impl JsonSchemaValidator {
    /// Compiles a JSON Schema value into a `JsonSchemaValidator`.
    pub fn compile(schema_json: &Value) -> Result<Self, anyhow::Error> {
        let validator = draft202012::options()
            .build(schema_json)
            .map_err(|e| anyhow::anyhow!("failed to compile schema: {}", e))?;
        Ok(Self { inner: validator })
    }
}

impl SchemaValidator for JsonSchemaValidator {
    fn is_valid(&self, instance: &Value) -> bool {
        self.inner.is_valid(instance)
    }

    fn validate(&self, instance: &Value) -> Vec<SchemaValidationError> {
        self.inner
            .iter_errors(instance)
            .map(|e| SchemaValidationError {
                instance_path: e
                    .instance_path()
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join("/"),
                message: e.to_string(),
            })
            .collect()
    }
}

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
    "jsonSchema": { "type": "object" },
    "statusSchema": { "type": "object" }
  },
  "unevaluatedProperties": false
}"#;

/// Compiles the meta-schema into a reusable `JsonSchemaValidator`.
///
/// Called once at server startup. The resulting validator is injected into
/// `ObjectService` for validating Schema registration payloads.
pub fn compile_meta_schema() -> Result<JsonSchemaValidator, anyhow::Error> {
    let schema_json: serde_json::Value = serde_json::from_str(META_SCHEMA_JSON)
        .map_err(|e| anyhow::anyhow!("failed to parse meta-schema JSON: {}", e))?;

    JsonSchemaValidator::compile(&schema_json)
        .map_err(|e| anyhow::anyhow!("failed to compile meta-schema: {}", e))
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
        let errors = validator.validate(&missing_target_group);
        assert!(!errors.is_empty(), "expected validation errors");
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

    // T11: statusSchema as optional property — valid with statusSchema
    #[test]
    fn valid_schema_with_status_schema_passes_meta_schema() {
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
            },
            "statusSchema": {
                "type": "object",
                "properties": {
                    "phase": { "type": "string" }
                }
            }
        });
        assert!(validator.is_valid(&valid_payload));
    }

    // T12: statusSchema as non-object fails meta-schema validation
    #[test]
    fn status_schema_as_non_object_fails_meta_schema() {
        let validator = compile_meta_schema().expect("meta-schema should compile");
        let invalid_payload = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "jsonSchema": { "type": "object" },
            "statusSchema": "not an object"
        });
        assert!(!validator.is_valid(&invalid_payload));
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
