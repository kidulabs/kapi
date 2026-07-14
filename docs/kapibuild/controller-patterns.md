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
