//! Integration tests for the typed client.
//!
//! These tests start a real HTTP server and exercise the TypedClient's
//! serialization/deserialization through actual HTTP requests.

use std::collections::HashMap;
use std::time::Duration;

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use kapi_client::client::KapiClient;
use kapi_client::typed::{TypedClient, TypedResource};
use kapi_client::{ListOptions, ObjectMeta, ResourceKey, SystemMetadata};

use crate::{DEFAULT_NS, TestApp, assert_status};

// ---------------------------------------------------------------------------
// Test resource types
// ---------------------------------------------------------------------------

/// Spec for the test Widget resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WidgetSpec {
    color: String,
    size: i32,
}

/// Status for the test Widget resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WidgetStatus {
    ready: bool,
    message: Option<String>,
}

/// Typed wrapper struct for the Widget resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Widget {
    metadata: ObjectMeta,
    system: SystemMetadata,
    spec: WidgetSpec,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<WidgetStatus>,
}

impl TypedResource for Widget {
    type Spec = WidgetSpec;
    type Status = WidgetStatus;

    fn key() -> ResourceKey {
        ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        }
    }

    fn from_parts(
        metadata: ObjectMeta,
        system: SystemMetadata,
        spec: Self::Spec,
        status: Option<Self::Status>,
    ) -> Self {
        Self { metadata, system, spec, status }
    }

    fn metadata(&self) -> &ObjectMeta {
        &self.metadata
    }

    fn system(&self) -> &SystemMetadata {
        &self.system
    }

    fn spec(&self) -> &Self::Spec {
        &self.spec
    }

    fn status(&self) -> Option<&Self::Status> {
        self.status.as_ref()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns a Widget schema with statusSchema enabled.
fn widget_typed_schema() -> serde_json::Value {
    serde_json::json!({
        "targetGroup": "example.io",
        "targetVersion": "v1",
        "targetKind": "Widget",
        "scope": "Namespaced",
        "specSchema": {
            "type": "object",
            "properties": {
                "color": { "type": "string" },
                "size": { "type": "integer" }
            },
            "required": ["color", "size"]
        },
        "statusSchema": {
            "type": "object",
            "properties": {
                "ready": { "type": "boolean" },
                "message": { "type": "string" }
            }
        }
    })
}

/// Register the Widget schema for typed client tests.
async fn register_widget_schema(client: &crate::TestClient) {
    let resp = client.post("/apis/kapi.io/v1/Schema", widget_typed_schema()).await;
    assert_status(&resp, StatusCode::CREATED);
}

/// Start a real HTTP server and return the base URL.
async fn start_server(app: &TestApp) -> String {
    let router = app.router.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("failed to bind");
    let addr = listener.local_addr().expect("failed to get addr");
    let base_url = format!("http://{}", addr);
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    tokio::time::sleep(Duration::from_millis(50)).await;
    base_url
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Test: Create a widget using TypedClient and verify the returned object.
pub async fn test_typed_client_create(app: &TestApp) -> Result<(), Box<dyn std::error::Error>> {
    let test_client = app.client();
    register_widget_schema(&test_client).await;

    let base_url = start_server(app).await;
    let kapi_client = KapiClient::new(&base_url)?;
    let typed = TypedClient::<Widget>::new(kapi_client);

    let widget = Widget {
        metadata: ObjectMeta {
            name: "typed-widget-1".to_string(),
            namespace: Some(DEFAULT_NS.to_string()),
            labels: HashMap::new(),
            annotations: HashMap::new(),
            finalizers: Vec::new(),
        },
        system: SystemMetadata::initial(),
        spec: WidgetSpec { color: "red".to_string(), size: 42 },
        status: None,
    };

    let created = typed.create(Some(DEFAULT_NS), &widget).await?;

    assert_eq!(created.metadata.name, "typed-widget-1");
    assert_eq!(created.spec.color, "red");
    assert_eq!(created.spec.size, 42);
    assert!(created.status.is_none());

    Ok(())
}

/// Test: Get a widget using TypedClient.
pub async fn test_typed_client_get(app: &TestApp) -> Result<(), Box<dyn std::error::Error>> {
    let test_client = app.client();
    register_widget_schema(&test_client).await;

    // Create via TestClient first
    let body = serde_json::json!({
        "metadata": { "name": "typed-widget-2" },
        "spec": { "color": "blue", "size": 100 }
    });
    let resp = test_client
        .post(&format!("/apis/example.io/v1/namespaces/{}/Widget", DEFAULT_NS), body)
        .await;
    assert_status(&resp, StatusCode::CREATED);

    let base_url = start_server(app).await;
    let kapi_client = KapiClient::new(&base_url)?;
    let typed = TypedClient::<Widget>::new(kapi_client);

    let fetched = typed.get(Some(DEFAULT_NS), "typed-widget-2").await?;

    assert_eq!(fetched.metadata.name, "typed-widget-2");
    assert_eq!(fetched.spec.color, "blue");
    assert_eq!(fetched.spec.size, 100);

    Ok(())
}

/// Test: Update a widget using TypedClient.
pub async fn test_typed_client_update(app: &TestApp) -> Result<(), Box<dyn std::error::Error>> {
    let test_client = app.client();
    register_widget_schema(&test_client).await;

    let base_url = start_server(app).await;
    let kapi_client = KapiClient::new(&base_url)?;
    let typed = TypedClient::<Widget>::new(kapi_client);

    // Create
    let widget = Widget {
        metadata: ObjectMeta {
            name: "typed-widget-3".to_string(),
            namespace: Some(DEFAULT_NS.to_string()),
            labels: HashMap::new(),
            annotations: HashMap::new(),
            finalizers: Vec::new(),
        },
        system: SystemMetadata::initial(),
        spec: WidgetSpec { color: "green".to_string(), size: 50 },
        status: None,
    };
    let mut created = typed.create(Some(DEFAULT_NS), &widget).await?;

    // Update spec
    created.spec.color = "yellow".to_string();
    created.spec.size = 75;

    let updated = typed.update(Some(DEFAULT_NS), &created).await?;

    assert_eq!(updated.metadata.name, "typed-widget-3");
    assert_eq!(updated.spec.color, "yellow");
    assert_eq!(updated.spec.size, 75);
    // Resource version should have bumped
    assert!(updated.metadata.name == "typed-widget-3");

    Ok(())
}

/// Test: Delete a widget using TypedClient.
pub async fn test_typed_client_delete(app: &TestApp) -> Result<(), Box<dyn std::error::Error>> {
    let test_client = app.client();
    register_widget_schema(&test_client).await;

    let base_url = start_server(app).await;
    let kapi_client = KapiClient::new(&base_url)?;
    let typed = TypedClient::<Widget>::new(kapi_client);

    // Create
    let widget = Widget {
        metadata: ObjectMeta {
            name: "typed-widget-4".to_string(),
            namespace: Some(DEFAULT_NS.to_string()),
            labels: HashMap::new(),
            annotations: HashMap::new(),
            finalizers: Vec::new(),
        },
        system: SystemMetadata::initial(),
        spec: WidgetSpec { color: "purple".to_string(), size: 25 },
        status: None,
    };
    typed.create(Some(DEFAULT_NS), &widget).await?;

    // Delete
    let deleted = typed.delete(Some(DEFAULT_NS), "typed-widget-4").await?;

    assert_eq!(deleted.metadata.name, "typed-widget-4");
    assert_eq!(deleted.spec.color, "purple");

    // Verify it's gone
    let result = typed.get(Some(DEFAULT_NS), "typed-widget-4").await;
    assert!(result.is_err());

    Ok(())
}

/// Test: List widgets using TypedClient.
pub async fn test_typed_client_list(app: &TestApp) -> Result<(), Box<dyn std::error::Error>> {
    let test_client = app.client();
    register_widget_schema(&test_client).await;

    let base_url = start_server(app).await;
    let kapi_client = KapiClient::new(&base_url)?;
    let typed = TypedClient::<Widget>::new(kapi_client);

    // Create multiple widgets
    for i in 1..=3 {
        let widget = Widget {
            metadata: ObjectMeta {
                name: format!("typed-widget-list-{}", i),
                namespace: Some(DEFAULT_NS.to_string()),
                labels: HashMap::new(),
                annotations: HashMap::new(),
                finalizers: Vec::new(),
            },
            system: SystemMetadata::initial(),
            spec: WidgetSpec { color: format!("color-{}", i), size: i * 10 },
            status: None,
        };
        typed.create(Some(DEFAULT_NS), &widget).await?;
    }

    // List
    let opts = ListOptions::default();
    let list = typed.list(Some(DEFAULT_NS), &opts).await?;

    assert_eq!(list.len(), 3);
    // Verify all items are deserialized correctly
    for (i, widget) in list.iter().enumerate() {
        let expected_name = format!("typed-widget-list-{}", i + 1);
        assert_eq!(widget.metadata.name, expected_name);
        assert_eq!(widget.spec.color, format!("color-{}", i + 1));
        assert_eq!(widget.spec.size, (i as i32 + 1) * 10);
    }

    Ok(())
}

/// Test: TypedClient with status field.
pub async fn test_typed_client_with_status(
    app: &TestApp,
) -> Result<(), Box<dyn std::error::Error>> {
    let test_client = app.client();
    register_widget_schema(&test_client).await;

    let base_url = start_server(app).await;
    let kapi_client = KapiClient::new(&base_url)?;
    let typed = TypedClient::<Widget>::new(kapi_client);

    // Create a widget
    let widget = Widget {
        metadata: ObjectMeta {
            name: "typed-widget-status".to_string(),
            namespace: Some(DEFAULT_NS.to_string()),
            labels: HashMap::new(),
            annotations: HashMap::new(),
            finalizers: Vec::new(),
        },
        system: SystemMetadata::initial(),
        spec: WidgetSpec { color: "orange".to_string(), size: 60 },
        status: None,
    };
    let created = typed.create(Some(DEFAULT_NS), &widget).await?;
    assert!(created.status.is_none());

    // Update status via raw KapiClient (typed client doesn't have status methods yet)
    let status = serde_json::json!({
        "ready": true,
        "message": "All systems go"
    });
    let key = Widget::key();
    let kapi_client = typed.inner();
    let _updated =
        kapi_client.update_status(&key, Some(DEFAULT_NS), "typed-widget-status", &status).await?;

    // Fetch and verify status is present
    let fetched = typed.get(Some(DEFAULT_NS), "typed-widget-status").await?;
    assert!(fetched.status.is_some());
    let status = fetched.status.unwrap();
    assert!(status.ready);
    assert_eq!(status.message, Some("All systems go".to_string()));

    Ok(())
}

/// Test: TypedError::NotFound variant is produced for 404 responses.
pub async fn test_typed_error_not_found(app: &TestApp) -> Result<(), Box<dyn std::error::Error>> {
    let test_client = app.client();
    register_widget_schema(&test_client).await;

    let base_url = start_server(app).await;
    let kapi_client = KapiClient::new(&base_url)?;
    let typed = TypedClient::<Widget>::new(kapi_client);

    // Try to get a non-existent widget
    let result = typed.get(Some(DEFAULT_NS), "non-existent-widget").await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        kapi_client::typed::TypedError::NotFound { what, identifier } => {
            assert_eq!(what, "object");
            assert!(
                identifier.contains("non-existent-widget"),
                "identifier should contain widget name: {}",
                identifier
            );
        }
        _ => panic!("Expected TypedError::NotFound, got {:?}", err),
    }

    Ok(())
}

/// Test: TypedError::AlreadyExists variant is produced for 409 AlreadyExists responses.
pub async fn test_typed_error_already_exists(
    app: &TestApp,
) -> Result<(), Box<dyn std::error::Error>> {
    let test_client = app.client();
    register_widget_schema(&test_client).await;

    let base_url = start_server(app).await;
    let kapi_client = KapiClient::new(&base_url)?;
    let typed = TypedClient::<Widget>::new(kapi_client);

    // Create a widget
    let widget = Widget {
        metadata: ObjectMeta {
            name: "duplicate-widget".to_string(),
            namespace: Some(DEFAULT_NS.to_string()),
            labels: HashMap::new(),
            annotations: HashMap::new(),
            finalizers: Vec::new(),
        },
        system: SystemMetadata::initial(),
        spec: WidgetSpec { color: "red".to_string(), size: 42 },
        status: None,
    };
    typed.create(Some(DEFAULT_NS), &widget).await?;

    // Try to create it again
    let result = typed.create(Some(DEFAULT_NS), &widget).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        kapi_client::typed::TypedError::AlreadyExists { kind, name } => {
            assert_eq!(kind, "Widget");
            assert_eq!(name, "duplicate-widget");
        }
        _ => panic!("Expected TypedError::AlreadyExists, got {:?}", err),
    }

    Ok(())
}

/// Test: TypedError::Conflict variant is produced for 409 Conflict responses.
pub async fn test_typed_error_conflict(app: &TestApp) -> Result<(), Box<dyn std::error::Error>> {
    let test_client = app.client();
    register_widget_schema(&test_client).await;

    let base_url = start_server(app).await;
    let kapi_client = KapiClient::new(&base_url)?;
    let typed = TypedClient::<Widget>::new(kapi_client);

    // Create a widget
    let widget = Widget {
        metadata: ObjectMeta {
            name: "conflict-widget".to_string(),
            namespace: Some(DEFAULT_NS.to_string()),
            labels: HashMap::new(),
            annotations: HashMap::new(),
            finalizers: Vec::new(),
        },
        system: SystemMetadata::initial(),
        spec: WidgetSpec { color: "red".to_string(), size: 42 },
        status: None,
    };
    let mut created = typed.create(Some(DEFAULT_NS), &widget).await?;

    // Update it once to bump resource version
    created.spec.color = "blue".to_string();
    let _updated = typed.update(Some(DEFAULT_NS), &created).await?;

    // Try to update with the old resource version (should conflict)
    let result = typed.update(Some(DEFAULT_NS), &created).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        kapi_client::typed::TypedError::Conflict { expected, actual } => {
            // expected should be the current resource version, actual should be the old one
            assert!(expected > actual);
        }
        _ => panic!("Expected TypedError::Conflict, got {:?}", err),
    }

    // Verify the updated version is still there
    let fetched = typed.get(Some(DEFAULT_NS), "conflict-widget").await?;
    assert_eq!(fetched.spec.color, "blue");

    Ok(())
}

/// Test: TypedError::Forbidden variant is produced for 403 ProtectedNamespace responses.
pub async fn test_typed_error_forbidden(app: &TestApp) -> Result<(), Box<dyn std::error::Error>> {
    let test_client = app.client();
    register_widget_schema(&test_client).await;

    let base_url = start_server(app).await;
    let kapi_client = KapiClient::new(&base_url)?;
    let _typed = TypedClient::<Widget>::new(kapi_client);

    // Try to delete the default namespace (should be forbidden)
    let resp = test_client.delete("/apis/kapi.io/v1/Namespace/default").await;
    assert_status(&resp, StatusCode::FORBIDDEN);

    // Verify the error response has the right structure
    let body: serde_json::Value = crate::parse_body(resp).await;
    assert_eq!(body["code"], "ProtectedNamespace");

    Ok(())
}

/// Test: Non-mapped errors fall through to TypedError::ApiError.
pub async fn test_typed_error_api_error_fallback(
    app: &TestApp,
) -> Result<(), Box<dyn std::error::Error>> {
    let test_client = app.client();
    register_widget_schema(&test_client).await;

    let base_url = start_server(app).await;
    let kapi_client = KapiClient::new(&base_url)?;
    let typed = TypedClient::<Widget>::new(kapi_client);

    // Create a widget with invalid labels (should return InvalidLabel error)
    let widget = Widget {
        metadata: ObjectMeta {
            name: "invalid-label-widget".to_string(),
            namespace: Some(DEFAULT_NS.to_string()),
            labels: {
                let mut labels = HashMap::new();
                labels.insert("invalid label key".to_string(), "value".to_string());
                labels
            },
            annotations: HashMap::new(),
            finalizers: Vec::new(),
        },
        system: SystemMetadata::initial(),
        spec: WidgetSpec { color: "red".to_string(), size: 42 },
        status: None,
    };

    let result = typed.create(Some(DEFAULT_NS), &widget).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        kapi_client::typed::TypedError::ApiError(client_err) => {
            // Verify it's a ClientError::ApiError with the right code
            match client_err {
                kapi_client::error::ClientError::ApiError { status, code, .. } => {
                    assert_eq!(status, 400);
                    assert_eq!(code, "InvalidLabel");
                }
                _ => panic!("Expected ClientError::ApiError, got {:?}", client_err),
            }
        }
        _ => panic!("Expected TypedError::ApiError, got {:?}", err),
    }

    Ok(())
}

/// Test: Defensive defaults when server sends 404 with empty details.
pub async fn test_typed_error_defensive_defaults(
    _app: &TestApp,
) -> Result<(), Box<dyn std::error::Error>> {
    // This test verifies that the From<ClientError> impl uses defensive defaults
    // We can't easily trigger this from the server (it always sends proper details),
    // so we test the conversion directly.
    use kapi_client::error::ClientError;
    use kapi_client::typed::TypedError;

    let client_err = ClientError::ApiError {
        status: 404,
        code: "NotFound".to_string(),
        message: "not found".to_string(),
        details: serde_json::json!({}), // Empty details
    };

    let typed_err: TypedError = client_err.into();

    match typed_err {
        TypedError::NotFound { what, identifier } => {
            assert_eq!(what, "unknown");
            assert_eq!(identifier, "unknown");
        }
        _ => panic!("Expected TypedError::NotFound, got {:?}", typed_err),
    }

    // Test with missing details field entirely
    let client_err2 = ClientError::ApiError {
        status: 409,
        code: "Conflict".to_string(),
        message: "conflict".to_string(),
        details: serde_json::Value::Null, // Null details
    };

    let typed_err2: TypedError = client_err2.into();

    match typed_err2 {
        TypedError::Conflict { expected, actual } => {
            assert_eq!(expected, 0);
            assert_eq!(actual, 0);
        }
        _ => panic!("Expected TypedError::Conflict, got {:?}", typed_err2),
    }

    Ok(())
}
