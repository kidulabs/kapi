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
            None,
            ListOptions { limit: None, continue_token: None, ..Default::default() },
        )
        .await?;

    // Parse each StoredObject's data field into SchemaData and generate dynamic content
    for item in &schema_list.items {
        let schema_data: crate::object::types::SchemaData =
            match serde_json::from_value(item.spec.clone()) {
                Ok(sd) => sd,
                Err(_) => continue,
            };

        let schema_name = format!(
            "{}.{}.{}",
            schema_data.target_kind, schema_data.target_group, schema_data.target_version
        );
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
///
/// Schema is a cluster-scoped kind, so only cluster-scoped paths are generated.
pub(crate) fn build_static_paths() -> Vec<(String, Value)> {
    let schema_error_ref = json!({ "$ref": "#/components/schemas/AppError" });
    let stored_object_ref = json!({ "$ref": "#/components/schemas/StoredObject" });
    let list_response_ref = json!({ "$ref": "#/components/schemas/ListResponse" });

    // Example: list schemas response
    let schema_list_example = json!({
        "items": [
            {
                "key": { "group": "kapi.io", "version": "v1", "kind": "Schema" },
                "metadata": { "name": "Widget.example.io.v1", "labels": {}, "annotations": {} },
                "system": {
                    "resourceVersion": 1,
                    "generation": 1,
                    "createdAt": "2024-06-01T00:00:00Z",
                    "updatedAt": "2024-06-01T00:00:00Z"
                },
                "spec": {
                    "targetGroup": "example.io",
                    "targetVersion": "v1",
                    "targetKind": "Widget",
                    "scope": "Namespaced",
                    "specSchema": {
                        "type": "object",
                        "properties": {
                            "color": { "type": "string" },
                            "size": { "type": "integer" }
                        }
                    }
                }
            }
        ],
        "continueToken": null
    });

    // Example: create schema request
    let schema_create_example = json!({
        "metadata": {
            "labels": { "team": "backend" }
        },
        "targetGroup": "example.io",
        "targetVersion": "v1",
        "targetKind": "Widget",
        "scope": "Namespaced",
        "specSchema": {
            "type": "object",
            "properties": {
                "color": { "type": "string" },
                "size": { "type": "integer" }
            }
        }
    });

    // Example: get schema response
    let schema_get_example = json!({
        "key": { "group": "kapi.io", "version": "v1", "kind": "Schema" },
        "metadata": { "name": "Widget.example.io.v1", "labels": {}, "annotations": {} },
        "system": {
            "resourceVersion": 1,
            "generation": 1,
            "createdAt": "2024-06-01T00:00:00Z",
            "updatedAt": "2024-06-01T00:00:00Z"
        },
        "spec": {
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "scope": "Namespaced",
            "specSchema": {
                "type": "object",
                "properties": {
                    "color": { "type": "string" },
                    "size": { "type": "integer" }
                }
            }
        }
    });

    vec![
        // Combined GET+POST for /apis/kapi.io/v1/Schema
        (
            "/apis/kapi.io/v1/Schema".to_string(),
            json!({
                "get": {
                    "summary": "List all registered Schema objects",
                    "description": "Schema objects define the API surface. Each Schema registers a new kind (group/version/kind) and provides a JSON Schema for validating objects of that kind. Schemas are cluster-scoped resources.",
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
                                    "schema": list_response_ref,
                                    "example": schema_list_example
                                }
                            }
                        }
                    }
                },
                "post": {
                    "summary": "Register a new Schema",
                    "description": "Register a new API kind by providing target group, version, kind, and a JSON Schema for validating the spec. The schema name is auto-generated as `{targetKind}.{targetGroup}.{targetVersion}`.",
                    "operationId": "createSchema",
                    "parameters": [],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": schema_create_request_schema(),
                                "example": schema_create_example
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Schema created successfully",
                            "content": {
                                "application/json": {
                                    "schema": stored_object_ref,
                                    "example": schema_get_example
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
                            "description": "The schema name (e.g. Widget.example.io.v1)",
                            "example": "Widget.example.io.v1"
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "The Schema object",
                            "content": {
                                "application/json": {
                                    "schema": stored_object_ref,
                                    "example": schema_get_example
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
                            "description": "The schema name (e.g. Widget.example.io.v1)",
                            "example": "Widget.example.io.v1"
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
/// Schema name is auto-generated as `{targetKind}.{targetGroup}.{targetVersion}` by the handler.
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
/// For namespaced kinds (scope == "Namespaced"), also generates namespace-scoped variants:
/// - `GET/POST /apis/{group}/{version}/namespaces/{namespace}/{kind}` — namespace-scoped collection
/// - `GET/PUT/DELETE /apis/{group}/{version}/namespaces/{namespace}/{kind}/{name}` — namespace-scoped item
///
/// Only `name` on item paths and `namespace` on namespace-scoped paths need path parameters.
/// The list GET also documents the optional `?watch=true` query parameter.
///
/// Examples are added to operations to show realistic request/response bodies:
/// - Cluster-scoped list of a namespaced kind shows cross-namespace items (two objects from different namespaces)
/// - Namespace-scoped list shows items from a single namespace
/// - Create operations include example request bodies
pub(crate) fn build_kind_paths(
    schema_data: &crate::object::types::SchemaData,
    comp_name: &str,
) -> Vec<(String, Value)> {
    let group = &schema_data.target_group;
    let version = &schema_data.target_version;
    let kind = &schema_data.target_kind;
    let is_namespaced = schema_data.scope == crate::schema::SCOPE_NAMESPACED;

    // Reusable refs
    let stored_ref = json!({ "$ref": format!("#/components/schemas/{comp_name}StoredObject") });
    let list_ref = json!({ "$ref": format!("#/components/schemas/{comp_name}ListResponse") });
    let error_ref = json!({ "$ref": "#/components/schemas/AppError" });
    let status_ref =
        json!({ "nullable": true, "description": "Status subresource, or null if not set" });

    let name_param = build_name_param("The object name");

    let mut all_paths = Vec::new();

    // ============================================================
    // 1. Cluster-scoped collection path: GET (list) + POST (create)
    // ============================================================
    // Cross-namespace list example: shows objects from multiple namespaces
    let list_example_cross_ns = if is_namespaced {
        Some(build_list_response_example(schema_data, comp_name, true))
    } else {
        None
    };
    // Create example: shows a realistic request body
    let create_example = Some(build_create_request_example(schema_data));
    let get_example = Some(build_get_response_example(schema_data, comp_name, None));

    let cluster_collection_path = format!("/apis/{group}/{version}/{kind}");
    all_paths.push((
        cluster_collection_path,
        json!({
            "get": {
                "summary": format!("List {} objects{}", kind, if is_namespaced { " (cross-namespace)" } else { "" }),
                "description": if is_namespaced {
                    format!("Cross-namespace list of {} objects. Returns objects from all namespaces that the user has access to. Each object includes its `metadata.namespace` field indicating which namespace it belongs to.", kind)
                } else {
                    format!("List all {} objects.", kind)
                },
                "operationId": format!("list{}", comp_name),
                "parameters": build_list_parameters(),
                "responses": {
                    "200": {
                        "description": format!("A list of {} objects", kind),
                        "content": {
                            "application/json": {
                                "schema": list_ref,
                                "example": list_example_cross_ns
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
                "summary": format!("Create a new {} object (cluster-scoped)", kind),
                "description": if is_namespaced {
                    format!("Create a {} object. For namespaced kinds, the namespace is taken from `metadata.namespace` in the request body. Alternatively, use the namespace-scoped POST endpoint at `/apis/{group}/{version}/namespaces/{{namespace}}/{kind}`.", kind)
                } else {
                    format!("Create a new {} object.", kind)
                },
                "operationId": format!("create{}", comp_name),
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": build_create_request_schema(schema_data),
                            "example": create_example
                        }
                    }
                },
                "responses": {
                    "201": {
                        "description": format!("{} object created", kind),
                        "content": {
                            "application/json": {
                                "schema": stored_ref,
                                "example": get_example
                            }
                        }
                    },
                    "400": {
                        "description": "Bad request — missing required fields or validation failure",
                        "content": { "application/json": { "schema": error_ref } }
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
    ));

    // ============================================================
    // 2. Cluster-scoped item path: GET + PUT + DELETE
    // ============================================================
    let cluster_item_path = format!("/apis/{group}/{version}/{kind}/{{name}}");
    all_paths.push((
        cluster_item_path,
        json!({
            "get": {
                "summary": format!("Get a {} object by name", kind),
                "operationId": format!("get{}", comp_name),
                "parameters": [name_param.clone()],
                "responses": {
                    "200": {
                        "description": format!("The {} object", kind),
                        "content": {
                            "application/json": {
                                "schema": stored_ref,
                                "example": get_example
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
                "parameters": [name_param.clone()],
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": stored_ref,
                            "example": get_example
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
                "parameters": [name_param.clone()],
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
                        "description": "Conflict — object has finalizers or is being deleted",
                        "content": { "application/json": { "schema": error_ref } }
                    }
                }
            }
        }),
    ));

    // ============================================================
    // 3. Cluster-scoped status path: GET + PUT
    // ============================================================
    let cluster_status_path = format!("/apis/{group}/{version}/{kind}/{{name}}/status");
    all_paths.push((
        cluster_status_path,
        json!({
            "get": {
                "summary": format!("Get the status subresource of a {} object", kind),
                "operationId": format!("get{}Status", comp_name),
                "parameters": [name_param.clone()],
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
                "parameters": [name_param.clone()],
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
    ));

    // ============================================================
    // 4. Namespace-scoped paths (only for namespaced kinds)
    // ============================================================
    if is_namespaced {
        let namespace_param = build_namespace_param();
        let ns_name_param = build_name_param("The object name");
        let ns_get_example =
            Some(build_get_response_example(schema_data, comp_name, Some("production")));

        // Namespace-scoped collection
        let ns_collection_path = format!("/apis/{group}/{version}/namespaces/{{namespace}}/{kind}");
        let ns_list_example = Some(build_list_response_example(schema_data, comp_name, false));
        all_paths.push((
            ns_collection_path,
            json!({
                "get": {
                    "summary": format!("List {} objects in a namespace", kind),
                    "description": format!("Namespace-scoped list of {} objects. Returns only objects in the specified namespace.", kind),
                    "operationId": format!("list{}Namespaced", comp_name),
                    "parameters": [
                        namespace_param.clone(),
                        json!({
                            "name": "watch",
                            "in": "query",
                            "required": false,
                            "schema": { "type": "boolean" },
                            "description": "Enable SSE watch stream"
                        }),
                        json!({
                            "name": "fieldSelector",
                            "in": "query",
                            "required": false,
                            "schema": { "type": "string" },
                            "description": "Filter results by field selector (e.g., metadata.name=my-obj). On list requests, filters the returned objects. On watch requests, filters the event stream."
                        }),
                        json!({
                            "name": "labelSelector",
                            "in": "query",
                            "required": false,
                            "schema": { "type": "string" },
                            "description": "Filter results by label selector."
                        })
                    ],
                    "responses": {
                        "200": {
                            "description": format!("A list of {} objects in the namespace", kind),
                            "content": {
                                "application/json": {
                                    "schema": list_ref,
                                    "example": ns_list_example
                                }
                            }
                        },
                        "400": {
                            "description": "Invalid field selector or label selector",
                            "content": { "application/json": { "schema": error_ref } }
                        }
                    }
                },
                "post": {
                    "summary": format!("Create a new {} object in a namespace", kind),
                    "description": format!("Create a {} object in the specified namespace. The namespace is taken from the URL path parameter, not from the request body.", kind),
                    "operationId": format!("create{}Namespaced", comp_name),
                    "parameters": [namespace_param.clone()],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": build_create_request_schema(schema_data),
                                "example": create_example
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": format!("{} object created in namespace", kind),
                            "content": {
                                "application/json": {
                                    "schema": stored_ref,
                                    "example": ns_get_example
                                }
                            }
                        },
                        "400": {
                            "description": "Bad request — missing required fields or validation failure",
                            "content": { "application/json": { "schema": error_ref } }
                        },
                        "404": {
                            "description": "Schema not found for this kind",
                            "content": { "application/json": { "schema": error_ref } }
                        },
                        "409": {
                            "description": "AlreadyExists — object with same name already exists in this namespace",
                            "content": { "application/json": { "schema": error_ref } }
                        },
                        "422": {
                            "description": "Schema validation failed",
                            "content": { "application/json": { "schema": error_ref } }
                        }
                    }
                }
            }),
        ));

        // Namespace-scoped item
        let ns_item_path =
            format!("/apis/{group}/{version}/namespaces/{{namespace}}/{kind}/{{name}}");
        all_paths.push((
            ns_item_path,
            json!({
                "get": {
                    "summary": format!("Get a {} object by name in a namespace", kind),
                    "operationId": format!("get{}Namespaced", comp_name),
                    "parameters": [
                        namespace_param.clone(),
                        ns_name_param.clone()
                    ],
                    "responses": {
                        "200": {
                            "description": format!("The {} object in the namespace", kind),
                            "content": {
                                "application/json": {
                                    "schema": stored_ref,
                                    "example": ns_get_example
                                }
                            }
                        },
                        "404": {
                            "description": format!("{} object not found in this namespace", kind),
                            "content": { "application/json": { "schema": error_ref } }
                        }
                    }
                },
                "put": {
                    "summary": format!("Update a {} object in a namespace", kind),
                    "operationId": format!("update{}Namespaced", comp_name),
                    "parameters": [
                        namespace_param.clone(),
                        ns_name_param.clone()
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
                            "description": format!("{} object updated in namespace", kind),
                            "content": {
                                "application/json": {
                                    "schema": stored_ref
                                }
                            }
                        },
                        "404": {
                            "description": format!("{} object not found in this namespace", kind),
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
                    "summary": format!("Delete a {} object in a namespace", kind),
                    "operationId": format!("delete{}Namespaced", comp_name),
                    "parameters": [
                        namespace_param.clone(),
                        ns_name_param.clone()
                    ],
                    "responses": {
                        "200": {
                            "description": format!("{} object deleted in namespace", kind),
                            "content": {
                                "application/json": {
                                    "schema": stored_ref
                                }
                            }
                        },
                        "404": {
                            "description": format!("{} object not found in this namespace", kind),
                            "content": { "application/json": { "schema": error_ref } }
                        },
                        "409": {
                            "description": "Conflict — object has finalizers or is being deleted",
                            "content": { "application/json": { "schema": error_ref } }
                        }
                    }
                }
            }),
        ));

        // Namespace-scoped status subresource
        let ns_status_path =
            format!("/apis/{group}/{version}/namespaces/{{namespace}}/{kind}/{{name}}/status");
        all_paths.push((
            ns_status_path,
            json!({
                "get": {
                    "summary": format!("Get the status subresource of a {} object in a namespace", kind),
                    "operationId": format!("get{}StatusNamespaced", comp_name),
                    "parameters": [
                        namespace_param.clone(),
                        name_param.clone()
                    ],
                    "responses": {
                        "200": {
                            "description": format!("The status of the {} object in the namespace (null if not set)", kind),
                            "content": {
                                "application/json": {
                                    "schema": status_ref
                                }
                            }
                        },
                        "404": {
                            "description": format!("{} object not found in this namespace or status subresource not enabled for this kind", kind),
                            "content": { "application/json": { "schema": error_ref } }
                        }
                    }
                },
                "put": {
                    "summary": format!("Update the status subresource of a {} object in a namespace", kind),
                    "operationId": format!("update{}StatusNamespaced", comp_name),
                    "parameters": [
                        namespace_param.clone(),
                        name_param.clone()
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
                            "description": format!("{} status updated in namespace", kind),
                            "content": {
                                "application/json": {
                                    "schema": stored_ref
                                }
                            }
                        },
                        "404": {
                            "description": format!("{} object not found in this namespace or status subresource not enabled for this kind", kind),
                            "content": { "application/json": { "schema": error_ref } }
                        },
                        "422": {
                            "description": "Status validation failed against statusSchema",
                            "content": { "application/json": { "schema": error_ref } }
                        }
                    }
                }
            }),
        ));
    }

    all_paths
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
                    "namespace": {
                        "type": "string",
                        "description": "Optional namespace hint. For namespace-scoped routes, the namespace from the URL takes precedence. For cluster-scoped creates of a namespaced kind, this can be used to specify the namespace.",
                        "nullable": true
                    },
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

/// Builds an example request body for creating a namespaced object.
fn build_create_request_example(schema_data: &crate::object::types::SchemaData) -> Value {
    let example_spec = build_spec_example(&schema_data.spec_schema);
    json!({
        "metadata": {
            "name": "my-widget"
        },
        "spec": example_spec
    })
}

/// Builds an example response body for a created/get object.
fn build_get_response_example(
    schema_data: &crate::object::types::SchemaData,
    _comp_name: &str,
    namespace: Option<&str>,
) -> Value {
    let example_spec = build_spec_example(&schema_data.spec_schema);
    let namespace_value = match namespace {
        Some(ns) => json!(ns),
        None => Value::Null,
    };
    json!({
        "key": {
            "group": schema_data.target_group,
            "version": schema_data.target_version,
            "kind": schema_data.target_kind
        },
        "metadata": {
            "name": "my-widget",
            "namespace": namespace_value,
            "labels": { "app": "example", "env": "production" },
            "annotations": { "description": "An example widget" }
        },
        "system": {
            "resourceVersion": 1,
            "generation": 1,
            "createdAt": "2024-06-01T00:00:00Z",
            "updatedAt": "2024-06-01T00:00:00Z"
        },
        "spec": example_spec
    })
}

/// Builds an example list response body, optionally showing cross-namespace items.
fn build_list_response_example(
    schema_data: &crate::object::types::SchemaData,
    _comp_name: &str,
    cross_namespace: bool,
) -> Value {
    let example_spec = build_spec_example(&schema_data.spec_schema);

    if cross_namespace {
        json!({
            "items": [
                {
                    "key": { "group": schema_data.target_group, "version": schema_data.target_version, "kind": schema_data.target_kind },
                    "metadata": { "name": "widget-prod", "namespace": "production", "labels": {}, "annotations": {} },
                    "system": { "resourceVersion": 1, "generation": 1, "createdAt": "2024-06-01T00:00:00Z", "updatedAt": "2024-06-01T00:00:00Z" },
                    "spec": example_spec
                },
                {
                    "key": { "group": schema_data.target_group, "version": schema_data.target_version, "kind": schema_data.target_kind },
                    "metadata": { "name": "widget-staging", "namespace": "staging", "labels": {}, "annotations": {} },
                    "system": { "resourceVersion": 1, "generation": 1, "createdAt": "2024-06-01T00:00:00Z", "updatedAt": "2024-06-01T00:00:00Z" },
                    "spec": example_spec
                }
            ],
            "continueToken": null
        })
    } else {
        json!({
            "items": [
                {
                    "key": { "group": schema_data.target_group, "version": schema_data.target_version, "kind": schema_data.target_kind },
                    "metadata": { "name": "my-widget", "namespace": "production", "labels": {}, "annotations": {} },
                    "system": { "resourceVersion": 1, "generation": 1, "createdAt": "2024-06-01T00:00:00Z", "updatedAt": "2024-06-01T00:00:00Z" },
                    "spec": example_spec
                }
            ],
            "continueToken": null
        })
    }
}

/// Attempts to build a realistic example value from a JSON Schema.
/// Falls back to a generic object if schema analysis fails.
fn build_spec_example(schema: &serde_json::Value) -> serde_json::Value {
    match schema.get("properties") {
        Some(props) => {
            let mut example = serde_json::Map::new();
            if let Some(obj) = props.as_object() {
                for (key, prop) in obj.iter().take(3) {
                    let val = match prop.get("type").and_then(|t| t.as_str()) {
                        Some("string") => json!("string"),
                        Some("integer") => json!(42),
                        Some("number") => json!(1.5),
                        Some("boolean") => json!(true),
                        Some("array") => json!([]),
                        Some("object") => json!({}),
                        _ => json!("value"),
                    };
                    example.insert(key.clone(), val);
                }
            }
            json!(example)
        }
        None => json!({ "key": "value" }),
    }
}

/// Builds a standard set of parameters for a list operation: watch, fieldSelector, labelSelector.
fn build_list_parameters() -> Vec<Value> {
    vec![
        json!({
            "name": "watch",
            "in": "query",
            "required": false,
            "schema": { "type": "boolean" },
            "description": "Enable SSE watch stream"
        }),
        json!({
            "name": "fieldSelector",
            "in": "query",
            "required": false,
            "schema": { "type": "string" },
            "description": "Filter results by field selector (e.g., metadata.name=my-obj). On list requests, filters the returned objects. On watch requests, filters the event stream."
        }),
        json!({
            "name": "labelSelector",
            "in": "query",
            "required": false,
            "schema": { "type": "string" },
            "description": "Filter results by label selector. Supports: key=value (equality), key!=value (inequality), key (existence), !key (non-existence), comma-separated (AND). On list requests, filters the returned objects. On watch requests, filters the event stream. When both fieldSelector and labelSelector are present on watch, they are combined with AND semantics."
        }),
    ]
}

/// Builds standard name path parameter.
fn build_name_param(description: &str) -> Value {
    json!({
        "name": "name",
        "in": "path",
        "required": true,
        "schema": { "type": "string" },
        "description": description,
        "example": "my-widget"
    })
}

/// Builds standard namespace path parameter.
fn build_namespace_param() -> Value {
    json!({
        "name": "namespace",
        "in": "path",
        "required": true,
        "schema": { "type": "string" },
        "description": "The namespace of the object",
        "example": "production"
    })
}

/// Builds the request body schema for updating the status subresource.
///
/// The wire format is `{ status: ...userDataProperties }`.
fn build_status_update_request_schema(schema_data: &crate::object::types::SchemaData) -> Value {
    let status_schema =
        schema_data.status_schema.clone().unwrap_or_else(|| json!({ "type": "object" }));

    json!({
        "type": "object",
        "properties": {
            "status": status_schema
        },
        "required": ["status"]
    })
}
