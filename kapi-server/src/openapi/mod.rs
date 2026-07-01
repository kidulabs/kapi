//! Dynamic OpenAPI 3.0.3 spec generation.
//!
//! Builds an OpenAPI 3.0.3 document from scratch on every request by:
//! 1. Including static component schemas for kapi built-in types
//! 2. Including static paths for Schema CRUD operations
//! 3. Querying the store for registered Schema objects
//! 4. Generating per-kind paths and component schemas for each registered Schema
//!
//! # Module Structure
//!
//! - [`components`] — Static and dynamic component schema builders
//! - [`paths`] — Static and dynamic path builders + spec orchestrator
//! - [`swagger`] — Swagger UI HTML constant and handler

mod components;
mod paths;
mod swagger;

// Re-export public API to preserve compatibility with external consumers.
// Consumers continue to use `crate::openapi::component_name`, etc.
#[allow(unused_imports)]
pub use components::component_name;
pub use swagger::get_swagger_ui_handler;

use axum::Json;
use axum::extract::State;
use serde_json::Value;

use crate::error::AppError;
use crate::routes::AppState;

/// Handler for `GET /openapi`.
///
/// Calls `build_openapi_spec` to generate the OpenAPI document from the
/// current state of registered schemas, and returns it as JSON.
pub async fn get_openapi_handler(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let spec = self::paths::build_openapi_spec(state.object_service()).await?;
    Ok(Json(spec))
}

#[cfg(test)]
mod tests {
    use super::components::{
        build_kind_list_response_component, build_kind_spec_component,
        build_kind_stored_object_component, build_static_components, component_name,
    };
    use super::paths::{build_kind_paths, build_openapi_spec, build_static_paths};
    use serde_json::Value;
    use std::collections::HashMap;

    #[test]
    fn component_name_splits_dots_and_pascal_cases() {
        assert_eq!(component_name("Widget.example.io.v1"), "WidgetExampleIoV1");
    }

    #[test]
    fn component_name_multi_segment_group() {
        assert_eq!(component_name("Deployment.apps"), "DeploymentApps");
    }

    #[test]
    fn component_name_same_kind_different_group_no_collision() {
        assert_eq!(component_name("Widget.example.io.v1"), "WidgetExampleIoV1");
        assert_eq!(component_name("Widget.other.io"), "WidgetOtherIo");
        assert_ne!(component_name("Widget.example.io.v1"), component_name("Widget.other.io"),);
    }

    #[test]
    fn build_static_components_contains_all_eleven() {
        let components = build_static_components();
        let names: Vec<&str> = components.iter().map(|(n, _)| n.as_str()).collect();
        let expected = [
            "ResourceKey",
            "ObjectMeta",
            "SystemMetadata",
            "StoredObject",
            "ListResponse",
            "WatchEvent",
            "WatchEventType",
            "ValidationError",
            "InvalidFieldSelector",
            "InvalidFinalizer",
            "ObjectBeingDeleted",
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
        assert!(props.contains_key("system"));
        assert!(props.contains_key("spec"));
        assert_eq!(props["key"]["$ref"], "#/components/schemas/ResourceKey");
        assert_eq!(props["metadata"]["$ref"], "#/components/schemas/ObjectMeta");
        assert_eq!(props["system"]["$ref"], "#/components/schemas/SystemMetadata");
        let required = obj["required"].as_array().unwrap();
        assert!(required.iter().any(|r| r == "key"));
        assert!(required.iter().any(|r| r == "system"));
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
    fn build_static_components_invalid_field_selector_shape() {
        let components = build_static_components();
        let ifs = components.iter().find(|(n, _)| n == "InvalidFieldSelector").unwrap();
        let obj = ifs.1.as_object().unwrap();
        assert_eq!(obj["type"], "object");
        let props = obj["properties"].as_object().unwrap();
        assert!(props.contains_key("error"));
        assert!(props.contains_key("code"));
        assert!(props.contains_key("details"));
    }

    #[test]
    fn build_static_components_watch_event_type_enum() {
        let components = build_static_components();
        let wet = components.iter().find(|(n, _)| n == "WatchEventType").unwrap();
        let obj = wet.1.as_object().unwrap();
        assert_eq!(obj["type"], "string");
        let variants: Vec<&str> =
            obj["enum"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert!(variants.contains(&"Added"));
        assert!(variants.contains(&"Modified"));
        assert!(variants.contains(&"Deleted"));
    }

    #[test]
    fn build_kind_spec_component_uses_user_schema_directly() {
        let schema_data = crate::object::types::SchemaData {
            target_group: "example.io".to_string(),
            target_version: "v1".to_string(),
            target_kind: "Widget".to_string(),
            spec_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "color": { "type": "string" },
                    "size": { "type": "integer" }
                }
            }),
            status_schema: None,
            scope: "Namespaced".to_string(),
        };
        let (name, schema) = build_kind_spec_component(&schema_data, "WidgetExampleIo");
        assert_eq!(name, "WidgetExampleIo");
        let obj = schema.as_object().unwrap();
        assert_eq!(obj["type"], "object");
        // User fields appear at the top level (no `value` wrapper).
        let props = obj["properties"].as_object().unwrap();
        assert!(props.contains_key("color"));
        assert!(props.contains_key("size"));
        assert_eq!(props["color"]["type"], "string");
        assert_eq!(props["size"]["type"], "integer");
    }

    #[test]
    fn build_kind_stored_object_component_has_correct_refs() {
        let (name, schema) = build_kind_stored_object_component("WidgetExampleIo");
        assert_eq!(name, "WidgetExampleIoStoredObject");
        let obj = schema.as_object().unwrap();
        assert_eq!(obj["type"], "object");
        let props = obj["properties"].as_object().unwrap();
        assert_eq!(props["key"]["$ref"], "#/components/schemas/ResourceKey");
        assert_eq!(props["metadata"]["$ref"], "#/components/schemas/ObjectMeta");
        assert_eq!(props["system"]["$ref"], "#/components/schemas/SystemMetadata");
        assert_eq!(props["spec"]["$ref"], "#/components/schemas/WidgetExampleIo");
        let required = obj["required"].as_array().unwrap();
        assert!(required.iter().any(|r| r == "key"));
        assert!(required.iter().any(|r| r == "metadata"));
        assert!(required.iter().any(|r| r == "system"));
        assert!(required.iter().any(|r| r == "spec"));
    }

    #[test]
    fn build_kind_list_response_component_has_items_array_with_ref() {
        let (name, schema) = build_kind_list_response_component("WidgetExampleIo");
        assert_eq!(name, "WidgetExampleIoListResponse");
        let obj = schema.as_object().unwrap();
        assert_eq!(obj["type"], "object");
        let items = &obj["properties"]["items"];
        assert_eq!(items["type"], "array");
        assert_eq!(items["items"]["$ref"], "#/components/schemas/WidgetExampleIoStoredObject");
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
        let (_path, collection) =
            paths.iter().find(|(p, _)| p == "/apis/kapi.io/v1/Schema").unwrap();
        let rb = &collection["post"]["requestBody"];
        assert_eq!(rb["required"], true);
        let schema = &rb["content"]["application/json"]["schema"];
        // Schema create uses allOf: metadata (with labels) + SchemaData
        let all_of = schema["allOf"].as_array().unwrap();
        assert_eq!(all_of.len(), 2);
        // First part: metadata with labels
        let metadata = &all_of[0]["properties"]["metadata"]["properties"];
        assert!(metadata.get("labels").is_some());
        // Second part: SchemaData ref
        assert_eq!(all_of[1]["$ref"], "#/components/schemas/SchemaData");
    }

    #[test]
    fn build_static_paths_post_has_error_responses() {
        let paths = build_static_paths();
        let (_path, collection) =
            paths.iter().find(|(p, _)| p == "/apis/kapi.io/v1/Schema").unwrap();
        let responses = &collection["post"]["responses"];
        assert!(responses.get("201").is_some());
        assert!(responses.get("404").is_some());
        assert!(responses.get("409").is_some());
        assert!(responses.get("422").is_some());
    }

    #[test]
    fn build_kind_paths_has_collection_and_item() {
        let schema_data = crate::object::types::SchemaData {
            target_group: "example.io".to_string(),
            target_version: "v1".to_string(),
            target_kind: "NamespacedWidget".to_string(),
            spec_schema: serde_json::json!({ "type": "object" }),
            status_schema: None,
            scope: "Namespaced".to_string(),
        };
        let paths = build_kind_paths(&schema_data, "NamespacedWidgetExampleIo");
        let path_map: std::collections::HashMap<&str, &Value> =
            paths.iter().map(|(p, v)| (p.as_str(), v)).collect();

        // Cluster-scoped paths
        let collection = path_map.get("/apis/example.io/v1/NamespacedWidget").unwrap();
        assert!(collection.get("get").is_some(), "missing GET collection");
        assert!(collection.get("post").is_some(), "missing POST collection");

        let item = path_map.get("/apis/example.io/v1/NamespacedWidget/{name}").unwrap();
        assert!(item.get("get").is_some(), "missing GET item");
        assert!(item.get("put").is_some(), "missing PUT item");
        assert!(item.get("delete").is_some(), "missing DELETE item");

        let status = path_map.get("/apis/example.io/v1/NamespacedWidget/{name}/status").unwrap();
        assert!(status.get("get").is_some(), "missing GET status");
        assert!(status.get("put").is_some(), "missing PUT status");

        // Namespace-scoped paths (for namespaced scope)
        let ns_collection =
            path_map.get("/apis/example.io/v1/namespaces/{namespace}/NamespacedWidget").unwrap();
        assert!(ns_collection.get("get").is_some(), "missing namespace-scoped GET");
        assert!(ns_collection.get("post").is_some(), "missing namespace-scoped POST");

        let ns_item = path_map
            .get("/apis/example.io/v1/namespaces/{namespace}/NamespacedWidget/{name}")
            .unwrap();
        assert!(ns_item.get("get").is_some(), "missing namespace-scoped GET item");
        assert!(ns_item.get("put").is_some(), "missing namespace-scoped PUT item");
        assert!(ns_item.get("delete").is_some(), "missing namespace-scoped DELETE item");

        let ns_status = path_map
            .get("/apis/example.io/v1/namespaces/{namespace}/NamespacedWidget/{name}/status")
            .unwrap();
        assert!(ns_status.get("get").is_some(), "missing namespace-scoped GET status");
        assert!(ns_status.get("put").is_some(), "missing namespace-scoped PUT status");
    }

    #[test]
    fn build_kind_paths_cluster_scoped_has_no_namespace_paths() {
        let schema_data = crate::object::types::SchemaData {
            target_group: "example.io".to_string(),
            target_version: "v1".to_string(),
            target_kind: "ClusterWidget".to_string(),
            spec_schema: serde_json::json!({ "type": "object" }),
            status_schema: None,
            scope: "Cluster".to_string(),
        };
        let paths = build_kind_paths(&schema_data, "ClusterWidgetExampleIo");
        let path_map: std::collections::HashMap<&str, &Value> =
            paths.iter().map(|(p, v)| (p.as_str(), v)).collect();

        // Cluster-scoped paths should be present
        assert!(
            path_map.contains_key("/apis/example.io/v1/ClusterWidget"),
            "missing cluster-scoped collection"
        );
        assert!(
            path_map.contains_key("/apis/example.io/v1/ClusterWidget/{name}"),
            "missing cluster-scoped item"
        );

        // Namespace-scoped paths should NOT be present for cluster-scoped kinds
        assert!(
            !path_map.contains_key("/apis/example.io/v1/namespaces/{namespace}/ClusterWidget"),
            "namespace-scoped paths should not exist for cluster-scoped kind"
        );
    }

    #[test]
    fn build_kind_paths_has_namespace_parameter_on_namespaced_paths() {
        let schema_data = crate::object::types::SchemaData {
            target_group: "example.io".to_string(),
            target_version: "v1".to_string(),
            target_kind: "Widget".to_string(),
            spec_schema: serde_json::json!({ "type": "object" }),
            status_schema: None,
            scope: "Namespaced".to_string(),
        };
        let paths = build_kind_paths(&schema_data, "WidgetExampleIo");
        let path_map: std::collections::HashMap<&str, &Value> =
            paths.iter().map(|(p, v)| (p.as_str(), v)).collect();

        // Check namespace-scoped collection has namespace parameter in GET
        let ns_collection =
            path_map.get("/apis/example.io/v1/namespaces/{namespace}/Widget").unwrap();
        let get_params = ns_collection["get"]["parameters"].as_array().unwrap();
        let ns_param = get_params.iter().find(|p| p["name"] == "namespace").unwrap();
        assert_eq!(ns_param["in"], "path");
        assert_eq!(ns_param["required"], true);
        assert_eq!(ns_param["schema"]["type"], "string");

        // Check namespace-scoped item has namespace parameter
        let ns_item =
            path_map.get("/apis/example.io/v1/namespaces/{namespace}/Widget/{name}").unwrap();
        let get_item_params = ns_item["get"]["parameters"].as_array().unwrap();
        assert!(get_item_params.iter().any(|p| p["name"] == "namespace"));
        assert!(get_item_params.iter().any(|p| p["name"] == "name"));
    }

    #[test]
    fn build_kind_paths_cross_namespace_list_has_description() {
        let schema_data = crate::object::types::SchemaData {
            target_group: "example.io".to_string(),
            target_version: "v1".to_string(),
            target_kind: "Widget".to_string(),
            spec_schema: serde_json::json!({ "type": "object" }),
            status_schema: None,
            scope: "Namespaced".to_string(),
        };
        let paths = build_kind_paths(&schema_data, "WidgetExampleIo");
        let path_map: std::collections::HashMap<&str, &Value> =
            paths.iter().map(|(p, v)| (p.as_str(), v)).collect();

        let collection = path_map.get("/apis/example.io/v1/Widget").unwrap();
        let desc = collection["get"]["description"].as_str().unwrap_or("");
        assert!(
            desc.contains("Cross-namespace"),
            "cluster-scoped list of namespaced kind should mention cross-namespace: {desc}"
        );
    }

    #[test]
    fn build_kind_paths_list_has_watch_param() {
        let schema_data = crate::object::types::SchemaData {
            target_group: "example.io".to_string(),
            target_version: "v1".to_string(),
            target_kind: "Widget".to_string(),
            spec_schema: serde_json::json!({ "type": "object" }),
            status_schema: None,
            scope: "Namespaced".to_string(),
        };
        let paths = build_kind_paths(&schema_data, "WidgetExampleIo");
        let (_path, collection) =
            paths.iter().find(|(p, _)| p == "/apis/example.io/v1/Widget").unwrap();
        let params = collection["get"]["parameters"].as_array().unwrap();
        let watch = params.iter().find(|p| p["name"] == "watch").unwrap();
        assert_eq!(watch["in"], "query");
        assert_eq!(watch["schema"]["type"], "boolean");
        assert_eq!(watch["required"], false);
    }

    #[test]
    fn build_kind_paths_list_has_field_selector_param() {
        let schema_data = crate::object::types::SchemaData {
            target_group: "example.io".to_string(),
            target_version: "v1".to_string(),
            target_kind: "Widget".to_string(),
            spec_schema: serde_json::json!({ "type": "object" }),
            status_schema: None,
            scope: "Namespaced".to_string(),
        };
        let paths = build_kind_paths(&schema_data, "WidgetExampleIo");
        let (_path, collection) =
            paths.iter().find(|(p, _)| p == "/apis/example.io/v1/Widget").unwrap();
        let params = collection["get"]["parameters"].as_array().unwrap();
        let field_selector = params.iter().find(|p| p["name"] == "fieldSelector").unwrap();
        assert_eq!(field_selector["in"], "query");
        assert_eq!(field_selector["schema"]["type"], "string");
        assert_eq!(field_selector["required"], false);
    }

    #[test]
    fn build_kind_paths_list_has_400_response() {
        let schema_data = crate::object::types::SchemaData {
            target_group: "example.io".to_string(),
            target_version: "v1".to_string(),
            target_kind: "Widget".to_string(),
            spec_schema: serde_json::json!({ "type": "object" }),
            status_schema: None,
            scope: "Namespaced".to_string(),
        };
        let paths = build_kind_paths(&schema_data, "WidgetExampleIo");
        let (_path, collection) =
            paths.iter().find(|(p, _)| p == "/apis/example.io/v1/Widget").unwrap();
        let responses = &collection["get"]["responses"];
        assert!(responses.get("400").is_some(), "missing 400 response");
        let resp = &responses["400"];
        assert_eq!(
            resp["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/AppError"
        );
    }

    #[test]
    fn build_kind_paths_post_has_201_and_errors() {
        let schema_data = crate::object::types::SchemaData {
            target_group: "example.io".to_string(),
            target_version: "v1".to_string(),
            target_kind: "Widget".to_string(),
            spec_schema: serde_json::json!({ "type": "object" }),
            status_schema: None,
            scope: "Namespaced".to_string(),
        };
        let paths = build_kind_paths(&schema_data, "WidgetExampleIo");
        let (_path, collection) =
            paths.iter().find(|(p, _)| p == "/apis/example.io/v1/Widget").unwrap();
        let responses = &collection["post"]["responses"];
        assert!(responses.get("201").is_some());
        assert!(responses.get("404").is_some());
        assert!(responses.get("409").is_some());
        assert!(responses.get("422").is_some());
    }

    #[test]
    fn build_kind_paths_item_only_has_name_param() {
        let schema_data = crate::object::types::SchemaData {
            target_group: "example.io".to_string(),
            target_version: "v1".to_string(),
            target_kind: "Widget".to_string(),
            spec_schema: serde_json::json!({ "type": "object" }),
            status_schema: None,
            scope: "Namespaced".to_string(),
        };
        let paths = build_kind_paths(&schema_data, "WidgetExampleIo");
        let (_path, item) =
            paths.iter().find(|(p, _)| p == "/apis/example.io/v1/Widget/{name}").unwrap();
        let params = item["get"]["parameters"].as_array().unwrap();
        let names: Vec<&str> = params.iter().map(|p| p["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["name"], "only name param should be present, GVK is in the URL");
    }

    /// Helper to create services for testing with a fresh store and event bus.
    fn make_test_services()
    -> (crate::object::service::ObjectService, crate::object::schema_service::SchemaService) {
        use crate::event::EventPublisher;
        let store: std::sync::Arc<dyn crate::store::ObjectStore> =
            std::sync::Arc::new(crate::store::memory::InMemoryStore::new());
        let event_bus: std::sync::Arc<dyn EventPublisher> =
            std::sync::Arc::new(crate::event::EventBus::default());
        let meta_validator: std::sync::Arc<dyn crate::schema::SchemaValidator> =
            std::sync::Arc::new(
                crate::schema::meta_schema::compile_meta_schema()
                    .expect("meta-schema should compile"),
            );
        let schema_registry =
            crate::schema::SchemaRegistry::new(store.clone(), meta_validator.clone());
        let schema_service = crate::object::schema_service::SchemaService::new(
            store.clone(),
            event_bus.clone(),
            meta_validator,
        );
        let object_service =
            crate::object::service::ObjectService::new(store, event_bus, schema_registry);
        (object_service, schema_service)
    }

    #[tokio::test]
    async fn build_openapi_spec_includes_dynamic_paths_and_components() {
        let (service, schema_service) = make_test_services();
        let schema_key = crate::schema::schema_key();
        let schema_data = serde_json::json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": {
                "type": "object",
                "properties": {
                    "color": { "type": "string" }
                }
            }
        });
        schema_service
            .create(
                schema_key,
                crate::object::types::ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                schema_data,
            )
            .await
            .unwrap();

        let spec = build_openapi_spec(&service).await.unwrap();
        let paths = spec["paths"].as_object().unwrap();
        assert!(paths.contains_key("/apis/example.io/v1/Widget"), "missing collection path");
        assert!(paths.contains_key("/apis/example.io/v1/Widget/{name}"), "missing item path");
        let schemas = spec["components"]["schemas"].as_object().unwrap();
        assert!(schemas.contains_key("WidgetExampleIoV1"), "missing spec component");
        assert!(schemas.contains_key("WidgetExampleIoV1StoredObject"), "missing stored component");
        assert!(schemas.contains_key("WidgetExampleIoV1ListResponse"), "missing list component");
    }

    #[tokio::test]
    async fn build_openapi_spec_reflects_mutations() {
        let (service, schema_service) = make_test_services();
        let schema_key = crate::schema::schema_key();
        let schema_data = serde_json::json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "object" }
        });

        // Register schema → build spec → verify paths exist
        schema_service
            .create(
                schema_key.clone(),
                crate::object::types::ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                schema_data.clone(),
            )
            .await
            .unwrap();
        let spec_after_create = build_openapi_spec(&service).await.unwrap();
        assert!(
            spec_after_create["paths"]
                .as_object()
                .unwrap()
                .contains_key("/apis/example.io/v1/Widget")
        );

        // Delete schema → build spec → verify paths removed
        schema_service.delete(schema_key, "Widget.example.io.v1".to_string()).await.unwrap();
        let spec_after_delete = build_openapi_spec(&service).await.unwrap();
        assert!(
            !spec_after_delete["paths"]
                .as_object()
                .unwrap()
                .contains_key("/apis/example.io/v1/Widget"),
            "paths should be removed after schema deletion"
        );
    }
}
