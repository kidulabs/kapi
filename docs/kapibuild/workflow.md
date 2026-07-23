# Complete Workflow

## Overview

This guide walks through the complete lifecycle of a kapi controller project:
from initial scaffolding to running a controller that watches and reconciles
resources.

## Step-by-Step

### 1. Initialize the Project

```bash
kapibuild init my-controller
```

This creates:

```
my-controller/
├── Cargo.toml          # Project manifest (edition 2024)
├── Kapifile            # Project metadata (YAML)
├── schemas/            # Generated JSON schemas
└── src/
    ├── main.rs         # Entry point with Manager setup
    ├── api/             # API type definitions (by `kapibuild api create`)
    ├── types/           # Generated typed wrappers (by `kapibuild api generate`)
    └── controllers/    # Controller implementations
        └── mod.rs
```

### 2. Create an API Resource

```bash
kapibuild api create \
    --group example.io \
    --version v1 \
    --kind Widget \
    --status
```

This generates:

- `src/api/example.io/v1/widget.rs` — Spec and Status structs
- Updates `Kapifile` — Adds resource metadata

### 3. Edit the API Types

Open `src/api/example.io/v1/widget.rs` and define your spec and status fields:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
kapibuild api generate
```

This parses your types and generates `schemas/example.io_Widget.json`:

```json
{
    "kind": "Schema",
    "apiVersion": "kapi.io/v1",
    "metadata": { "name": "Widget.example.io.v1" },
    "spec": {
        "targetGroup": "example.io",
        "targetVersion": "v1",
        "targetKind": "Widget",
        "scope": "Namespaced",
        "specSchema": { /* JSON Schema derived from WidgetSpec */ },
        "statusSchema": { /* JSON Schema derived from WidgetStatus */ }
    }
}
```

This command also generates typed wrapper structs in `src/types/`:

- `src/types/example_io/v1/widget.rs` — Wrapper struct `Widget` with `metadata`, `system`, `spec`, and optional `status` fields, plus `key()` and `schema_data()` methods
- `src/types/example_io/v1/widget.rs` — `TypedResource` impl for use with `TypedClient`
- `mod.rs` files for both `src/api/` and `src/types/` module trees (auto-generated)

The typed wrappers enable type-safe access in controllers:

```rust
use kapi_client::typed::TypedClient;

let typed_client = TypedClient::<Widget>::new(ctx.client.clone());
let widget = typed_client.get(namespace, &name).await?;
let spec = widget.spec(); // Returns &WidgetSpec
```

### 5. Generate the Controller

```bash
kapibuild controller generate \
    --group example.io \
    --version v1 \
    --kind Widget
```

This generates:

- `src/controllers/widget_controller.rs` — Controller skeleton with finalizer pattern and status update logic
- Updates `src/controllers/mod.rs` — Exports the new controller module
- Updates `src/main.rs` — Wires the controller to the Manager

You can also generate controllers for all resources at once by omitting the flags:

```bash
kapibuild controller generate
```

This scans all registered API resources and generates controllers for any that don't already have one, skipping resources that already have a controller file.

### 6. Edit the Controller

Open `src/controllers/widget_controller.rs` and implement your business logic. The generated skeleton uses `TypedClient` for type-safe operations:

```rust
use async_trait::async_trait;
use kapi_controller::reconciler::{ReconcileContext, ReconcileResult, Reconciler};
use kapi_controller::TypedClient;
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

        let typed_client = TypedClient::<Widget>::new(ctx.client.clone());
        let widget = typed_client.get(req.namespace.as_deref(), &req.name).await?;

        let spec = widget.spec();
        info!("  color={}, replicas={}", spec.color, spec.replicas);

        // Update status
        let status = WidgetStatus {
            phase: "Running".to_string(),
            observed_replicas: spec.replicas,
        };
        typed_client.inner()
            .update_status(&req.key, req.namespace.as_deref(), &req.name, &status)
            .await?;

        Ok(ReconcileResult::default())
    }
}
```

### 7. Apply the Schema to the Server

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

### 8. Create Objects

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

### 9. Run the Controller

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

When you modify your types, repeat steps 4 and 7:

```bash
# 1. Edit {kind}.rs (e.g., src/api/example.io/v1/widget.rs)
# 2. Regenerate schema
kapibuild api generate

# 3. Re-register schema (delete + apply)
kapi delete Schema example.io_Widget
kapi apply -f schemas/example.io_Widget.json
```

> **Note**: Schema updates require delete + recreate. The kapi server does not
> support in-place schema updates yet.

> **Note**: `kapibuild api generate` also regenerates the typed wrapper structs
> in `src/types/`, keeping them in sync with your API type definitions.

## Types-Only Workflow

If you only need kapibuild for type generation — you want the generated types and
schemas but plan to write controllers in a separate project — skip step 5
(`controller generate`):

```bash
# 1. Scaffold the types project
kapibuild init my-types
cd my-types

# 2. Create API resources
kapibuild api create --group example.io --version v1 --kind Widget --status

# 3. Edit your types
# (edit src/api/example.io/v1/widget.rs)

# 4. Generate schemas + typed wrappers
kapibuild api generate
```

This gives you a crate with:
- **`src/types/`** — Wrapper structs implementing `TypedResource`, ready for `TypedClient<T>`
- **`schemas/`** — JSON schema files for server registration

You can then depend on this crate from your controller project and use the
generated types directly. See [Controller Patterns](controller-patterns.md#using-kapibuild-as-a-types-library)
for the complete example.

## Command Reference

### kapibuild Commands

| Command                                           | Description                           |
|---------------------------------------------------|---------------------------------------|
| `kapibuild init <path>`                           | Scaffold a new controller project     |
| `kapibuild api create --group <g> --version <v> --kind <k> [--scope {Namespaced,Cluster}] [--status]` | Add a new API resource |
| `kapibuild api generate`                          | Generate JSON Schema and typed wrappers from Rust types |
| `kapibuild controller generate [--group <g> --version <v> --kind <k>]` | Generate controller scaffolding (specific resource or auto-discover all) |

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
