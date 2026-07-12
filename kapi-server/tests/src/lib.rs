use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use futures::StreamExt;
use http_body_util::BodyExt;
use http_body_util::BodyStream;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::mpsc;
use tower::ServiceExt;

use kapi_server::AppConfig;
use kapi_server::event::{EventBus, EventPublisher};
use kapi_server::object::types::{WatchEvent, WatchEventType};
use kapi_server::store::ObjectStore;
use kapi_server::store::memory::InMemoryStore;
use kapi_server::store::sqlite::SQLiteStore;

pub mod controller_runtime;
pub mod controller_runtime_multi;
pub mod finalizers;
pub mod generation_semantics;
pub mod list_filtering;
pub mod namespace;
pub mod namespace_resource;
pub mod object_annotations;
pub mod object_crud;
pub mod object_labels;
pub mod optimistic_concurrency;
pub mod schema_deletion;
pub mod schema_validation;
pub mod status_subresource;
pub mod watch_events;

pub struct TestApp {
    pub router: Router,
    pub store: Arc<dyn ObjectStore>,
    pub event_bus: Arc<dyn EventPublisher>,
}

impl TestApp {
    pub async fn with_store(store: Arc<dyn ObjectStore>) -> Self {
        let event_bus: Arc<dyn EventPublisher> = Arc::new(EventBus::default());

        let config = AppConfig { port: 0, store: store.clone(), event_bus: event_bus.clone() };

        let router = kapi_server::create_app(&config).await.expect("failed to build app");

        Self { router, store, event_bus }
    }

    pub fn client(&self) -> TestClient {
        TestClient { router: self.router.clone() }
    }
}

pub struct TestStore {
    pub name: &'static str,
    pub factory: Box<dyn Fn() -> Arc<dyn ObjectStore>>,
}

pub fn all_test_stores() -> Vec<TestStore> {
    vec![
        TestStore { name: "memory", factory: Box::new(|| Arc::new(InMemoryStore::new())) },
        TestStore {
            name: "sqlite",
            factory: Box::new(|| {
                let store = SQLiteStore::new(":memory:").expect("failed to create SQLite store");
                Arc::new(store)
            }),
        },
    ]
}

#[derive(Clone)]
pub struct TestClient {
    router: Router,
}

impl TestClient {
    pub async fn get(&self, uri: &str) -> axum::response::Response<Body> {
        let req = Request::builder().method(Method::GET).uri(uri).body(Body::empty()).unwrap();
        self.router.clone().oneshot(req).await.unwrap()
    }

    pub async fn post(&self, uri: &str, body: Value) -> axum::response::Response<Body> {
        let req = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        self.router.clone().oneshot(req).await.unwrap()
    }

    pub async fn put(&self, uri: &str, body: Value) -> axum::response::Response<Body> {
        let req = Request::builder()
            .method(Method::PUT)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        self.router.clone().oneshot(req).await.unwrap()
    }

    pub async fn delete(&self, uri: &str) -> axum::response::Response<Body> {
        let req = Request::builder().method(Method::DELETE).uri(uri).body(Body::empty()).unwrap();
        self.router.clone().oneshot(req).await.unwrap()
    }
}

pub async fn parse_body<T: DeserializeOwned>(response: axum::response::Response<Body>) -> T {
    let body = response.into_body();
    let bytes = body.collect().await.expect("failed to read body").to_bytes();
    serde_json::from_slice(&bytes).expect("failed to parse JSON body")
}

pub fn assert_status(response: &axum::response::Response<Body>, expected: StatusCode) {
    assert_eq!(
        response.status(),
        expected,
        "expected status {expected}, got {}",
        response.status()
    );
}

pub fn widget_schema() -> Value {
    serde_json::json!({
        "targetGroup": "example.io",
        "targetVersion": "v1",
        "targetKind": "Widget",
        "specSchema": {
            "type": "object",
            "properties": {
                "color": { "type": "string" },
                "size": { "type": "integer" }
            },
            "required": ["color", "size"]
        }
    })
}

/// Register a Widget schema with an explicit "Namespaced" scope for namespace testing.
pub fn widget_namespaced_schema() -> Value {
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
        }
    })
}

pub fn widget(name: &str, color: &str, size: i64) -> Value {
    serde_json::json!({
        "metadata": { "name": name },
        "spec": {
            "color": color,
            "size": size
        }
    })
}

pub fn widget_with_labels(name: &str, color: &str, size: i64, labels: Value) -> Value {
    serde_json::json!({
        "metadata": { "name": name, "labels": labels },
        "spec": {
            "color": color,
            "size": size
        }
    })
}

/// Default namespace for namespaced integration tests.
pub const DEFAULT_NS: &str = "default";

/// Build a namespace-scoped collection URL for the Widget kind.
pub fn widget_collection_url(namespace: &str) -> String {
    format!("/apis/example.io/v1/namespaces/{namespace}/Widget")
}

/// Build a namespace-scoped item URL for the Widget kind.
pub fn widget_item_url(namespace: &str, name: &str) -> String {
    format!("/apis/example.io/v1/namespaces/{namespace}/Widget/{name}")
}

/// Build a namespace-scoped status URL for the Widget kind.
pub fn widget_status_url(namespace: &str, name: &str) -> String {
    format!("/apis/example.io/v1/namespaces/{namespace}/Widget/{name}/status")
}

pub fn parse_sse_events(buffer: &mut Vec<u8>) -> Vec<WatchEvent> {
    let mut events = Vec::new();

    let sep = if buffer.windows(4).position(|w| w == b"\r\n\r\n").is_some() {
        b"\r\n\r\n" as &[u8]
    } else if buffer.windows(2).position(|w| w == b"\n\n").is_some() {
        b"\n\n" as &[u8]
    } else {
        return events;
    };
    let sep_len = sep.len();

    loop {
        let pos = buffer.windows(sep_len).position(|w| w == sep);
        match pos {
            Some(end) => {
                let event_bytes = &buffer[..end];
                if let Ok(text) = std::str::from_utf8(event_bytes) {
                    let mut event_data = None;
                    let line_sep = if text.contains("\r\n") { "\r\n" } else { "\n" };
                    for line in text.split(line_sep) {
                        if let Some(data) = line.strip_prefix("data:") {
                            event_data = Some(data.trim().to_string());
                        }
                    }
                    if let Some(json) = event_data
                        && let Ok(event) = serde_json::from_str::<WatchEvent>(&json)
                    {
                        events.push(event);
                    }
                }
                buffer.drain(..=end + sep_len - 1);
            }
            None => break,
        }
    }
    events
}

pub async fn watch_events(client: &TestClient, uri: &str) -> mpsc::Receiver<WatchEvent> {
    let (tx, rx) = mpsc::channel(32);
    let client = client.clone();
    let uri = uri.to_string();

    tokio::spawn(async move {
        let response = client.get(&uri).await;
        let mut stream = BodyStream::new(response.into_body());
        let mut buf = Vec::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(frame) => {
                    if let Some(data) = frame.data_ref() {
                        buf.extend_from_slice(data);
                        let events = parse_sse_events(&mut buf);
                        for event in events {
                            if tx.send(event).await.is_err() {
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("watch stream error: {e}");
                    break;
                }
            }
        }
    });

    rx
}

pub async fn register_widget_schema(client: &TestClient) {
    let resp = client.post("/apis/kapi.io/v1/Schema", widget_namespaced_schema()).await;
    assert_status(&resp, StatusCode::CREATED);
}

/// Creates a Namespace object via the test client. Used by tests that
/// reference non-default namespaces. Requires the test app to have
/// bootstrapped the Namespace schema (it does by default via kapi_server::create_app).
pub async fn register_namespace(client: &TestClient, name: &str) {
    let body = serde_json::json!({
        "metadata": { "name": name },
        "spec": { "annotations": {} }
    });
    let resp = client.post("/apis/kapi.io/v1/Namespace", body).await;
    assert_status(&resp, StatusCode::CREATED);
}

/// Register a schema with an explicit scope parameter.
pub async fn register_schema_with_scope(
    client: &TestClient,
    target_group: &str,
    target_version: &str,
    target_kind: &str,
    scope: &str,
    spec_schema: Value,
) {
    let schema = serde_json::json!({
        "targetGroup": target_group,
        "targetVersion": target_version,
        "targetKind": target_kind,
        "scope": scope,
        "specSchema": spec_schema,
    });
    let resp = client.post("/apis/kapi.io/v1/Schema", schema).await;
    assert_status(&resp, StatusCode::CREATED);
}
