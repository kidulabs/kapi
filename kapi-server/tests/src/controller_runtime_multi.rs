use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use axum::http::StatusCode;
use serde_json::Value;

use kapi_client::client::KapiClient;
use kapi_controller::manager::Manager;
use kapi_controller::reconciler::{ReconcileContext, ReconcileResult, Reconciler};
use kapi_core::ResourceKey;

use crate::{DEFAULT_NS, TestApp, assert_status};

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

/// Returns a Gadget schema with statusSchema enabled.
fn gadget_controller_schema() -> Value {
    serde_json::json!({
        "targetGroup": "example.io",
        "targetVersion": "v1",
        "targetKind": "Gadget",
        "scope": "Namespaced",
        "specSchema": {
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "weight": { "type": "integer" }
            },
            "required": ["name", "weight"]
        },
        "statusSchema": {
            "type": "object",
            "properties": {
                "reconciled": { "type": "boolean" }
            }
        }
    })
}

/// Register a Widget schema with status support.
async fn register_widget_controller_schema(client: &crate::TestClient) {
    let resp = client.post("/apis/kapi.io/v1/Schema", widget_controller_schema()).await;
    assert_status(&resp, StatusCode::CREATED);
}

/// Register a Gadget schema with status support.
async fn register_gadget_controller_schema(client: &crate::TestClient) {
    let resp = client.post("/apis/kapi.io/v1/Schema", gadget_controller_schema()).await;
    assert_status(&resp, StatusCode::CREATED);
}

/// ResourceKey for the Widget kind.
fn widget_key() -> ResourceKey {
    ResourceKey { group: "example.io".into(), version: "v1".into(), kind: "Widget".into() }
}

/// ResourceKey for the Gadget kind.
fn gadget_key() -> ResourceKey {
    ResourceKey { group: "example.io".into(), version: "v1".into(), kind: "Gadget".into() }
}

// ---------------------------------------------------------------------------
// Reconciler implementations
// ---------------------------------------------------------------------------

/// A reconciler that counts invocations.
struct CountingReconciler {
    call_count: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl Reconciler for CountingReconciler {
    async fn reconcile(
        &self,
        _ctx: ReconcileContext,
    ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(ReconcileResult::default())
    }
}

/// A reconciler that panics on the first call and then succeeds.
///
/// Used to test that a panic in one controller task does not affect other
/// controller tasks managed by the same [`Manager`].
struct PanickingReconciler {
    call_count: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl Reconciler for PanickingReconciler {
    async fn reconcile(
        &self,
        _ctx: ReconcileContext,
    ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        if count == 0 {
            panic!("intentional panic for testing");
        }
        Ok(ReconcileResult::default())
    }
}

// ---------------------------------------------------------------------------
// Test 1: Manager starts multiple controllers
// ---------------------------------------------------------------------------

pub async fn test_manager_starts_multiple_controllers(
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

    // Register schemas for both kinds
    register_widget_controller_schema(&client).await;
    register_gadget_controller_schema(&client).await;

    let kapi_client = KapiClient::new(&base_url)?;
    let widget_count = Arc::new(AtomicU32::new(0));
    let gadget_count = Arc::new(AtomicU32::new(0));

    let mut manager = Manager::new(kapi_client);

    // Register Widget controller
    manager
        .controller_for(widget_key())
        .reconcile_with(CountingReconciler { call_count: widget_count.clone() })
        .namespace(DEFAULT_NS)
        .register();

    // Register Gadget controller
    manager
        .controller_for(gadget_key())
        .reconcile_with(CountingReconciler { call_count: gadget_count.clone() })
        .namespace(DEFAULT_NS)
        .register();

    let shutdown_tx = manager.shutdown_sender();
    let mgr_handle = tokio::spawn(async move { manager.start().await });

    // Let controllers establish watch connections
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Create a Widget object
    let body = serde_json::json!({
        "metadata": { "name": "multi-widget" },
        "spec": { "color": "red", "size": 10 },
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);

    // Create a Gadget object
    let body = serde_json::json!({
        "metadata": { "name": "multi-gadget" },
        "spec": { "name": "gadget-1", "weight": 42 },
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Gadget", body).await;
    assert_status(&resp, StatusCode::CREATED);

    // Wait for both controllers to have been invoked
    let ok = tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            let wc = widget_count.load(Ordering::SeqCst);
            let gc = gadget_count.load(Ordering::SeqCst);
            if wc > 0 && gc > 0 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        ok.is_ok(),
        "both controllers were not called within timeout (widget={}, gadget={})",
        widget_count.load(Ordering::SeqCst),
        gadget_count.load(Ordering::SeqCst),
    );

    // Shutdown
    let _ = shutdown_tx.send(());
    let mgr_result = tokio::time::timeout(Duration::from_secs(5), mgr_handle).await;
    assert!(mgr_result.is_ok(), "manager did not shut down within timeout");

    Ok(())
}

// ---------------------------------------------------------------------------
// Test 2: Manager graceful shutdown
// ---------------------------------------------------------------------------

pub async fn test_manager_graceful_shutdown(
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

    let mut manager = Manager::new(kapi_client);

    manager
        .controller_for(widget_key())
        .reconcile_with(CountingReconciler { call_count: call_count.clone() })
        .namespace(DEFAULT_NS)
        .register();

    let shutdown_tx = manager.shutdown_sender();
    let mgr_handle = tokio::spawn(async move { manager.start().await });

    // Let the controller establish the watch connection.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Create a Widget object.
    let body = serde_json::json!({
        "metadata": { "name": "graceful-widget" },
        "spec": { "color": "blue", "size": 5 },
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

    // Send shutdown signal.
    let _ = shutdown_tx.send(());

    // Verify manager completes gracefully (returns Ok(())).
    let mgr_result = tokio::time::timeout(Duration::from_secs(5), mgr_handle).await;
    assert!(mgr_result.is_ok(), "manager did not shut down within timeout");
    let result = mgr_result.unwrap();
    assert!(
        result.is_ok(),
        "manager.start() should return Ok(()) on graceful shutdown, got: {:?}",
        result
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Test 3: Panic isolation — one controller panicking does not affect others
// ---------------------------------------------------------------------------

pub async fn test_manager_panic_isolation(app: &TestApp) -> Result<(), Box<dyn std::error::Error>> {
    let client = app.client();

    // Start HTTP server from app.router
    let router = app.router.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let base_url = format!("http://{}", addr);
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Register schemas for both kinds
    register_widget_controller_schema(&client).await;
    register_gadget_controller_schema(&client).await;

    let kapi_client = KapiClient::new(&base_url)?;
    let widget_count = Arc::new(AtomicU32::new(0));
    let gadget_count = Arc::new(AtomicU32::new(0));

    let mut manager = Manager::new(kapi_client);

    // Widget controller uses PanickingReconciler — panics on first call
    manager
        .controller_for(widget_key())
        .reconcile_with(PanickingReconciler { call_count: widget_count.clone() })
        .namespace(DEFAULT_NS)
        .register();

    // Gadget controller uses CountingReconciler — should keep working
    manager
        .controller_for(gadget_key())
        .reconcile_with(CountingReconciler { call_count: gadget_count.clone() })
        .namespace(DEFAULT_NS)
        .register();

    let shutdown_tx = manager.shutdown_sender();
    let mgr_handle = tokio::spawn(async move { manager.start().await });

    // Let controllers establish watch connections
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Create a Gadget object first — the Gadget controller should reconcile
    // successfully (proving it works before any Widget panic).
    let body = serde_json::json!({
        "metadata": { "name": "panic-gadget" },
        "spec": { "name": "gadget-panic", "weight": 100 },
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Gadget", body).await;
    assert_status(&resp, StatusCode::CREATED);

    // Wait for the Gadget controller to reconcile.
    let ok = tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            if gadget_count.load(Ordering::SeqCst) > 0 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        ok.is_ok(),
        "Gadget controller was not called within timeout (gadget_count={})",
        gadget_count.load(Ordering::SeqCst),
    );

    // Now create a Widget object — this will trigger the PanickingReconciler.
    let body = serde_json::json!({
        "metadata": { "name": "panic-widget" },
        "spec": { "color": "red", "size": 10 },
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);

    // Give time for the Widget panic to be triggered
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify the Widget controller was called (it should have panicked).
    assert!(
        widget_count.load(Ordering::SeqCst) > 0,
        "Widget controller should have been called (and panicked)"
    );

    // Create another Gadget object to prove the Gadget controller is still
    // functioning despite the Widget controller's panic.
    let body = serde_json::json!({
        "metadata": { "name": "panic-gadget-2" },
        "spec": { "name": "gadget-after-panic", "weight": 200 },
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Gadget", body).await;
    assert_status(&resp, StatusCode::CREATED);

    // Wait for the Gadget controller to reconcile the second object.
    let ok = tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            if gadget_count.load(Ordering::SeqCst) >= 2 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        ok.is_ok(),
        "Gadget controller did not reconcile after Widget panic (gadget_count={})",
        gadget_count.load(Ordering::SeqCst),
    );

    // Shutdown
    let _ = shutdown_tx.send(());
    let mgr_result = tokio::time::timeout(Duration::from_secs(5), mgr_handle).await;
    assert!(mgr_result.is_ok(), "manager did not shut down within timeout");

    Ok(())
}
