//! Dynamic OpenAPI 3.0.3 spec generation.
//!
//! Builds an OpenAPI 3.0.3 document from scratch on every request by:
//! 1. Including static component schemas for kapi built-in types
//! 2. Including static paths for Schema CRUD operations
//! 3. Querying the store for registered Schema objects
//! 4. Generating per-kind paths and component schemas for each registered Schema

use serde_json::{json, Value};

use axum::extract::State;
use axum::response::Html;
use axum::Json;

use crate::error::AppError;
use crate::object::service::ObjectService;
use crate::object::types::{ListOptions, SchemaData};
use crate::routes::AppState;
use crate::store::ResourceKey;

/// Returns the ResourceKey for Schema objects stored under the kapi.io group.
fn schema_resource_key() -> ResourceKey {
    ResourceKey {
        group: "kapi.io".to_string(),
        version: "v1".to_string(),
        kind: "Schema".to_string(),
    }
}

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

/// Builds a complete OpenAPI 3.0.3 document.
///
/// Queries the store for registered Schema objects and generates:
/// - Static components/schemas for kapi built-in types
/// - Static paths for Schema CRUD (/apis/kapi.io/v1/Schema)
/// - Dynamic per-kind paths and component schemas for each registered Schema
pub async fn build_openapi_spec(service: &ObjectService) -> Result<Value, AppError> {
    // Build all components (static + dynamic)
    let mut all_schemas = serde_json::Map::new();
    for (name, schema) in build_static_components() {
        all_schemas.insert(name, schema);
    }

    // Build all paths (static + dynamic)
    let mut all_paths = serde_json::Map::new();
    for (path, path_item) in build_static_paths() {
        all_paths.insert(path, path_item);
    }

    // Discover registered schemas by listing Schema objects from the store
    let schema_list = service
        .list(schema_resource_key(), ListOptions {
            limit: None,
            continue_token: None,
        })
        .await?;

    // Parse each StoredObject's data field into SchemaData and generate dynamic content
    for item in &schema_list.items {
        let schema_data: SchemaData = match serde_json::from_value(item.data.value.clone()) {
            Ok(sd) => sd,
            Err(_) => continue,
        };

        let schema_name = format!("{}.{}", schema_data.target_kind, schema_data.target_group);
        let comp_name = component_name(&schema_name);

        // Generate the three component schemas for this kind
        let (data_name, data_schema) = build_kind_data_component(&schema_data, &comp_name);
        all_schemas.insert(data_name, data_schema);

        let (stored_name, stored_schema) = build_kind_stored_object_component(&comp_name);
        all_schemas.insert(stored_name, stored_schema);

        let (list_name, list_schema) = build_kind_list_response_component(&comp_name);
        all_schemas.insert(list_name, list_schema);

        // Generate dynamic paths for this kind
        for (path, path_item) in build_kind_paths(&schema_data, &comp_name) {
            all_paths.insert(path, path_item);
        }
    }

    let document = json!({
        "openapi": "3.0.3",
        "info": {
            "title": "kapi API",
            "version": "0.1.0",
            "description": "Dynamic Kubernetes-style API server"
        },
        "paths": all_paths,
        "components": {
            "schemas": all_schemas
        }
    });

    Ok(document)
}

/// Handler for `GET /openapi`.
///
/// Calls `build_openapi_spec` to generate the OpenAPI document from the
/// current state of registered schemas, and returns it as JSON.
pub async fn get_openapi_handler(
    State(state): State<AppState>,
) -> Result<Json<Value>, AppError> {
    let spec = build_openapi_spec(&state.object_service).await?;
    Ok(Json(spec))
}

/// Swagger UI HTML page loaded from CDN, configured to fetch the spec from `/openapi`.
const SWAGGER_UI_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>kapi API — Swagger UI</title>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui.css" />
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>
    SwaggerUIBundle({ url: "/openapi", dom_id: "#swagger-ui" });
  </script>
</body>
</html>"##;

/// Handler for `GET /swagger-ui/`.
///
/// Serves a minimal HTML page that loads Swagger UI from CDN and points to `/openapi`.
pub async fn get_swagger_ui_handler() -> Html<&'static str> {
    Html(SWAGGER_UI_HTML)
}

/// Returns the static component schemas for kapi built-in types.
///
/// These are always present regardless of which schemas are registered.
fn build_static_components() -> Vec<(String, Value)> {
    vec![
        // ResourceKey: { group, version, kind }
        ("ResourceKey".to_string(), json!({
            "type": "object",
            "properties": {
                "group": { "type": "string" },
                "version": { "type": "string" },
                "kind": { "type": "string" }
            },
            "required": ["group", "version", "kind"]
        })),
        // ObjectMetadata: metadata with versioning and timestamps
        ("ObjectMetadata".to_string(), json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "resourceVersion": { "type": "integer", "format": "int64" },
                "createdAt": { "type": "string", "format": "date-time" },
                "updatedAt": { "type": "string", "format": "date-time" }
            },
            "required": ["name", "resourceVersion", "createdAt", "updatedAt"]
        })),
        // UserData: envelope holding arbitrary JSON data
        ("UserData".to_string(), json!({
            "type": "object",
            "properties": {
                "value": {}
            },
            "required": ["value"]
        })),
        // StoredObject: generic envelope wrapping a stored resource
        ("StoredObject".to_string(), json!({
            "type": "object",
            "properties": {
                "key": { "$ref": "#/components/schemas/ResourceKey" },
                "metadata": { "$ref": "#/components/schemas/ObjectMetadata" },
                "data": { "$ref": "#/components/schemas/UserData" }
            },
            "required": ["key", "metadata", "data"]
        })),
        // ListResponse: paginated list of StoredObjects
        ("ListResponse".to_string(), json!({
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
        })),
        // WatchEvent: SSE event payload with type and object
        ("WatchEvent".to_string(), json!({
            "type": "object",
            "properties": {
                "eventType": { "$ref": "#/components/schemas/WatchEventType" },
                "object": { "$ref": "#/components/schemas/StoredObject" }
            },
            "required": ["eventType", "object"]
        })),
        // WatchEventType: enum of event kinds
        ("WatchEventType".to_string(), json!({
            "type": "string",
            "enum": ["Added", "Modified", "Deleted"]
        })),
        // ValidationError: field-level validation failure
        ("ValidationError".to_string(), json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "message": { "type": "string" }
            },
            "required": ["path", "message"]
        })),
        // AppError: standard error response shape
        ("AppError".to_string(), json!({
            "type": "object",
            "properties": {
                "error": { "type": "string" },
                "code": { "type": "string" },
                "details": {}
            },
            "required": ["error", "code"]
        })),
        // SchemaData: payload for Schema registration
        ("SchemaData".to_string(), json!({
            "type": "object",
            "properties": {
                "targetGroup": { "type": "string" },
                "targetVersion": { "type": "string" },
                "targetKind": { "type": "string" },
                "jsonSchema": {}
            },
            "required": ["targetGroup", "targetVersion", "targetKind", "jsonSchema"]
        })),
    ]
}

/// Builds the request body schema for creating a Schema object.
///
/// Combines `metadata.name` (required by the handler) with the SchemaData component.
fn schema_create_request_schema() -> Value {
    json!({
        "allOf": [
            {
                "type": "object",
                "properties": {
                    "metadata": {
                        "type": "object",
                        "properties": {
                            "name": {
                                "type": "string",
                                "description": "Schema name, format: {Kind}.{group} (e.g. Widget.example.io)"
                            }
                        },
                        "required": ["name"]
                    }
                },
                "required": ["metadata"]
            },
            { "$ref": "#/components/schemas/SchemaData" }
        ]
    })
}

/// Returns the static paths for Schema CRUD operations.
///
/// These paths are always present and operate on the `kapi.io/v1/Schema` kind:
/// - GET /apis/kapi.io/v1/Schema — list all registered schemas
/// - POST /apis/kapi.io/v1/Schema — register a new schema
/// - GET /apis/kapi.io/v1/Schema/{name} — get a specific schema
/// - DELETE /apis/kapi.io/v1/Schema/{name} — delete a schema
fn build_static_paths() -> Vec<(String, Value)> {
    let schema_error_ref = json!({ "$ref": "#/components/schemas/AppError" });
    let stored_object_ref = json!({ "$ref": "#/components/schemas/StoredObject" });
    let list_response_ref = json!({ "$ref": "#/components/schemas/ListResponse" });

    vec![
        // Combined GET+POST for /apis/kapi.io/v1/Schema
        ("/apis/kapi.io/v1/Schema".to_string(), json!({
            "get": {
                "summary": "List all registered Schema objects",
                "operationId": "listSchemas",
                "parameters": [],
                "responses": {
                    "200": {
                        "description": "A list of Schema objects",
                        "content": {
                            "application/json": {
                                "schema": list_response_ref
                            }
                        }
                    }
                }
            },
            "post": {
                "summary": "Register a new Schema",
                "operationId": "createSchema",
                "parameters": [],
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": schema_create_request_schema()
                        }
                    }
                },
                "responses": {
                    "201": {
                        "description": "Schema created successfully",
                        "content": {
                            "application/json": {
                                "schema": stored_object_ref
                            }
                        }
                    },
                    "404": {
                        "description": "Not found",
                        "content": { "application/json": { "schema": schema_error_ref } }
                    },
                    "409": {
                        "description": "Conflict — duplicate schema",
                        "content": { "application/json": { "schema": schema_error_ref } }
                    },
                    "422": {
                        "description": "Invalid schema — meta-schema validation failure",
                        "content": { "application/json": { "schema": schema_error_ref } }
                    }
                }
            }
        })),
        // Combined GET+DELETE for /apis/kapi.io/v1/Schema/{name}
        ("/apis/kapi.io/v1/Schema/{name}".to_string(), json!({
            "get": {
                "summary": "Get a Schema by name",
                "operationId": "getSchema",
                "parameters": [
                    {
                        "name": "name",
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string" },
                        "description": "The schema name (e.g. Widget.example.io)"
                    }
                ],
                "responses": {
                    "200": {
                        "description": "The Schema object",
                        "content": {
                            "application/json": {
                                "schema": stored_object_ref
                            }
                        }
                    },
                    "404": {
                        "description": "Schema not found",
                        "content": { "application/json": { "schema": schema_error_ref } }
                    }
                }
            },
            "delete": {
                "summary": "Delete a Schema by name",
                "operationId": "deleteSchema",
                "parameters": [
                    {
                        "name": "name",
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string" },
                        "description": "The schema name (e.g. Widget.example.io)"
                    }
                ],
                "responses": {
                    "200": {
                        "description": "Schema deleted successfully",
                        "content": {
                            "application/json": {
                                "schema": stored_object_ref
                            }
                        }
                    },
                    "404": {
                        "description": "Schema not found",
                        "content": { "application/json": { "schema": schema_error_ref } }
                    },
                    "409": {
                        "description": "Conflict — schema has existing objects of the target kind",
                        "content": { "application/json": { "schema": schema_error_ref } }
                    }
                }
            }
        })),
    ]
}

/// Builds the data component schema for a registered kind.
///
/// Wraps the user's `jsonSchema` as an OpenAPI Schema Object.
/// Returns `(component_name, schema_object)`.
fn build_kind_data_component(schema_data: &SchemaData, comp_name: &str) -> (String, Value) {
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
/// Mirrors the wire format: `{ key, metadata, data }` where `data` references
/// the kind-specific data component.
/// Returns `(component_name, schema_object)` — the name is `{comp_name}StoredObject`.
fn build_kind_stored_object_component(comp_name: &str) -> (String, Value) {
    let stored_name = format!("{comp_name}StoredObject");
    let schema = json!({
        "type": "object",
        "properties": {
            "key": { "$ref": "#/components/schemas/ResourceKey" },
            "metadata": { "$ref": "#/components/schemas/ObjectMetadata" },
            "data": { "$ref": format!("#/components/schemas/{comp_name}") }
        },
        "required": ["key", "metadata", "data"]
    });
    (stored_name, schema)
}

/// Builds the ListResponse component for a registered kind.
///
/// Contains `items` array referencing the kind-specific StoredObject and
/// an optional `continueToken` for pagination.
/// Returns `(component_name, schema_object)` — the name is `{comp_name}ListResponse`.
/// Builds dynamic OpenAPI paths for a registered kind.
///
/// Generates two concrete path entries (GVK is baked into the URL, so no
/// group/version/kind path parameters are needed):
/// - `GET/POST /apis/{group}/{version}/{kind}` — collection (list + create)
/// - `GET/PUT/DELETE /apis/{group}/{version}/{kind}/{name}` — item (get + update + delete)
///
/// Only `name` on item paths needs a path parameter. The list GET also
/// documents the optional `?watch=true` query parameter.
fn build_kind_paths(schema_data: &SchemaData, comp_name: &str) -> Vec<(String, Value)> {
    let group = &schema_data.target_group;
    let version = &schema_data.target_version;
    let kind = &schema_data.target_kind;

    let collection_path = format!("/apis/{group}/{version}/{kind}");
    let item_path = format!("/apis/{group}/{version}/{kind}/{{name}}");

    let stored_ref = json!({ "$ref": format!("#/components/schemas/{comp_name}StoredObject") });
    let list_ref = json!({ "$ref": format!("#/components/schemas/{comp_name}ListResponse") });
    let error_ref = json!({ "$ref": "#/components/schemas/AppError" });

    vec![
        // Collection path: GET (list) + POST (create)
        (collection_path, json!({
            "get": {
                "summary": format!("List {} objects", kind),
                "operationId": format!("list{}", comp_name),
                "parameters": [
                    {
                        "name": "watch",
                        "in": "query",
                        "required": false,
                        "schema": { "type": "boolean" },
                        "description": "Enable SSE watch stream"
                    }
                ],
                "responses": {
                    "200": {
                        "description": format!("A list of {} objects", kind),
                        "content": {
                            "application/json": {
                                "schema": list_ref
                            }
                        }
                    }
                }
            },
            "post": {
                "summary": format!("Create a new {} object", kind),
                "operationId": format!("create{}", comp_name),
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": build_create_request_schema(schema_data)
                        }
                    }
                },
                "responses": {
                    "201": {
                        "description": format!("{} object created", kind),
                        "content": {
                            "application/json": {
                                "schema": stored_ref
                            }
                        }
                    },
                    "404": {
                        "description": "Schema not found for this kind",
                        "content": { "application/json": { "schema": error_ref } }
                    },
                    "409": {
                        "description": "Conflict — object with same name already exists",
                        "content": { "application/json": { "schema": error_ref } }
                    },
                    "422": {
                        "description": "Schema validation failed",
                        "content": { "application/json": { "schema": error_ref } }
                    }
                }
            }
        })),
        // Item path: GET (get) + PUT (update) + DELETE (delete)
        (item_path, json!({
            "get": {
                "summary": format!("Get a {} object by name", kind),
                "operationId": format!("get{}", comp_name),
                "parameters": [
                    {
                        "name": "name",
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string" },
                        "description": "The object name"
                    }
                ],
                "responses": {
                    "200": {
                        "description": format!("The {} object", kind),
                        "content": {
                            "application/json": {
                                "schema": stored_ref
                            }
                        }
                    },
                    "404": {
                        "description": format!("{} object not found", kind),
                        "content": { "application/json": { "schema": error_ref } }
                    }
                }
            },
            "put": {
                "summary": format!("Update a {} object", kind),
                "operationId": format!("update{}", comp_name),
                "parameters": [
                    {
                        "name": "name",
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string" },
                        "description": "The object name"
                    }
                ],
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": stored_ref
                        }
                    }
                },
                "responses": {
                    "200": {
                        "description": format!("{} object updated", kind),
                        "content": {
                            "application/json": {
                                "schema": stored_ref
                            }
                        }
                    },
                    "404": {
                        "description": format!("{} object not found", kind),
                        "content": { "application/json": { "schema": error_ref } }
                    },
                    "409": {
                        "description": "Conflict — version mismatch",
                        "content": { "application/json": { "schema": error_ref } }
                    },
                    "422": {
                        "description": "Schema validation failed",
                        "content": { "application/json": { "schema": error_ref } }
                    }
                }
            },
            "delete": {
                "summary": format!("Delete a {} object", kind),
                "operationId": format!("delete{}", comp_name),
                "parameters": [
                    {
                        "name": "name",
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string" },
                        "description": "The object name"
                    }
                ],
                "responses": {
                    "200": {
                        "description": format!("{} object deleted", kind),
                        "content": {
                            "application/json": {
                                "schema": stored_ref
                            }
                        }
                    },
                    "404": {
                        "description": format!("{} object not found", kind),
                        "content": { "application/json": { "schema": error_ref } }
                    },
                    "409": {
                        "description": "Conflict — schema has objects of this kind",
                        "content": { "application/json": { "schema": error_ref } }
                    }
                }
            }
        })),
    ]
}

fn build_kind_list_response_component(comp_name: &str) -> (String, Value) {
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

/// Builds the request body schema for creating an object of a registered kind.
///
/// The actual wire format is: `{ metadata: { name }, ...userDataProperties }`.
/// The handler extracts `metadata.name` and passes the remaining properties
/// as the data payload for validation against the registered schema.
fn build_create_request_schema(schema_data: &SchemaData) -> Value {
    let metadata_part = json!({
        "type": "object",
        "properties": {
            "metadata": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Object name, unique within this kind" }
                },
                "required": ["name"]
            }
        },
        "required": ["metadata"]
    });

    json!({
        "allOf": [
            metadata_part,
            schema_data.json_schema
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_name_splits_dots_and_pascal_cases() {
        assert_eq!(component_name("Widget.example.io"), "WidgetExampleIo");
    }

    #[test]
    fn component_name_multi_segment_group() {
        assert_eq!(component_name("Deployment.apps"), "DeploymentApps");
    }

    #[test]
    fn component_name_same_kind_different_group_no_collision() {
        assert_eq!(component_name("Widget.example.io"), "WidgetExampleIo");
        assert_eq!(component_name("Widget.other.io"), "WidgetOtherIo");
        assert_ne!(
            component_name("Widget.example.io"),
            component_name("Widget.other.io"),
        );
    }

    #[test]
    fn build_static_components_contains_all_ten() {
        let components = build_static_components();
        let names: Vec<&str> = components.iter().map(|(n, _)| n.as_str()).collect();
        let expected = [
            "ResourceKey",
            "ObjectMetadata",
            "UserData",
            "StoredObject",
            "ListResponse",
            "WatchEvent",
            "WatchEventType",
            "ValidationError",
            "AppError",
            "SchemaData",
        ];
        for name in &expected {
            assert!(names.contains(name), "Missing component: {name}");
        }
        assert_eq!(names.len(), expected.len(), "Unexpected component count");
    }

    #[test]
    fn build_static_components_stored_object_shape() {
        let components = build_static_components();
        let stored = components.iter().find(|(n, _)| n == "StoredObject").unwrap();
        let obj = stored.1.as_object().unwrap();
        assert_eq!(obj["type"], "object");
        let props = obj["properties"].as_object().unwrap();
        assert!(props.contains_key("key"));
        assert!(props.contains_key("metadata"));
        assert!(props.contains_key("data"));
        assert_eq!(props["key"]["$ref"], "#/components/schemas/ResourceKey");
        let required = obj["required"].as_array().unwrap();
        assert!(required.iter().any(|r| r == "key"));
    }

    #[test]
    fn build_static_components_app_error_shape() {
        let components = build_static_components();
        let err = components.iter().find(|(n, _)| n == "AppError").unwrap();
        let obj = err.1.as_object().unwrap();
        assert_eq!(obj["type"], "object");
        let props = obj["properties"].as_object().unwrap();
        assert!(props.contains_key("error"));
        assert!(props.contains_key("code"));
        assert!(props.contains_key("details"));
        let required = obj["required"].as_array().unwrap();
        assert!(required.iter().any(|r| r == "error"));
        assert!(required.iter().any(|r| r == "code"));
    }

    #[test]
    fn build_static_components_watch_event_type_enum() {
        let components = build_static_components();
        let wet = components.iter().find(|(n, _)| n == "WatchEventType").unwrap();
        let obj = wet.1.as_object().unwrap();
        assert_eq!(obj["type"], "string");
        let variants: Vec<&str> = obj["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(variants.contains(&"Added"));
        assert!(variants.contains(&"Modified"));
        assert!(variants.contains(&"Deleted"));
    }

    #[test]
    fn build_kind_data_component_wraps_user_schema() {
        let schema_data = SchemaData {
            target_group: "example.io".to_string(),
            target_version: "v1".to_string(),
            target_kind: "Widget".to_string(),
            json_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "color": { "type": "string" },
                    "size": { "type": "integer" }
                }
            }),
        };
        let (name, schema) = build_kind_data_component(&schema_data, "WidgetExampleIo");
        assert_eq!(name, "WidgetExampleIo");
        let obj = schema.as_object().unwrap();
        assert_eq!(obj["type"], "object");
        let props = obj["properties"].as_object().unwrap();
        assert!(props.contains_key("value"));
        let value_schema = &props["value"];
        assert_eq!(value_schema["type"], "object");
        assert!(value_schema["properties"]["color"].as_object().is_some());
        assert_eq!(value_schema["properties"]["color"]["type"], "string");
        assert_eq!(value_schema["properties"]["size"]["type"], "integer");
    }

    #[test]
    fn build_kind_stored_object_component_has_correct_refs() {
        let (name, schema) = build_kind_stored_object_component("WidgetExampleIo");
        assert_eq!(name, "WidgetExampleIoStoredObject");
        let obj = schema.as_object().unwrap();
        assert_eq!(obj["type"], "object");
        let props = obj["properties"].as_object().unwrap();
        assert_eq!(props["key"]["$ref"], "#/components/schemas/ResourceKey");
        assert_eq!(props["metadata"]["$ref"], "#/components/schemas/ObjectMetadata");
        assert_eq!(props["data"]["$ref"], "#/components/schemas/WidgetExampleIo");
        let required = obj["required"].as_array().unwrap();
        assert!(required.iter().any(|r| r == "key"));
        assert!(required.iter().any(|r| r == "metadata"));
        assert!(required.iter().any(|r| r == "data"));
    }

    #[test]
    fn build_kind_list_response_component_has_items_array_with_ref() {
        let (name, schema) = build_kind_list_response_component("WidgetExampleIo");
        assert_eq!(name, "WidgetExampleIoListResponse");
        let obj = schema.as_object().unwrap();
        assert_eq!(obj["type"], "object");
        let items = &obj["properties"]["items"];
        assert_eq!(items["type"], "array");
        assert_eq!(
            items["items"]["$ref"],
            "#/components/schemas/WidgetExampleIoStoredObject"
        );
        let required = obj["required"].as_array().unwrap();
        assert!(required.iter().any(|r| r == "items"));
        assert!(obj["properties"]["continueToken"].as_object().is_some());
    }

    #[test]
    fn build_static_paths_contains_schema_crud() {
        let paths = build_static_paths();
        let path_map: std::collections::HashMap<&str, &Value> =
            paths.iter().map(|(p, v)| (p.as_str(), v)).collect();

        let collection = path_map.get("/apis/kapi.io/v1/Schema").unwrap();
        assert!(collection.get("get").is_some(), "missing GET collection");
        assert!(collection.get("post").is_some(), "missing POST collection");

        let item = path_map.get("/apis/kapi.io/v1/Schema/{name}").unwrap();
        assert!(item.get("get").is_some(), "missing GET item");
        assert!(item.get("delete").is_some(), "missing DELETE item");

        let get_params = item["get"]["parameters"].as_array().unwrap();
        assert!(get_params.iter().any(|p| p["name"] == "name"));
    }

    #[test]
    fn build_static_paths_post_has_request_body_with_metadata() {
        let paths = build_static_paths();
        let (_path, collection) = paths.iter().find(|(p, _)| p == "/apis/kapi.io/v1/Schema").unwrap();
        let rb = &collection["post"]["requestBody"];
        assert_eq!(rb["required"], true);
        let schema = &rb["content"]["application/json"]["schema"];
        assert!(schema["allOf"].is_array(), "should use allOf to compose metadata + SchemaData");
        assert_eq!(schema["allOf"][0]["required"][0], "metadata");
        assert_eq!(schema["allOf"][1]["$ref"], "#/components/schemas/SchemaData");
    }

    #[test]
    fn build_static_paths_post_has_error_responses() {
        let paths = build_static_paths();
        let (_path, collection) = paths.iter().find(|(p, _)| p == "/apis/kapi.io/v1/Schema").unwrap();
        let responses = &collection["post"]["responses"];
        assert!(responses.get("201").is_some());
        assert!(responses.get("404").is_some());
        assert!(responses.get("409").is_some());
        assert!(responses.get("422").is_some());
    }

    #[test]
    fn build_kind_paths_has_collection_and_item() {
        let schema_data = SchemaData {
            target_group: "example.io".to_string(),
            target_version: "v1".to_string(),
            target_kind: "Widget".to_string(),
            json_schema: serde_json::json!({ "type": "object" }),
        };
        let paths = build_kind_paths(&schema_data, "WidgetExampleIo");
        let path_map: std::collections::HashMap<&str, &Value> =
            paths.iter().map(|(p, v)| (p.as_str(), v)).collect();

        let collection = path_map.get("/apis/example.io/v1/Widget").unwrap();
        assert!(collection.get("get").is_some(), "missing GET collection");
        assert!(collection.get("post").is_some(), "missing POST collection");

        let item = path_map.get("/apis/example.io/v1/Widget/{name}").unwrap();
        assert!(item.get("get").is_some(), "missing GET item");
        assert!(item.get("put").is_some(), "missing PUT item");
        assert!(item.get("delete").is_some(), "missing DELETE item");
    }

    #[test]
    fn build_kind_paths_list_has_watch_param() {
        let schema_data = SchemaData {
            target_group: "example.io".to_string(),
            target_version: "v1".to_string(),
            target_kind: "Widget".to_string(),
            json_schema: serde_json::json!({ "type": "object" }),
        };
        let paths = build_kind_paths(&schema_data, "WidgetExampleIo");
        let (_path, collection) = paths.iter().find(|(p, _)| p == "/apis/example.io/v1/Widget").unwrap();
        let params = collection["get"]["parameters"].as_array().unwrap();
        let watch = params.iter().find(|p| p["name"] == "watch").unwrap();
        assert_eq!(watch["in"], "query");
        assert_eq!(watch["schema"]["type"], "boolean");
        assert_eq!(watch["required"], false);
    }

    #[test]
    fn build_kind_paths_post_has_201_and_errors() {
        let schema_data = SchemaData {
            target_group: "example.io".to_string(),
            target_version: "v1".to_string(),
            target_kind: "Widget".to_string(),
            json_schema: serde_json::json!({ "type": "object" }),
        };
        let paths = build_kind_paths(&schema_data, "WidgetExampleIo");
        let (_path, collection) = paths.iter().find(|(p, _)| p == "/apis/example.io/v1/Widget").unwrap();
        let responses = &collection["post"]["responses"];
        assert!(responses.get("201").is_some());
        assert!(responses.get("404").is_some());
        assert!(responses.get("409").is_some());
        assert!(responses.get("422").is_some());
    }

    #[test]
    fn build_kind_paths_item_only_has_name_param() {
        let schema_data = SchemaData {
            target_group: "example.io".to_string(),
            target_version: "v1".to_string(),
            target_kind: "Widget".to_string(),
            json_schema: serde_json::json!({ "type": "object" }),
        };
        let paths = build_kind_paths(&schema_data, "WidgetExampleIo");
        let (_path, item) = paths.iter().find(|(p, _)| p == "/apis/example.io/v1/Widget/{name}").unwrap();
        let params = item["get"]["parameters"].as_array().unwrap();
        let names: Vec<&str> = params.iter().map(|p| p["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["name"], "only name param should be present, GVK is in the URL");
    }

    #[tokio::test]
    async fn build_openapi_spec_includes_dynamic_paths_and_components() {
        let service = make_test_service();
        let schema_key = ResourceKey {
            group: "kapi.io".to_string(),
            version: "v1".to_string(),
            kind: "Schema".to_string(),
        };
        let schema_data = serde_json::json!({
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
        service
            .create(schema_key, "Widget.example.io".to_string(), schema_data)
            .await
            .unwrap();

        let spec = build_openapi_spec(&service).await.unwrap();
        let paths = spec["paths"].as_object().unwrap();
        assert!(
            paths.contains_key("/apis/example.io/v1/Widget"),
            "missing collection path"
        );
        assert!(
            paths.contains_key("/apis/example.io/v1/Widget/{name}"),
            "missing item path"
        );
        let schemas = spec["components"]["schemas"].as_object().unwrap();
        assert!(schemas.contains_key("WidgetExampleIo"), "missing data component");
        assert!(schemas.contains_key("WidgetExampleIoStoredObject"), "missing stored component");
        assert!(schemas.contains_key("WidgetExampleIoListResponse"), "missing list component");
    }

    #[tokio::test]
    async fn build_openapi_spec_reflects_mutations() {
        let service = make_test_service();
        let schema_key = ResourceKey {
            group: "kapi.io".to_string(),
            version: "v1".to_string(),
            kind: "Schema".to_string(),
        };
        let schema_data = serde_json::json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "jsonSchema": { "type": "object" }
        });

        // Register schema → build spec → verify paths exist
        service
            .create(schema_key.clone(), "Widget.example.io".to_string(), schema_data.clone())
            .await
            .unwrap();
        let spec_after_create = build_openapi_spec(&service).await.unwrap();
        assert!(spec_after_create["paths"]
            .as_object()
            .unwrap()
            .contains_key("/apis/example.io/v1/Widget"));

        // Delete schema → build spec → verify paths removed
        service
            .delete(schema_key, "Widget.example.io".to_string())
            .await
            .unwrap();
        let spec_after_delete = build_openapi_spec(&service).await.unwrap();
        assert!(
            !spec_after_delete["paths"]
                .as_object()
                .unwrap()
                .contains_key("/apis/example.io/v1/Widget"),
            "paths should be removed after schema deletion"
        );
    }

    /// Helper to create an ObjectService for testing with a fresh store and event bus.
    fn make_test_service() -> ObjectService {
        let store: std::sync::Arc<dyn crate::store::ObjectStore> =
            std::sync::Arc::new(crate::store::memory::InMemoryStore::new());
        let event_bus = crate::event::EventBus::default();
        let meta_validator = crate::schema::meta_schema::compile_meta_schema()
            .expect("meta-schema should compile");
        ObjectService::new(store, event_bus, meta_validator)
    }
}
