# Controller Patterns

## Overview

This guide covers common patterns for writing kapi controllers. Controllers
follow the **watch → enqueue → reconcile** pattern: they watch resources via
Server-Sent Events, enqueue keys into a work queue, and call the user's
reconciler for each key.

## Basic Controller Structure

```rust
use async_trait::async_trait;
use kapi_controller::reconciler::{ReconcileContext, ReconcileResult, Reconciler};
use tracing::info;

pub struct WidgetReconciler;

#[async_trait]
impl Reconciler for WidgetReconciler {
    async fn reconcile(
        &self,
        ctx: ReconcileContext,
    ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>> {
        let req = &ctx.request;
        info!("Reconciling {}/{}",
            req.namespace.as_deref().unwrap_or("cluster"),
            req.name);

        // ... reconciliation logic ...

        Ok(ReconcileResult::default())
    }
}
```

## Generating a Controller

The `kapibuild controller generate` command creates a controller skeleton automatically:

```bash
kapibuild controller generate --group example.io --version v1 --kind Widget
```

This produces:

- `src/controllers/widget_controller.rs` — A `WidgetReconciler` skeleton with finalizer pattern, status update logic (if the resource has a status subresource), and TODO comments where you add your business logic
- Updates `src/controllers/mod.rs` — Adds `pub mod widget_controller;`
- Updates `src/main.rs` — Wires `WidgetReconciler` to the Manager

The generated skeleton uses `TypedClient` for type-safe operations and includes:

```rust
pub struct WidgetReconciler;

#[async_trait]
impl Reconciler for WidgetReconciler {
    async fn reconcile(
        &self,
        ctx: ReconcileContext,
    ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>> {
        let typed_client = TypedClient::<Widget>::new(ctx.client.clone());

        // TODO: fetch and reconcile the object
        // let widget = typed_client.get(...).await?;

        Ok(ReconcileResult::default())
    }
}
```

After generation, replace the `// TODO` sections with your actual reconciliation logic.

## Using the Typed Client in Controllers

`TypedClient<T>` wraps the raw client to provide type-safe access to your resource:

```rust
use kapi_client::typed::TypedClient;

// Create a typed client for Widget
let typed_client = TypedClient::<Widget>::new(ctx.client.clone());

// Fetch an object — returns a typed wrapper with spec() and status() methods
let widget = typed_client.get(req.namespace.as_deref(), &req.name).await?;

// Access typed fields without manual deserialization
let spec = widget.spec();
info!("Color: {}, Replicas: {}", spec.color, spec.replicas);

// For status updates, use the inner client:
let status = WidgetStatus {
    phase: "Running".to_string(),
    observed_replicas: spec.replicas,
};
typed_client.inner()
    .update_status(&req.key, req.namespace.as_deref(), &req.name, &status)
    .await?;
```

The `TypedClient` methods mirror the raw client but return your specific types:

| Method                  | Description                                  |
|-------------------------|----------------------------------------------|
| `get(ns, name)`         | Fetch a single object                        |
| `list(ns)`              | List all objects in a namespace              |
| `inner()`               | Access the underlying raw client             |

## Fetching and Deserializing Objects

Use `ctx.client.get()` to fetch the latest version of an object, then deserialize
its `spec` field:

```rust
use kapi_core::StoredObject;

async fn reconcile(&self, ctx: ReconcileContext) -> Result<ReconcileResult, ...> {
    let req = &ctx.request;

    // Fetch the object from the API server
    let obj = ctx.client
        .get(&req.key, req.namespace.as_deref(), &req.name)
        .await?;

    // Deserialize the spec
    let spec: WidgetSpec = serde_json::from_value(obj.spec.clone())?;

    // Access spec fields
    info!("Widget color: {}", spec.color);
    info!("Replicas: {}", spec.replicas);

    Ok(ReconcileResult::default())
}
```

## Updating Status

Use `ctx.client.update_status()` to update the status subresource. The status
is a separate PUT endpoint that only modifies the `status` field:

```rust
async fn reconcile(&self, ctx: ReconcileContext) -> Result<ReconcileResult, ...> {
    let req = &ctx.request;
    let obj = ctx.client
        .get(&req.key, req.namespace.as_deref(), &req.name)
        .await?;

    let spec: WidgetSpec = serde_json::from_value(obj.spec.clone())?;

    // Compute the new status
    let status = WidgetStatus {
        phase: "Running".to_string(),
        observed_replicas: spec.replicas,
    };

    // Update status subresource
    ctx.client
        .update_status(&req.key, req.namespace.as_deref(), &req.name, &status)
        .await?;

    Ok(ReconcileResult::default())
}
```

## Finalizer Pattern

Finalizers prevent hard-deletion of objects until the controller has performed
cleanup. The workflow is:

1. **Add finalizer** during reconciliation of a healthy object
2. **Detect deletion** via `is_deleting()`
3. **Run cleanup logic**
4. **Remove finalizer** — the server then hard-deletes the object

```rust
use kapi_controller::finalizer::{ensure_finalizer, is_deleting, remove_finalizer};

const FINALIZER_NAME: &str = "widgets.example.io/cleanup";

impl Reconciler for WidgetReconciler {
    async fn reconcile(&self, ctx: ReconcileContext) -> Result<ReconcileResult, ...> {
        let req = &ctx.request;
        let obj = ctx.client
            .get(&req.key, req.namespace.as_deref(), &req.name)
            .await?;

        // Handle deletion
        if is_deleting(&obj) {
            info!("Widget is being deleted, running cleanup");

            // TODO: Perform actual cleanup (e.g., delete cloud resources)
            cleanup_external_resources(&spec).await?;

            // Remove finalizer to allow hard-deletion
            remove_finalizer(&ctx.client, &obj, FINALIZER_NAME).await?;
            return Ok(ReconcileResult::default());
        }

        // Ensure finalizer is present on every reconcile
        ensure_finalizer(&ctx.client, &obj, FINALIZER_NAME).await?;

        // Normal reconciliation...
        Ok(ReconcileResult::default())
    }
}
```

### Finalizer Functions

| Function            | Purpose                                              |
|---------------------|------------------------------------------------------|
| `ensure_finalizer`  | Adds the finalizer if not present (idempotent)       |
| `is_deleting`       | Checks if `deletion_timestamp` is set on the object  |
| `remove_finalizer`  | Removes the finalizer (triggers hard-delete if last) |

## Requeue Logic

Return `ReconcileResult` with a `requeue_after` duration to re-enqueue the key
for later reconciliation:

```rust
use std::time::Duration;

async fn reconcile(&self, ctx: ReconcileContext) -> Result<ReconcileResult, ...> {
    // If the resource isn't ready yet, requeue after 30 seconds
    if !is_resource_ready().await {
        return Ok(ReconcileResult {
            requeue_after: Some(Duration::from_secs(30)),
        });
    }

    Ok(ReconcileResult::default()) // No requeue
}
```

### Requeue Strategies

| Pattern          | Code                                                       |
|------------------|------------------------------------------------------------|
| No requeue       | `ReconcileResult::default()`                               |
| Fixed requeue    | `ReconcileResult { requeue_after: Some(Duration::from_secs(30)), }` |
| Exponential backoff | Handled automatically by the work queue                    |

## Error Handling

Return an `Err` to signal a transient failure. The work queue will retry with
exponential backoff:

```rust
async fn reconcile(&self, ctx: ReconcileContext) -> Result<ReconcileResult, ...> {
    let resp = ctx.client
        .get(&req.key, req.namespace.as_deref(), &req.name)
        .await
        .map_err(|e| format!("Failed to fetch object: {e}"))?;

    // Deserialization errors are permanent — panic or return Err
    let spec: WidgetSpec = serde_json::from_value(resp.spec.clone())
        .map_err(|e| format!("Failed to deserialize spec: {e}"))?;

    // Transient errors: return Err, controller will retry
    if !external_service_available().await {
        return Err("External service not available".into());
    }

    Ok(ReconcileResult::default())
}
```

## Complete Controller Example

Here is a complete controller that manages Widget resources with finalizer support
and status updates:

```rust
use async_trait::async_trait;
use kapi_controller::finalizer::{ensure_finalizer, is_deleting, remove_finalizer};
use kapi_controller::reconciler::{ReconcileContext, ReconcileResult, Reconciler};
use kapi_core::StoredObject;
use tracing::info;

const FINALIZER_NAME: &str = "widgets.example.io/cleanup";

pub struct WidgetReconciler;

#[async_trait]
impl Reconciler for WidgetReconciler {
    async fn reconcile(
        &self,
        ctx: ReconcileContext,
    ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>> {
        let req = &ctx.request;
        info!("Reconciling Widget {}/{}",
            req.namespace.as_deref().unwrap_or("cluster"),
            req.name);

        // Fetch the current state
        let obj = ctx.client
            .get(&req.key, req.namespace.as_deref(), &req.name)
            .await?;

        let spec: WidgetSpec = serde_json::from_value(obj.spec.clone())?;

        // Handle deletion
        if is_deleting(&obj) {
            info!("Widget is being deleted, cleaning up");
            cleanup_external_resources(&spec).await;
            remove_finalizer(&ctx.client, &obj, FINALIZER_NAME).await?;
            return Ok(ReconcileResult::default());
        }

        // Ensure finalizer
        ensure_finalizer(&ctx.client, &obj, FINALIZER_NAME).await?;

        // Reconcile: ensure external resources match spec
        reconcile_external_resources(&spec).await?;

        // Update status
        let status = WidgetStatus {
            phase: "Running".to_string(),
            observed_replicas: spec.replicas,
        };
        ctx.client
            .update_status(&req.key, req.namespace.as_deref(), &req.name, &status)
            .await?;

        info!("Widget reconciled successfully");
        Ok(ReconcileResult::default())
    }
}

async fn cleanup_external_resources(_spec: &WidgetSpec) {
    // TODO: Delete cloud resources, release external dependencies
    info!("Cleanup complete");
}

async fn reconcile_external_resources(_spec: &WidgetSpec) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // TODO: Create/update external resources to match spec
    Ok(())
}
```

## Watching Multiple Resource Types

A single controller can watch multiple resource types by registering multiple
controller instances on the Manager:

```rust
manager
    .controller_for(Widget::key())
    .reconcile_with(WidgetReconciler)
    .register();

manager
    .controller_for(Gadget::key())
    .reconcile_with(GadgetReconciler)
    .register();
```

Each reconciler runs independently with its own work queue and event watcher.

## Using kapibuild as a Types Library

You can use kapibuild to generate types and schemas in a dedicated crate, then
consume those types from a separate controller project. This lets you version and
publish your API types independently from your controller logic.

### Types Crate

Start by scaffolding a types-only project — run `init`, `api create`, and `api
generate`, but skip `controller generate`:

```bash
kapibuild init my-types
cd my-types

kapibuild api create \
    --group example.io \
    --version v1 \
    --kind Widget \
    --status

# Edit src/api/example.io/v1/widget.rs — define your Spec and Status fields

kapibuild api generate
```

This produces:

- `src/api/` — your Spec and Status structs (you edit these)
- `src/types/` — generated wrapper structs with `TypedResource` impls (auto-generated)
- `schemas/` — JSON schema files for server registration

The `src/types/` wrappers expose everything a controller needs: `Widget::key()`
for the `ResourceKey`, and the `TypedResource` trait for `TypedClient<Widget>`.

#### Re-exporting Types

To make imports clean for downstream crates, add a `src/lib.rs` that re-exports
the generated types:

```rust
// src/lib.rs
pub mod api;
pub mod types;

// Re-export for convenience
pub use types::example_io::v1::widget::Widget;
pub use api::example_io::v1::widget::{WidgetSpec, WidgetStatus};
```

### Controller Project

In your controller crate, depend on the types crate:

```toml
[dependencies]
my-types = { path = "../my-types" }
kapi-client = "0.1.0"
kapi-controller = "0.1.0"
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = "0.3"
async-trait = "0.1"
```

Then write your controller using the generated types:

```rust
use async_trait::async_trait;
use kapi_client::typed::TypedClient;
use kapi_controller::finalizer;
use kapi_controller::reconciler::{ReconcileContext, ReconcileResult, Reconciler};
use tracing::info;

use my_types::{Widget, WidgetSpec, WidgetStatus};

const FINALIZER_NAME: &str = "widgets.example.io/cleanup";

pub struct WidgetReconciler;

#[async_trait]
impl Reconciler for WidgetReconciler {
    async fn reconcile(
        &self,
        ctx: ReconcileContext,
    ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>> {
        let typed_client = TypedClient::<Widget>::new(ctx.client.clone());

        // Fetch the object using the typed client.
        let widget = typed_client
            .get(ctx.request.namespace.as_deref(), &ctx.request.name)
            .await?;

        // Handle deletion with finalizer pattern.
        let obj = ctx.client
            .get(&Widget::key(), ctx.request.namespace.as_deref(), &ctx.request.name)
            .await?;

        if finalizer::is_deleting(&obj) {
            info!("Widget is being deleted, running cleanup");
            // TODO: Add cleanup logic here.
            finalizer::remove_finalizer(&ctx.client, &obj, FINALIZER_NAME).await?;
            return Ok(ReconcileResult::default());
        }

        // Ensure finalizer is present.
        finalizer::ensure_finalizer(&ctx.client, &obj, FINALIZER_NAME).await?;

        // Reconcile: access typed spec directly.
        let spec = widget.spec();
        info!("Reconciling Widget: color={}, replicas={}", spec.color, spec.replicas);

        // Update status.
        let status = WidgetStatus {
            phase: "Running".to_string(),
            observed_replicas: spec.replicas,
        };
        typed_client
            .inner()
            .update_status(
                &Widget::key(),
                ctx.request.namespace.as_deref(),
                &ctx.request.name,
                &serde_json::to_value(&status)?,
            )
            .await?;

        Ok(ReconcileResult::default())
    }
}
```

Wire it up in main.rs:

```rust
use kapi_client::client::KapiClient;
use kapi_controller::manager::Manager;
use my_types::Widget;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt::init();

    let client = KapiClient::new("http://localhost:8080")?;
    let mut manager = Manager::new(client);

    manager
        .controller_for(Widget::key())
        .reconcile_with(WidgetReconciler)
        .register();

    manager.start().await?;

    Ok(())
}
```

### Project Layout

```
my-types/                  # Types crate (generated by kapibuild)
├── Cargo.toml
├── Kapifile
├── schemas/
│   └── example.io_Widget.json
└── src/
    ├── lib.rs             # Re-exports
    ├── api/               # Your Spec/Status definitions
    └── types/             # Generated TypedResource wrappers

my-controller/             # Controller crate (your code)
├── Cargo.toml             # depends on my-types
└── src/
    ├── main.rs            # Manager setup
    └── controller.rs      # WidgetReconciler
```

### Schema Registration

Before the controller can receive events, the schema must be registered with the
kapi server. You can register it from the types crate's generated schema file:

```bash
# Register schema from the types crate
kapi apply -f ../my-types/schemas/example.io_Widget.json
```

Or register it programmatically at controller startup using the schema JSON
embedded in your binary.
