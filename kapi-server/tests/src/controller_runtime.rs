use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use axum::http::StatusCode;
use serde_json::Value;
use tokio::sync::broadcast;

use kapi_client::client::KapiClient;
use kapi_controller::controller::Controller;
use kapi_controller::reconciler::{ReconcileContext, ReconcileResult, Reconciler};
use kapi_core::ResourceKey;

use crate::{DEFAULT_NS, TestApp, assert_status, parse_body};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns a Widget schema with statusSchema enabled (required for controller
/// status updates).
fn widget_controller_schema() -> Value {
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
                "reconciled": { "type": "boolean" },
                "count": { "type": "integer" }
            }
        }
    })
}

/// Register a Widget schema with status support.
async fn register_widget_controller_schema(client: &crate::TestClient) {
    let resp = client.post("/apis/kapi.io/v1/Schema", widget_controller_schema()).await;
    assert_status(&resp, StatusCode::CREATED);
}

fn widget_url(namespace: &str, name: &str) -> String {
    format!("/apis/example.io/v1/namespaces/{namespace}/Widget/{name}")
}

fn widget_status_url(namespace: &str, name: &str) -> String {
    format!("/apis/example.io/v1/namespaces/{namespace}/Widget/{name}/status")
}

// ---------------------------------------------------------------------------
// Reconciler implementations
// ---------------------------------------------------------------------------

/// A reconciler that counts invocations and writes the count into `.status`.
struct CountingReconciler {
    call_count: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl Reconciler for CountingReconciler {
    async fn reconcile(
        &self,
        ctx: ReconcileContext,
    ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        let status = serde_json::json!({
            "reconciled": true,
            "count": self.call_count.load(Ordering::SeqCst),
        });
        ctx.client
            .update_status(
                &ctx.request.key,
                ctx.request.namespace.as_deref(),
                &ctx.request.name,
                &status,
            )
            .await?;

        Ok(ReconcileResult::default())
    }
}

/// A reconciler that always fails (used to exercise backoff).
struct ErrorReconciler {
    call_count: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl Reconciler for ErrorReconciler {
    async fn reconcile(
        &self,
        _ctx: ReconcileContext,
    ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Err("always fails".into())
    }
}

/// A reconciler that manages finalizers.
///
/// * If the object is being deleted → remove the finalizer so the server can
///   hard-delete it.
/// * Otherwise → ensure the finalizer is present.
struct FinalizerReconciler;

#[async_trait::async_trait]
impl Reconciler for FinalizerReconciler {
    async fn reconcile(
        &self,
        ctx: ReconcileContext,
    ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>> {
        let obj = ctx
            .client
            .get(&ctx.request.key, ctx.request.namespace.as_deref(), &ctx.request.name)
            .await?;

        if kapi_controller::finalizer::is_deleting(&obj) {
            kapi_controller::finalizer::remove_finalizer(
                &ctx.client,
                &obj,
                "controller.kapi.io/cleanup",
            )
            .await?;
        } else {
            kapi_controller::finalizer::ensure_finalizer(
                &ctx.client,
                &obj,
                "controller.kapi.io/cleanup",
            )
            .await?;
        }

        Ok(ReconcileResult::default())
    }
}

// ---------------------------------------------------------------------------
// Helper: build a ResourceKey for the Widget kind
// ---------------------------------------------------------------------------

fn widget_key() -> ResourceKey {
    ResourceKey { group: "example.io".into(), version: "v1".into(), kind: "Widget".into() }
}

// ---------------------------------------------------------------------------
// Scenario 6.1: Create object → reconciler is called → status is updated
// ---------------------------------------------------------------------------

pub async fn test_controller_reconciles_on_create(
    app: &TestApp,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = app.client();

    // Start HTTP server from app.router
    let router = app.router.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let base_url = format!("http://{}", addr);
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    tokio::time::sleep(Duration::from_millis(50)).await;

    register_widget_controller_schema(&client).await;

    let kapi_client = KapiClient::new(&base_url)?;
    let call_count = Arc::new(AtomicU32::new(0));

    let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

    let controller = Controller::new(
        widget_key(),
        Arc::new(CountingReconciler { call_count: call_count.clone() }),
        kapi_client,
    )
    .namespace(DEFAULT_NS)
    .shutdown_signal(shutdown_rx);

    let handle = tokio::spawn(async move { controller.start().await });

    // Let the controller establish the watch connection.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Create an object.
    let body = serde_json::json!({
        "metadata": { "name": "create-widget" },
        "spec": { "color": "red", "size": 10 },
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);

    // Wait for the reconciler to be invoked.
    let ok = tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            if call_count.load(Ordering::SeqCst) > 0 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(ok.is_ok(), "reconciler was not called within timeout");

    // Verify status was updated.
    let resp = client.get(&widget_status_url(DEFAULT_NS, "create-widget")).await;
    assert_status(&resp, StatusCode::OK);
    let status: Value = parse_body(resp).await;
    assert_eq!(status["reconciled"], true, "status.reconciled should be true");
    assert!(status["count"].as_u64().unwrap_or(0) >= 1, "status.count should be >= 1");

    // Clean shutdown.
    let _ = shutdown_tx.send(());
    handle.await.unwrap();

    Ok(())
}

// ---------------------------------------------------------------------------
// Scenario 6.2: Rapid updates are deduplicated by the work queue
// ---------------------------------------------------------------------------

pub async fn test_controller_deduplication_on_updates(
    app: &TestApp,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = app.client();

    // Start HTTP server from app.router
    let router = app.router.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let base_url = format!("http://{}", addr);
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    tokio::time::sleep(Duration::from_millis(50)).await;

    register_widget_controller_schema(&client).await;

    let kapi_client = KapiClient::new(&base_url)?;
    let call_count = Arc::new(AtomicU32::new(0));

    let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

    let controller = Controller::new(
        widget_key(),
        Arc::new(CountingReconciler { call_count: call_count.clone() }),
        kapi_client,
    )
    .namespace(DEFAULT_NS)
    .shutdown_signal(shutdown_rx);

    let handle = tokio::spawn(async move { controller.start().await });

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Create the initial object.
    let body = serde_json::json!({
        "metadata": { "name": "dedup-widget" },
        "spec": { "color": "red", "size": 1 },
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);

    // Wait for the first reconcile.
    let ok = tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            if call_count.load(Ordering::SeqCst) > 0 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(ok.is_ok(), "reconciler was not called for initial create");

    // Perform 20 rapid spec updates, retrying on 409 when the controller's
    // status update bumps the resourceVersion between our GET and PUT.
    for i in 1..=20 {
        for retry in 0..5 {
            // Re-fetch the object to get the latest resourceVersion (CAS).
            let resp = client.get(&widget_url(DEFAULT_NS, "dedup-widget")).await;
            assert_status(&resp, StatusCode::OK);
            let obj: Value = parse_body(resp).await;

            let rv = obj["system"]["resourceVersion"].as_u64().unwrap_or(0);
            let created_at = obj["system"]["createdAt"].as_str().unwrap_or("").to_string();
            let updated_at = obj["system"]["updatedAt"].as_str().unwrap_or("").to_string();

            let update_body = serde_json::json!({
                "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
                "metadata": { "name": "dedup-widget" },
                "system": {
                    "resourceVersion": rv,
                    "createdAt": created_at,
                    "updatedAt": updated_at,
                },
                "spec": { "color": "red", "size": i + 1 },
            });
            let resp = client.put(&widget_url(DEFAULT_NS, "dedup-widget"), update_body).await;
            if retry < 4 && resp.status() == StatusCode::CONFLICT {
                // Controller bumped resourceVersion; retry with fresh state.
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
            assert_status(&resp, StatusCode::OK);
            break;
        }
    }

    // Allow time for events to be delivered and the queue to drain.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let count = call_count.load(Ordering::SeqCst);
    assert!(
        count < 18,
        "expected deduplication to keep reconcile calls well below 20, got {count}"
    );

    let _ = shutdown_tx.send(());
    handle.await.unwrap();

    Ok(())
}

// ---------------------------------------------------------------------------
// Scenario 6.3: Error reconciler triggers retry with backoff
// ---------------------------------------------------------------------------

pub async fn test_controller_error_backoff(
    app: &TestApp,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = app.client();

    // Start HTTP server from app.router
    let router = app.router.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let base_url = format!("http://{}", addr);
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    tokio::time::sleep(Duration::from_millis(50)).await;

    register_widget_controller_schema(&client).await;

    let kapi_client = KapiClient::new(&base_url)?;
    let call_count = Arc::new(AtomicU32::new(0));

    let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

    let controller = Controller::new(
        widget_key(),
        Arc::new(ErrorReconciler { call_count: call_count.clone() }),
        kapi_client,
    )
    .namespace(DEFAULT_NS)
    .shutdown_signal(shutdown_rx);

    let handle = tokio::spawn(async move { controller.start().await });

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Create an object — the error reconciler will fail and be retried.
    let body = serde_json::json!({
        "metadata": { "name": "error-widget" },
        "spec": { "color": "red", "size": 10 },
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);

    // Wait for at least 2 reconcile attempts (initial + retry with backoff).
    let ok = tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if call_count.load(Ordering::SeqCst) >= 2 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(
        ok.is_ok(),
        "error reconciler was not retried within timeout (call_count={})",
        call_count.load(Ordering::SeqCst)
    );

    let count = call_count.load(Ordering::SeqCst);
    assert!(count >= 2, "expected at least 2 reconcile calls after error, got {count}");

    let _ = shutdown_tx.send(());
    handle.await.unwrap();

    Ok(())
}

// ---------------------------------------------------------------------------
// Scenario 6.4: Finalizer helpers (ensure / remove)
// ---------------------------------------------------------------------------

pub async fn test_controller_finalizer_helpers(
    app: &TestApp,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = app.client();

    // Start HTTP server from app.router
    let router = app.router.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let base_url = format!("http://{}", addr);
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    tokio::time::sleep(Duration::from_millis(50)).await;

    register_widget_controller_schema(&client).await;

    let kapi_client = KapiClient::new(&base_url)?;

    let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

    let controller = Controller::new(widget_key(), Arc::new(FinalizerReconciler), kapi_client)
        .namespace(DEFAULT_NS)
        .shutdown_signal(shutdown_rx);

    let handle = tokio::spawn(async move { controller.start().await });

    tokio::time::sleep(Duration::from_millis(200)).await;

    // 1) Create an object.
    let body = serde_json::json!({
        "metadata": { "name": "finalizer-widget" },
        "spec": { "color": "red", "size": 10 },
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);

    // 2) Wait for the reconciler to add the finalizer.
    let ok = tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            let resp = client.get(&widget_url(DEFAULT_NS, "finalizer-widget")).await;
            if resp.status() == StatusCode::OK {
                let obj: Value = parse_body(resp).await;
                if let Some(finalizers) = obj["metadata"]["finalizers"].as_array()
                    && finalizers.iter().any(|v| v == "controller.kapi.io/cleanup")
                {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(ok.is_ok(), "finalizer was not added within timeout");

    // 3) Delete the object (soft-delete because finalizer is present).
    let resp = client.delete(&widget_url(DEFAULT_NS, "finalizer-widget")).await;
    assert_status(&resp, StatusCode::OK);
    let deleted: Value = parse_body(resp).await;
    assert!(
        deleted["system"]["deletionTimestamp"].is_string(),
        "expected deletionTimestamp to be set after delete with finalizer"
    );

    // 4) The reconciler should remove the finalizer, after which the server
    //    hard-deletes the object.
    let ok = tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            let resp = client.get(&widget_url(DEFAULT_NS, "finalizer-widget")).await;
            if resp.status() == StatusCode::NOT_FOUND {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(ok.is_ok(), "object was not hard-deleted after finalizer removal within timeout");

    let _ = shutdown_tx.send(());
    handle.await.unwrap();

    Ok(())
}
