//! Static and dynamic component schema builders for OpenAPI 3.0.3 spec generation.
//!
//! Generates the component schemas under `components/schemas` in the OpenAPI
//! document. Includes built-in kapi types (ResourceKey, StoredObject, etc.)
//! and dynamically generated per-kind schemas from registered Schema objects.

use serde_json::{Value, json};

use crate::object::types::SchemaData;

/// Converts a schema name (format: `{Kind}.{group}`) into an OpenAPI component name.
///
/// Splits on dots, PascalCases each segment, and concatenates them.
///
/// # Examples
///
/// ```
/// # use kapi::openapi::component_name;
/// assert_eq!(component_name("Widget.example.io"), "WidgetExampleIo");
/// assert_eq!(component_name("Deployment.apps"), "DeploymentApps");
/// assert_eq!(component_name("Widget.example.io"), component_name("Widget.example.io"));
/// ```
pub fn component_name(schema_name: &str) -> String {
    schema_name
        .split('.')
        .filter(|s| !s.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let upper = first.to_uppercase();
                    let lower = chars.as_str().to_lowercase();
                    format!("{upper}{lower}")
                }
            }
        })
        .collect::<Vec<_>>()
        .concat()
}

/// Returns the static component schemas for kapi built-in types.
///
/// These are always present regardless of which schemas are registered.
pub(crate) fn build_static_components() -> Vec<(String, Value)> {
    vec![
        // ResourceKey: { group, version, kind }
        (
            "ResourceKey".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "group": { "type": "string" },
                    "version": { "type": "string" },
                    "kind": { "type": "string" }
                },
                "required": ["group", "version", "kind"]
            }),
        ),
        // ObjectMeta: user-controlled metadata
        (
            "ObjectMeta".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "labels": {
                        "type": "object",
                        "additionalProperties": { "type": "string" }
                    }
                },
                "required": ["name"]
            }),
        ),
        // SystemMetadata: server-managed lifecycle fields
        (
            "SystemMetadata".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "resourceVersion": { "type": "integer", "format": "int64" },
                    "createdAt": { "type": "string", "format": "date-time" },
                    "updatedAt": { "type": "string", "format": "date-time" }
                },
                "required": ["resourceVersion", "createdAt", "updatedAt"]
            }),
        ),
        // UserData: envelope holding arbitrary JSON data
        (
            "UserData".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "value": {}
                },
                "required": ["value"]
            }),
        ),
        // StoredObject: generic envelope wrapping a stored resource
        (
            "StoredObject".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "key": { "$ref": "#/components/schemas/ResourceKey" },
                    "metadata": { "$ref": "#/components/schemas/ObjectMeta" },
                    "system": { "$ref": "#/components/schemas/SystemMetadata" },
                    "data": { "$ref": "#/components/schemas/UserData" }
                },
                "required": ["key", "metadata", "system", "data"]
            }),
        ),
        // ListResponse: paginated list of StoredObjects
        (
            "ListResponse".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "items": {
                        "type": "array",
                        "items": { "$ref": "#/components/schemas/StoredObject" }
                    },
                    "continueToken": {
                        "type": "string",
                        "nullable": true
                    }
                },
                "required": ["items"]
            }),
        ),
        // WatchEvent: SSE event payload with type and object
        (
            "WatchEvent".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "eventType": { "$ref": "#/components/schemas/WatchEventType" },
                    "object": { "$ref": "#/components/schemas/StoredObject" }
                },
                "required": ["eventType", "object"]
            }),
        ),
        // WatchEventType: enum of event kinds
        (
            "WatchEventType".to_string(),
            json!({
                "type": "string",
                "enum": ["Added", "Modified", "Deleted"]
            }),
        ),
        // ValidationError: field-level validation failure
        (
            "ValidationError".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "message": { "type": "string" }
                },
                "required": ["path", "message"]
            }),
        ),
        // InvalidFieldSelector: invalid fieldSelector query parameter error
        (
            "InvalidFieldSelector".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "error": { "type": "string" },
                    "code": { "type": "string" },
                    "details": {
                        "type": "object",
                        "properties": {
                            "message": { "type": "string" }
                        }
                    }
                },
                "required": ["error", "code"]
            }),
        ),
        // AppError: standard error response shape
        (
            "AppError".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "error": { "type": "string" },
                    "code": { "type": "string" },
                    "details": {}
                },
                "required": ["error", "code"]
            }),
        ),
        // SchemaData: payload for Schema registration
        (
            "SchemaData".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "targetGroup": { "type": "string" },
                    "targetVersion": { "type": "string" },
                    "targetKind": { "type": "string" },
                    "jsonSchema": {}
                },
                "required": ["targetGroup", "targetVersion", "targetKind", "jsonSchema"]
            }),
        ),
    ]
}

/// Builds the data component schema for a registered kind.
///
/// Wraps the user's `jsonSchema` as an OpenAPI Schema Object.
pub(crate) fn build_kind_data_component(
    schema_data: &SchemaData,
    comp_name: &str,
) -> (String, Value) {
    let schema = json!({
        "type": "object",
        "properties": {
            "value": schema_data.json_schema
        },
        "required": ["value"]
    });
    (comp_name.to_string(), schema)
}

/// Builds the StoredObject envelope component for a registered kind.
///
/// Mirrors the wire format: `{ key, metadata, system, data }` where `data` references
/// the kind-specific data component.
pub(crate) fn build_kind_stored_object_component(comp_name: &str) -> (String, Value) {
    let stored_name = format!("{comp_name}StoredObject");
    let schema = json!({
        "type": "object",
        "properties": {
            "key": { "$ref": "#/components/schemas/ResourceKey" },
            "metadata": { "$ref": "#/components/schemas/ObjectMeta" },
            "system": { "$ref": "#/components/schemas/SystemMetadata" },
            "data": { "$ref": format!("#/components/schemas/{comp_name}") }
        },
        "required": ["key", "metadata", "system", "data"]
    });
    (stored_name, schema)
}

/// Builds the ListResponse component for a registered kind.
///
/// Contains `items` array referencing the kind-specific StoredObject and
/// an optional `continueToken` for pagination.
pub(crate) fn build_kind_list_response_component(comp_name: &str) -> (String, Value) {
    let list_name = format!("{comp_name}ListResponse");
    let stored_name = format!("{comp_name}StoredObject");
    let schema = json!({
        "type": "object",
        "properties": {
            "items": {
                "type": "array",
                "items": { "$ref": format!("#/components/schemas/{stored_name}") }
            },
            "continueToken": {
                "type": "string",
                "nullable": true
            }
        },
        "required": ["items"]
    });
    (list_name, schema)
}
