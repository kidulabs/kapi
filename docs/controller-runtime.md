# Controller Runtime SDK

The `kapi-controller` crate provides a framework for writing controllers that
watch kapi resources and reconcile desired state. It follows the
**watch → enqueue → reconcile** pattern popularised by the Kubernetes
controller-runtime project.

```text
┌──────────┐   SSE watch   ┌──────────┐   QueueKey   ┌────────────┐
│ kapi     │ ─────────────→ │ WorkQueue │ ────────────→ │ Reconciler │
│ server   │                │           │              │  (user)    │
│          │ ←── events ─── │ (dedup /  │              │            │
│          │                │  backoff) │              │ get/update │
│          │                └──────────┘              │ status     │
│          │                                           └──────┬─────┘
│          │                                                  │
│          │ ←─────────── HTTP API calls ──────────────────── │
└──────────┘
```

### Dependencies

```toml
# Cargo.toml
[dependencies]
kapi-controller = { path = "../kapi-controller" }
```

The crate re-exports key types from `kapi-core` and `kapi-client` via its
dependencies, so you typically need only:

```rust
use kapi_controller::prelude::*;
```

For now, import directly from the source crates:

```rust
use kapi_core::{ResourceKey, ObjectMeta, StoredObject, WatchFilter};
use kapi_client::client::KapiClient;
use kapi_controller::reconciler::{Reconciler, ReconcileContext, ReconcileResult};
use kapi_controller::controller::Controller;
use kapi_controller::finalizer::{is_deleting, ensure_finalizer, remove_finalizer, typed};
```

---

## Key Concepts

### Reconciler Trait

The [`Reconciler`] trait is the heart of the SDK. You implement it with your
business logic:

```rust
#[async_trait]
pub trait Reconciler: Send + Sync {
    async fn reconcile(
        &self,
        ctx: ReconcileContext,
    ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>>;
}
```

- **`ReconcileContext`** contains a `ReconcileRequest` (identifying the object)
  and a `KapiClient` for making API calls.
- Return [`ReconcileResult::default()`] to signal success without re-queuing.
- Return a `ReconcileResult` with `requeue_after:
  Some(Duration::from_secs(30))` to re-process the object after a delay, or
  `Some(Duration::ZERO)` to re-queue immediately.
- Return `Err(...)` to signal a transient failure — the controller
  automatically re-queues with exponential backoff.

### Controller

[`Controller`] orchestrates the watch-then-reconcile loop for a single resource
kind:

```rust
let controller = Controller::new(key, Arc::new(MyReconciler), client)
    .namespace("default")
    .shutdown_signal(rx);

controller.start().await;
```

**Builder methods:**

| Method            | Description                                              |
|-------------------|----------------------------------------------------------|
| `namespace(ns)`   | Watch only objects in the given namespace.               |
| `watch_filter(f)` | Apply a label/field selector filter on the watch stream. |
| `shutdown_signal(rx)` | Graceful shutdown via `broadcast::Receiver<()>`.     |

**Internals:**

1. Spawns a background task that opens an SSE watch stream.
2. Watch events (`Added`, `Modified`, `Deleted`) are pushed into a
   [`WorkQueue`]. `StatusModified` events are filtered out.
3. When the watch stream drops (e.g. network error), the task performs a full
   re-list and enqueues every object, ensuring no changes are missed.
4. The reconcile loop (on the current task) dequeues keys one at a time,
   fetches the object, and calls [`Reconciler::reconcile()`].

### WorkQueue

[`WorkQueue`] is a FIFO queue with **deduplication** and **exponential
backoff**:

| Method                | Description                                           |
|-----------------------|-------------------------------------------------------|
| `add(key)`            | Enqueue a key (no-op if already pending).             |
| `get()`               | Block until a key is available.                       |
| `done(key, success)`  | Mark processed. On failure, re-queue after backoff.   |
| `requeue_after(key, duration)` | Re-queue after a custom delay.               |

Backoff sequence: 1 s, 2 s, 4 s, 8 s, …, capped at 5 min.

### Finalizer Helpers

The controller SDK provides two layers of finalizer support:

**Typed lifecycle inspection** (on `TypedResource` trait):

| Method | Description |
|--------|-------------|
| `resource.is_deleting()` | Returns `true` when `system.deletion_timestamp` is set. |
| `resource.has_finalizer(name)` | Returns `true` when the named finalizer is present. |
| `resource.to_stored_object()` | Converts the typed resource to `StoredObject` for raw APIs. |

These methods let typed controllers inspect deletion state and finalizer membership without a second API fetch.

**Finalizer mutation helpers** (in `kapi_controller::finalizer`):

| Function | Description |
|----------|-------------|
| `is_deleting(obj)` | Returns `true` when `deletion_timestamp` is set (standalone function). |
| `ensure_finalizer(client, obj, finalizer)` | Adds `finalizer` to the object (CAS retry on 409). Raw variant takes `&StoredObject`. |
| `remove_finalizer(client, obj, finalizer)` | Removes `finalizer` from the object (CAS retry). Raw variant takes `&StoredObject`. |
| `typed::ensure_finalizer(client, obj, finalizer)` | Typed variant of `ensure_finalizer` — takes `&T where T: TypedResource`. |
| `typed::remove_finalizer(client, obj, finalizer)` | Typed variant of `remove_finalizer` — takes `&T where T: TypedResource`. |

All of these functions use **optimistic concurrency** (CAS). On a `409 Conflict`
they re-fetch the object and retry (up to 5 attempts).

The typed helpers live in a `typed` submodule because Rust does not allow
same-name overloads. Each accepts any `&T where T: TypedResource`, calls
`obj.to_stored_object()` once internally, and delegates to the raw
`&StoredObject` helper — so typed controllers never need to convert the
resource themselves. Note that the CAS re-fetch after a `409 Conflict`
operates on a `StoredObject`, never `T`; the passed-in `&T` is never updated
in place.

**When to use each:**

- Use `resource.is_deleting()` and `resource.has_finalizer()` for **inspecting** lifecycle state in typed controllers.
- **Typed controllers**: use `finalizer::typed::ensure_finalizer(&ctx.client, resource, FINALIZER_NAME)` and `finalizer::typed::remove_finalizer(&ctx.client, resource, FINALIZER_NAME)` directly for **mutating** finalizers — no `to_stored_object()` call needed.
- **Raw clients**: continue using `finalizer::ensure_finalizer(&ctx.client, &stored, FINALIZER_NAME)` and `finalizer::remove_finalizer(&ctx.client, &stored, FINALIZER_NAME)` with a `&StoredObject`.

This separation keeps the typed API simple while preserving the robust optimistic-concurrency behavior of the raw helpers.

### Manager

[`Manager`] orchestrates multiple controllers in a single process with shared
resources and coordinated lifecycle:

```rust
let client = KapiClient::new("http://localhost:8080")?;
let mut manager = Manager::new(client);

manager.controller_for(pod_key)
    .reconcile_with(PodReconciler)
    .namespace("default")
    .register();

manager.controller_for(node_key)
    .reconcile_with(NodeReconciler)
    .register();

manager.start().await?;
```

**Builder methods:**

| Method | Description |
|--------|-------------|
| `controller_for(key)` | Returns a [`ControllerBuilder`] for the given resource kind. |
| `shutdown_sender()` | Returns a clone of the shutdown broadcast sender. |
| `start()` | Starts all controllers and waits for shutdown signal. |

**ControllerBuilder methods:**

| Method | Description |
|--------|-------------|
| `reconcile_with(reconciler)` | Sets the reconciler implementation. |
| `namespace(ns)` | Restricts the controller to a specific namespace. |
| `register()` | Finalizes and adds the controller to the Manager. |

**Lifecycle:**

1. `Manager::start()` spawns a signal handler for SIGTERM/SIGINT.
2. Each registered controller is started as an independent tokio task.
3. On shutdown signal, all controllers receive the shutdown broadcast.
4. Manager waits up to 30 seconds for in-flight reconciles to complete.
5. If the timeout expires, the process force-exits with a warning.
6. Panics in individual controllers are caught and logged -- other controllers continue.

---

## Examples

### Example 1: Simple Reconciler

This controller watches a `Widget` kind, logs every reconciliation, and updates
the object's status with a "last reconciled" timestamp.

```rust
use std::sync::Arc;
use std::time::Duration;

use kapi_client::client::KapiClient;
use kapi_controller::reconciler::{Reconciler, ReconcileContext, ReconcileResult};
use kapi_controller::controller::Controller;
use kapi_core::ResourceKey;
use serde_json::json;
use tokio::sync::broadcast;

// ------------------------------------------------------------------
// Reconciler implementation
// ------------------------------------------------------------------

struct WidgetReconciler;

#[async_trait::async_trait]
impl Reconciler for WidgetReconciler {
    async fn reconcile(
        &self,
        ctx: ReconcileContext,
    ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>> {
        let obj = ctx
            .client
            .get(&ctx.request.key, ctx.request.namespace.as_deref(), &ctx.request.name)
            .await?;

        tracing::info!(
            kind = %obj.key.kind,
            name = %obj.metadata.name,
            namespace = ?obj.metadata.namespace,
            resource_version = obj.system.resource_version,
            "reconciling widget",
        );

        // Update the status sub-resource with a "last reconciled" timestamp.
        ctx.client
            .update_status(
                &ctx.request.key,
                ctx.request.namespace.as_deref(),
                &ctx.request.name,
                &json!({
                    "lastReconciled": chrono::Utc::now().to_rfc3339(),
                }),
            )
            .await?;

        // Return default = success, no re-queue.
        Ok(ReconcileResult::default())
    }
}

// ------------------------------------------------------------------
// Wiring
// ------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let client = KapiClient::new("http://localhost:8080")?;

    let key = ResourceKey {
        group: "example.io".into(),
        version: "v1".into(),
        kind: "Widget".into(),
    };

    let (_tx, rx) = broadcast::channel::<()>(1);

    let controller = Controller::new(key, Arc::new(WidgetReconciler), client)
        .namespace("default")
        .shutdown_signal(rx);

    tracing::info!("starting widget controller");
    controller.start().await;

    Ok(())
}
```

### Example 2: Reconciler with Finalizer Cleanup

This controller watches a `Widget` kind, adds a finalizer on creation, performs
cleanup when the object is marked for deletion, and removes the finalizer to
allow hard-deletion. It uses the typed lifecycle helpers for inspection and the
typed finalizer mutation helpers (`finalizer::typed`) to avoid a second API
fetch and any manual `to_stored_object()` conversion.

```rust
use std::sync::Arc;
use std::time::Duration;

use kapi_client::client::KapiClient;
use kapi_client::typed::{TypedClient, TypedResource};
use kapi_controller::reconciler::{Reconciler, ReconcileContext, ReconcileResult};
use kapi_controller::controller::Controller;
use kapi_controller::finalizer;
use kapi_core::{ObjectMeta, ResourceKey};
use serde_json::json;
use tokio::sync::broadcast;

const FINALIZER_NAME: &str = "widgets.example.io/cleanup";

// ------------------------------------------------------------------
// Reconciler implementation
// ------------------------------------------------------------------

struct WidgetCleanupReconciler;

#[async_trait::async_trait]
impl Reconciler for WidgetCleanupReconciler {
    async fn reconcile(
        &self,
        ctx: ReconcileContext,
    ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>> {
        // Fetch the typed resource (one API call).
        let typed_client = TypedClient::<Widget>::new(ctx.client.clone());
        let widget = typed_client
            .get(ctx.request.namespace.as_deref(), &ctx.request.name)
            .await?;

        // ── Case 1: Object is being deleted ───────────────────────
        // Use typed lifecycle helper instead of raw is_deleting(&obj).
        if widget.is_deleting() {
            // Perform external cleanup (e.g. release cloud resources).
            tracing::info!(
                name = %widget.metadata().name,
                "performing finalizer cleanup for widget",
            );

            // Simulate cleanup work.
            tokio::time::sleep(Duration::from_secs(1)).await;

            // Remove our finalizer so the object can be hard-deleted.
            // The typed helper converts to StoredObject internally and
            // delegates to the raw finalizer::remove_finalizer.
            finalizer::typed::remove_finalizer(&ctx.client, &widget, FINALIZER_NAME).await?;

            tracing::info!(
                name = %widget.metadata().name,
                "cleanup complete, finalizer removed",
            );

            return Ok(ReconcileResult::default());
        }

        // ── Case 2: Object is alive — ensure finalizer is set ────
        finalizer::typed::ensure_finalizer(&ctx.client, &widget, FINALIZER_NAME).await?;

        tracing::info!(
            name = %widget.metadata().name,
            "finalizer ensured on widget",
        );

        // Optionally update status.
        ctx.client
            .update_status(
                &ctx.request.key,
                ctx.request.namespace.as_deref(),
                &ctx.request.name,
                &json!({ "phase": "ready" }),
            )
            .await?;

        Ok(ReconcileResult::default())
    }
}

// ------------------------------------------------------------------
// Wiring
// ------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let client = KapiClient::new("http://localhost:8080")?;

    let key = ResourceKey {
        group: "example.io".into(),
        version: "v1".into(),
        kind: "Widget".into(),
    };

    let (_tx, rx) = broadcast::channel::<()>(1);

    let controller = Controller::new(key, Arc::new(WidgetCleanupReconciler), client)
        .namespace("default")
        .shutdown_signal(rx);

    tracing::info!("starting widget controller with finalizer support");
    controller.start().await;

    Ok(())
}
```

### Example 3: Running Multiple Controllers with Manager

This example shows how to run multiple controllers in one process using the
[`Manager`].

```rust
use std::sync::Arc;
use kapi_client::client::KapiClient;
use kapi_controller::manager::Manager;
use kapi_controller::reconciler::{Reconciler, ReconcileContext, ReconcileResult};
use kapi_core::ResourceKey;

struct PodReconciler;

#[async_trait::async_trait]
impl Reconciler for PodReconciler {
    async fn reconcile(
        &self,
        ctx: ReconcileContext,
    ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!(name = %ctx.request.name, "reconciling pod");
        Ok(ReconcileResult::default())
    }
}

struct NodeReconciler;

#[async_trait::async_trait]
impl Reconciler for NodeReconciler {
    async fn reconcile(
        &self,
        ctx: ReconcileContext,
    ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!(name = %ctx.request.name, "reconciling node");
        Ok(ReconcileResult::default())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let client = KapiClient::new("http://localhost:8080")?;
    let mut manager = Manager::new(client);

    let pod_key = ResourceKey {
        group: "example.io".into(),
        version: "v1".into(),
        kind: "Pod".into(),
    };
    let node_key = ResourceKey {
        group: "example.io".into(),
        version: "v1".into(),
        kind: "Node".into(),
    };

    manager.controller_for(pod_key)
        .reconcile_with(PodReconciler)
        .namespace("default")
        .register();

    manager.controller_for(node_key)
        .reconcile_with(NodeReconciler)
        .register();

    tracing::info!("starting controller manager");
    manager.start().await?;

    Ok(())
}
```

---

## API Reference

### `Reconciler` Trait

```rust
#[async_trait]
pub trait Reconciler: Send + Sync {
    async fn reconcile(
        &self,
        ctx: ReconcileContext,
    ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>>;
}
```

### `ReconcileRequest`

```rust
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ReconcileRequest {
    pub key: ResourceKey,           // group/version/kind
    pub name: String,               // object name
    pub namespace: Option<String>,   // None for cluster-scoped objects
}
```

### `ReconcileContext`

```rust
#[derive(Debug)]
pub struct ReconcileContext {
    pub request: ReconcileRequest,   // identifies the object
    pub client: KapiClient,          // authenticated HTTP client
}
```

### `ReconcileResult`

```rust
#[derive(Debug, Clone, Default)]
pub struct ReconcileResult {
    pub requeue_after: Option<Duration>,  // minimum delay before re-queue
}
```

### `Controller`

```rust
impl Controller {
    pub fn new(
        key: ResourceKey,
        reconciler: Arc<dyn Reconciler>,
        client: KapiClient,
    ) -> Self;

    pub fn namespace(self, ns: impl Into<String>) -> Self;
    pub fn watch_filter(self, filter: WatchFilter) -> Self;
    pub fn shutdown_signal(self, rx: broadcast::Receiver<()>) -> Self;
    pub async fn start(&self);
}
```

### `QueueKey`

```rust
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct QueueKey {
    pub key: ResourceKey,
    pub name: String,
    pub namespace: Option<String>,
}

impl QueueKey {
    pub fn new(key: ResourceKey, name: impl Into<String>, namespace: Option<String>) -> Self;
}
```

### `WorkQueue`

```rust
impl WorkQueue {
    pub fn new() -> Self;
    pub async fn add(&self, key: QueueKey);
    pub async fn get(&self) -> QueueKey;
    pub async fn done(&self, key: QueueKey, success: bool);
    pub async fn requeue_after(&self, key: QueueKey, duration: Duration);
}
```

### Finalizer Helpers

**Raw helpers** — operate on `&StoredObject`:

```rust
pub fn is_deleting(obj: &StoredObject) -> bool;

pub async fn ensure_finalizer(
    client: &KapiClient,
    obj: &StoredObject,
    finalizer: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

pub async fn remove_finalizer(
    client: &KapiClient,
    obj: &StoredObject,
    finalizer: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
```

**Typed helpers** — accept any `&T where T: TypedResource`. They call
`obj.to_stored_object()` once and delegate to the raw helpers above, so typed
controllers never need to convert the resource themselves. Because the CAS
retry loop always works on `StoredObject`, the re-fetch after a `409 Conflict`
returns a `StoredObject`, never `T` — the passed-in `&T` is not updated in
place. The typed variants live in a `typed` submodule since Rust does not allow
same-name overloads:

```rust
pub mod typed {
    pub async fn ensure_finalizer<T: TypedResource>(
        client: &KapiClient,
        obj: &T,
        finalizer: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    pub async fn remove_finalizer<T: TypedResource>(
        client: &KapiClient,
        obj: &T,
        finalizer: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
```

### `Manager`

```rust
impl Manager {
    pub fn new(client: KapiClient) -> Self;
    pub fn controller_for(&mut self, key: ResourceKey) -> ControllerBuilder<'_>;
    pub fn shutdown_sender(&self) -> broadcast::Sender<()>;
    pub async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
```

### `ControllerBuilder`

```rust
impl<'a> ControllerBuilder<'a> {
    pub fn reconcile_with(self, reconciler: impl Reconciler + 'static) -> Self;
    pub fn namespace(self, ns: impl Into<String>) -> Self;
    pub fn register(self);
}
```
