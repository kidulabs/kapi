//! Static and dynamic path builders for OpenAPI 3.0.3 spec generation.
//!
//! Defines static paths for Schema CRUD operations and dynamically
//! generates per-kind paths from registered Schema objects. Also contains
//! the top-level [`build_openapi_spec`] orchestrator that assembles the
//! full OpenAPI document from components, paths, and dynamic content.

use serde_json::{Value, json};

use crate::error::AppError;
use crate::object::service::ObjectService;
use crate::object::types::ListOptions;
use crate::openapi::components::{
    build_kind_list_response_component, build_kind_spec_component,
    build_kind_stored_object_component, build_static_components, component_name,
};
use crate::schema::schema_key;
use crate::store::ResourceKey;

/// Returns the ResourceKey for Schema objects stored under the kapi.io group.
fn schema_resource_key() -> ResourceKey {
    schema_key()
}

/// Builds a complete OpenAPI 3.0.3 document.
///
/// Queries the store for registered Schema objects and generates:
/// - Static components/schemas for kapi built-in types
/// - Static paths for Schema CRUD (/apis/kapi.io/v1/Schema)
/// - Dynamic per-kind paths and component schemas for each registered Schema
pub(crate) async fn build_openapi_spec(service: &ObjectService) -> Result<Value, AppError> {
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
        .list(
            schema_resource_key(),
            ListOptions {
                limit: None,
                continue_token: None,
                ..Default::default()
            },
        )
        .await?;

    // Parse each StoredObject's data field into SchemaData and generate dynamic content
    for item in &schema_list.items {
        let schema_data: crate::object::types::SchemaData =
            match serde_json::from_value(item.spec.clone()) {
                Ok(sd) => sd,
                Err(_) => continue,
            };

        let schema_name = format!("{}.{}", schema_data.target_kind, schema_data.target_group);
        let comp_name = component_name(&schema_name);

        // Generate the three component schemas for this kind
        let (spec_name, spec_schema) = build_kind_spec_component(&schema_data, &comp_name);
        all_schemas.insert(spec_name, spec_schema);

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
            "description": "Dynamic schema-driven API server"
        },
        "paths": all_paths,
        "components": {
            "schemas": all_schemas
        }
    });

    Ok(document)
}

/// Returns the static paths for Schema CRUD operations.
///
/// These paths are always present and operate on the `kapi.io/v1/Schema` kind:
/// - GET /apis/kapi.io/v1/Schema — list all registered schemas
/// - POST /apis/kapi.io/v1/Schema — register a new schema
/// - GET /apis/kapi.io/v1/Schema/{name} — get a specific schema
/// - DELETE /apis/kapi.io/v1/Schema/{name} — delete a schema
pub(crate) fn build_static_paths() -> Vec<(String, Value)> {
    let schema_error_ref = json!({ "$ref": "#/components/schemas/AppError" });
    let stored_object_ref = json!({ "$ref": "#/components/schemas/StoredObject" });
    let list_response_ref = json!({ "$ref": "#/components/schemas/ListResponse" });

    vec![
        // Combined GET+POST for /apis/kapi.io/v1/Schema
        (
            "/apis/kapi.io/v1/Schema".to_string(),
            json!({
                "get": {
                    "summary": "List all registered Schema objects",
                    "operationId": "listSchemas",
                    "parameters": [
                        {
                            "name": "fieldSelector",
                            "in": "query",
                            "required": false,
                            "schema": { "type": "string" },
                            "description": "Filter results by field selector (e.g., metadata.name=my-obj). On list requests, filters the returned objects. On watch requests, filters the event stream."
                        },
                        {
                            "name": "labelSelector",
                            "in": "query",
                            "required": false,
                            "schema": { "type": "string" },
                            "description": "Filter results by label selector. Supports: key=value (equality), key!=value (inequality), key (existence), !key (non-existence), comma-separated (AND). On list requests, filters the returned objects. On watch requests, filters the event stream. When both fieldSelector and labelSelector are present on watch, they are combined with AND semantics."
                        }
                    ],
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
                            "description": "AlreadyExists — duplicate schema",
                            "content": { "application/json": { "schema": schema_error_ref } }
                        },
                        "422": {
                            "description": "Invalid schema — meta-schema validation failure",
                            "content": { "application/json": { "schema": schema_error_ref } }
                        }
                    }
                }
            }),
        ),
        // Combined GET+DELETE for /apis/kapi.io/v1/Schema/{name}
        (
            "/apis/kapi.io/v1/Schema/{name}".to_string(),
            json!({
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
            }),
        ),
    ]
}

/// Builds the request body schema for creating a Schema object.
///
/// Schema name is auto-generated as `{targetKind}.{targetGroup}` by the handler.
/// The client does not need to supply `metadata.name`, but can supply `metadata.labels`.
pub(crate) fn schema_create_request_schema() -> Value {
    let metadata_part = json!({
        "type": "object",
        "properties": {
            "metadata": {
                "type": "object",
                "properties": {
                    "labels": {
                        "type": "object",
                        "additionalProperties": { "type": "string" },
                        "description": "Key-value labels for organizing and selecting objects"
                    },
                    "annotations": {
                        "type": "object",
                        "additionalProperties": { "type": "string" },
                        "description": "Arbitrary key-value metadata (non-queryable)"
                    }
                }
            }
        }
    });

    json!({
        "allOf": [
            metadata_part,
            { "$ref": "#/components/schemas/SchemaData" }
        ]
    })
}

/// Builds dynamic OpenAPI paths for a registered kind.
///
/// Generates two concrete path entries (GVK is baked into the URL, so no
/// group/version/kind path parameters are needed):
/// - `GET/POST /apis/{group}/{version}/{kind}` — collection (list + create)
/// - `GET/PUT/DELETE /apis/{group}/{version}/{kind}/{name}` — item (get + update + delete)
///
/// Only `name` on item paths needs a path parameter. The list GET also
/// documents the optional `?watch=true` query parameter.
pub(crate) fn build_kind_paths(
    schema_data: &crate::object::types::SchemaData,
    comp_name: &str,
) -> Vec<(String, Value)> {
    let group = &schema_data.target_group;
    let version = &schema_data.target_version;
    let kind = &schema_data.target_kind;

    let collection_path = format!("/apis/{group}/{version}/{kind}");
    let item_path = format!("/apis/{group}/{version}/{kind}/{{name}}");
    let status_path = format!("/apis/{group}/{version}/{kind}/{{name}}/status");

    let stored_ref = json!({ "$ref": format!("#/components/schemas/{comp_name}StoredObject") });
    let list_ref = json!({ "$ref": format!("#/components/schemas/{comp_name}ListResponse") });
    let error_ref = json!({ "$ref": "#/components/schemas/AppError" });
    let status_ref =
        json!({ "nullable": true, "description": "Status subresource, or null if not set" });

    vec![
        // Collection path: GET (list) + POST (create)
        (
            collection_path,
            json!({
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
                        },
                        {
                            "name": "fieldSelector",
                            "in": "query",
                            "required": false,
                            "schema": { "type": "string" },
                            "description": "Filter results by field selector (e.g., metadata.name=my-obj). On list requests, filters the returned objects. On watch requests, filters the event stream."
                        },
                        {
                            "name": "labelSelector",
                            "in": "query",
                            "required": false,
                            "schema": { "type": "string" },
                            "description": "Filter results by label selector. Supports: key=value (equality), key!=value (inequality), key (existence), !key (non-existence), comma-separated (AND). On list requests, filters the returned objects. On watch requests, filters the event stream. When both fieldSelector and labelSelector are present on watch, they are combined with AND semantics."
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
                        },
                        "400": {
                            "description": "Invalid field selector — unsupported field or malformed syntax. Invalid label selector — malformed syntax.",
                            "content": { "application/json": { "schema": error_ref } }
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
                            "description": "AlreadyExists — object with same name already exists",
                            "content": { "application/json": { "schema": error_ref } }
                        },
                        "422": {
                            "description": "Schema validation failed",
                            "content": { "application/json": { "schema": error_ref } }
                        }
                    }
                }
            }),
        ),
        // Item path: GET (get) + PUT (update) + DELETE (delete)
        (
            item_path,
            json!({
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
            }),
        ),
        // Status subresource path: GET (get status) + PUT (update status)
        (
            status_path,
            json!({
                "get": {
                    "summary": format!("Get the status subresource of a {} object", kind),
                    "operationId": format!("get{}Status", comp_name),
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
                            "description": format!("The status of the {} object (null if not set)", kind),
                            "content": {
                                "application/json": {
                                    "schema": status_ref
                                }
                            }
                        },
                        "404": {
                            "description": format!("{} object not found or status subresource not enabled for this kind", kind),
                            "content": { "application/json": { "schema": error_ref } }
                        }
                    }
                },
                "put": {
                    "summary": format!("Update the status subresource of a {} object", kind),
                    "operationId": format!("update{}Status", comp_name),
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
                                "schema": build_status_update_request_schema(schema_data)
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": format!("{} status updated", kind),
                            "content": {
                                "application/json": {
                                    "schema": stored_ref
                                }
                            }
                        },
                        "404": {
                            "description": format!("{} object not found or status subresource not enabled for this kind", kind),
                            "content": { "application/json": { "schema": error_ref } }
                        },
                        "422": {
                            "description": "Status validation failed against statusSchema",
                            "content": { "application/json": { "schema": error_ref } }
                        }
                    }
                }
            }),
        ),
    ]
}

/// Builds the request body schema for creating an object of a registered kind.
///
/// The wire format is: `{ metadata: { name, labels? }, spec: { ...userSchema } }`.
/// The handler extracts `metadata.name` and `metadata.labels`, validates that `spec`
/// is present, is a JSON object, and is non-empty. Unknown top-level fields are rejected.
fn build_create_request_schema(schema_data: &crate::object::types::SchemaData) -> Value {
    json!({
        "type": "object",
        "properties": {
            "metadata": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Object name, unique within this kind" },
                    "labels": {
                        "type": "object",
                        "additionalProperties": { "type": "string" },
                        "description": "Key-value labels for organizing and selecting objects"
                    },
                    "annotations": {
                        "type": "object",
                        "additionalProperties": { "type": "string" },
                        "description": "Arbitrary key-value metadata (non-queryable)"
                    }
                },
                "required": ["name"]
            },
            "spec": schema_data.spec_schema
        },
        "required": ["metadata", "spec"],
        "additionalProperties": false
    })
}

/// Builds the request body schema for updating the status subresource.
///
/// The wire format is `{ status: ...userDataProperties }`.
fn build_status_update_request_schema(schema_data: &crate::object::types::SchemaData) -> Value {
    let status_schema = schema_data
        .status_schema
        .clone()
        .unwrap_or_else(|| json!({ "type": "object" }));

    json!({
        "type": "object",
        "properties": {
            "status": status_schema
        },
        "required": ["status"]
    })
}
