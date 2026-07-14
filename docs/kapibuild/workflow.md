# Complete Workflow

## Overview

This guide walks through the complete lifecycle of a kapi controller project:
from initial scaffolding to running a controller that watches and reconciles
resources.

## Step-by-Step

### 1. Initialize the Project

```bash
kapibuild init --name my-controller
```

This creates:

```
my-controller/
├── Cargo.toml          # Project manifest (edition 2021)
├── Kapifile            # Project metadata (YAML)
├── api/                # API type definitions
├── schemas/            # Generated JSON schemas
└── src/
    ├── main.rs         # Entry point with Manager setup
    └── controllers/    # Controller implementations
        └── mod.rs
```

### 2. Create an API Resource

```bash
kapibuild create api \
    --group example.io \
    --version v1 \
    --kind Widget \
    --status \
    --controller
```

This generates:

- `api/example.io/v1/widget.rs` — Spec and Status structs with `#[derive(KapiResource)]`
- `api/example.io/v1/mod.rs` — Version module
- `api/example.io/mod.rs` — Group module
- `src/controllers/widget_controller.rs` — Controller skeleton with finalizer pattern
- Updates `src/controllers/mod.rs` — Exports the new controller module
- Updates `src/main.rs` — Wires the controller to the Manager
- Updates `Kapifile` — Adds resource metadata

### 3. Edit the API Types

Open `api/example.io/v1/widget.rs` and define your spec and status fields:

```rust
#[derive(Debug, Clone, KapiResource, Serialize, Deserialize, JsonSchema)]
#[kapi(group = "example.io", version = "v1", kind = "Widget", status = "WidgetStatus")]
pub struct WidgetSpec {
    pub color: String,

    #[schemars(range(min = 1, max = 100))]
    pub replicas: u32,

    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WidgetStatus {
    pub phase: String,
    pub observed_replicas: u32,
}
```

### 4. Generate the Schema

```bash
kapibuild generate
```

This parses your types and generates `schemas/example.io_Widget.json`:

```json
{
    "targetGroup": "example.io",
    "targetVersion": "v1",
    "targetKind": "Widget",
    "scope": "Namespaced",
    "specSchema": { /* JSON Schema derived from WidgetSpec */ },
    "statusSchema": { /* JSON Schema derived from WidgetStatus */ }
}
```

### 5. Edit the Controller

Open `src/controllers/widget_controller.rs` and implement your business logic:

```rust
use async_trait::async_trait;
use kapi_controller::reconciler::{ReconcileContext, ReconcileResult, Reconciler};
use kapi_core::StoredObject;
use tracing::info;

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

        let obj = ctx.client.get(&req.key, req.namespace.as_deref(), &req.name).await?;

        // Deserialize spec
        let spec: WidgetSpec = serde_json::from_value(obj.spec.clone())?;
        info!("  color={}, replicas={}", spec.color, spec.replicas);

        // Update status
        let mut status = WidgetStatus {
            phase: "Running".to_string(),
            observed_replicas: spec.replicas,
        };
        ctx.client.update_status(&req.key, req.namespace.as_deref(), &req.name, &status).await?;

        Ok(ReconcileResult::default())
    }
}
```

### 6. Apply the Schema to the Server

Start the kapi server:

```bash
# Terminal 1: Start the server
cargo run -p kapi-server
```

Register your schema using the `kapi` CLI:

```bash
# Terminal 2: Register the Widget schema
kapi apply -f schemas/example.io_Widget.json
```

### 7. Create Objects

Create a manifest file for your Widget:

```bash
cat > widget.yaml <<EOF
apiVersion: example.io/v1
kind: Widget
metadata:
  name: my-widget
spec:
  color: blue
  replicas: 3
EOF
```

Apply it using the `kapi` CLI:

```bash
kapi apply -f widget.yaml
```

List your widgets:

```bash
kapi get Widget
```

### 8. Run the Controller

```bash
cargo run -p my-controller
```

The controller connects to the kapi server, starts watching for Widget events,
and reconciles each object.

Watch for changes in another terminal:

```bash
kapi watch Widget
```

## Iterating on Schema Changes

When you modify your types, repeat steps 4-6:

```bash
# 1. Edit {kind}.rs (e.g., api/example.io/v1/widget.rs)
# 2. Regenerate schema
kapibuild generate

# 3. Re-register schema (delete + apply)
kapi delete Schema example.io_Widget
kapi apply -f schemas/example.io_Widget.json
```

> **Note**: Schema updates require delete + recreate. The kapi server does not
> support in-place schema updates yet.

## Command Reference

### kapibuild Commands

| Command                                           | Description                           |
|---------------------------------------------------|---------------------------------------|
| `kapibuild init --name <name>`                    | Scaffold a new controller project     |
| `kapibuild create api --group <g> --version <v> --kind <k> [--status] [--controller]` | Add a new API resource |
| `kapibuild create controller --group <g> --version <v> --kind <k>` | Add a controller for existing API |
| `kapibuild generate`                              | Generate JSON Schema from Rust types  |

### kapi CLI Commands

| Command                                           | Description                           |
|---------------------------------------------------|---------------------------------------|
| `kapi apply -f <file>`                            | Create or update a resource from file |
| `kapi get <kind> [name]`                          | List or get resources                 |
| `kapi delete <kind> <name>`                       | Delete a resource                     |
| `kapi watch <kind>`                               | Watch for resource events             |
| `kapi edit <kind> <name>`                         | Edit a resource in your editor        |
| `kapi status get <kind> <name>`                   | Get the status of a resource          |
| `kapi status apply <kind> <name> -f <file>`       | Apply a status update from file       |
