# Troubleshooting Guide

## Common Compilation Errors

> **Note:** The `KapiResource` derive macro and `#[kapi(...)]` attributes described below are not yet implemented. The current approach uses `Kapifile` for resource metadata instead.

### "the trait bound `WidgetSpec: Clone` is not satisfied"

The `KapiResource` derive macro generates a wrapper struct that derives `Clone`.
The spec struct must also implement `Clone`:

```rust
// ❌ Missing Clone
#[derive(KapiResource, Serialize, Deserialize, JsonSchema)]
pub struct WidgetSpec { ... }

// ✅ Add Clone
#[derive(Debug, Clone, KapiResource, Serialize, Deserialize, JsonSchema)]
pub struct WidgetSpec { ... }
```

### "the trait bound `WidgetSpec: Debug` is not satisfied"

Same issue for `Debug` — the generated wrapper derives `Debug`:

```rust
// ✅ Add Debug
#[derive(Debug, Clone, KapiResource, Serialize, Deserialize, JsonSchema)]
pub struct WidgetSpec { ... }
```

### "invalid scope 'Invalid': must be 'Namespaced' or 'Cluster'"

The `scope` attribute only accepts `"Namespaced"` or `"Cluster"`:

```rust
// ❌ Invalid
#[kapi(..., scope = "Global")]

// ✅ Valid
#[kapi(..., scope = "Namespaced")]
#[kapi(..., scope = "Cluster")]
```

### "cannot find macro `schemars`"

The `schema_data()` method calls `schemars::schema_for!()`. If schemars is not
in scope, add the dependency and import:

```toml
[dependencies]
schemars = "0.8"
```

### "failed to resolve: could not find `kapi_core` in the crate root"

The generated code references `kapi_core` types directly. Add `kapi-core` as a
dependency:

```toml
[dependencies]
kapi-core = { path = "../kapi-core" }
```

## Schema Validation Errors

### "Validation failed: ... is required"

The API request is missing a required field. Check:
- Is the field wrapped in `Option<T>`? If not, it's required.
- Did you use `#[serde(default)]` to make it optional?
- Check the generated schema in `schemas/` to see which fields are in `required`.

### "Validation failed: ... is not of type ..."

Type mismatch between the request and the schema. Common causes:
- Sending a string where a number is expected (e.g., `"3"` vs `3`)
- Sending an integer where a float is expected
- Missing quotes around string values

### "Validation failed: numeric instance is greater than the required maximum"

The value exceeds `#[schemars(range(max = ...))]`. Increase the range or fix the
request:

```rust
// Increase max replicas
#[schemars(range(min = 1, max = 1000))]
pub replicas: u32,
```

### "Validation failed: string instance is longer than the required length"

The string exceeds `#[schemars(length(max = ...))]`. Increase the limit or fix
the request.

## Controller Not Receiving Events

### Check Server Connection

Verify the controller can reach the kapi server:

```bash
curl http://localhost:8080/apis/example.io/v1/namespaces/default/Widget
```

If this fails, check:
- Is the kapi server running?
- Is the URL in `src/main.rs` correct?
- Is there a firewall or proxy issue?

### Verify Schema is Registered

The controller only receives events for registered schemas:

```bash
# Check if schema exists
curl http://localhost:8080/apis/kapi.io/v1/Schema | jq '.'
```

Look for your resource's group/version/kind. If missing, register the schema:

```bash
curl -X POST http://localhost:8080/apis/kapi.io/v1/Schema \
    -H "Content-Type: application/json" \
    -d @schemas/example.io_Widget.json
```

### Check Namespace

Namespaced resources require the namespace to exist:

```bash
# List namespaces
curl http://localhost:8080/apis/kapi.io/v1/Namespace

# Create namespace if needed
curl -X POST http://localhost:8080/apis/kapi.io/v1/Namespace \
    -H "Content-Type: application/json" \
    -d '{"metadata": {"name": "default"}}'
```

### Enable Tracing

Add more verbose logging to see what the controller is doing:

```rust
// In src/main.rs
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();
```

## Finalizer Not Being Removed

### Check Finalizer Name

The finalizer name must match exactly between `ensure_finalizer` and
`remove_finalizer`:

```rust
const FINALIZER_NAME: &str = "widgets.example.io/cleanup";
//                                             ^^ Must match exactly
```

### Verify Deletion Detection

The `is_deleting()` function checks for `deletion_timestamp` on the object.
If the object was created before the finalizer was added, it may not have a
deletion timestamp:

```rust
// Check if deletion timestamp is set manually
if obj.system.deletion_timestamp.is_some() {
    // Object is being deleted
}
```

### Race Condition

If the controller restarts during deletion, the finalizer might already be
removed but the object is still pending cleanup. Check the object directly:

```bash
curl http://localhost:8080/apis/example.io/v1/namespaces/default/Widget/my-widget
```

Look at `metadata.finalizers` and `system.deletionTimestamp`.

## Status Not Updating

### Status Subresource Not Registered

The schema must include `statusSchema` for the status endpoint to work. Check
if your resource was registered with a status schema:

```bash
curl http://localhost:8080/apis/kapi.io/v1/Schema | jq '.items[].spec.statusSchema'
```

If null, delete and re-create the schema with status included.

### Wrong HTTP Method

Status updates use `PUT`, not `PATCH`. The `update_status()` method handles this
correctly, but direct `curl` calls must use:

```bash
curl -X PUT http://localhost:8080/apis/example.io/v1/namespaces/default/Widget/my-widget/status \
    -H "Content-Type: application/json" \
    -d '{"phase": "Running", "observedReplicas": 3}'
```

## kapibuild controller generate Errors

### "API resource 'Widget' not found. Run 'kapibuild api create ...' first."

The `controller generate` command requires that the API resource already exists:
1. The file `api/<group>/<version>/<kind>.rs` must exist
2. The kind must be registered in `Kapifile`

Run `kapibuild api create --group <group> --version <version> --kind <kind>` first, then `kapibuild api generate` to create the typed wrappers.

### "Controller 'widget_controller.rs' already exists"

The controller file already exists. The command does not overwrite existing controllers. Either:
- Delete the existing file if you want to regenerate
- Edit the existing file directly

## How to Debug Issues

### 1. Enable Trace Logging

Set the logging level to DEBUG or TRACE:

```rust
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::TRACE)
    .init();
```

### 2. Check Server Logs

The kapi server logs all requests and validation errors. Look for:

```
[ERROR] validation failed: ...
[INFO] POST /apis/example.io/v1/namespaces/default/Widget
[WARN] schema not found for ...
```

### 3. Inspect Raw API Responses

Use `curl -v` to see full request/response details:

```bash
curl -v -X POST http://localhost:8080/apis/example.io/v1/namespaces/default/Widget \
    -H "Content-Type: application/json" \
    -d '{"metadata": {"name": "test"}, "spec": {}}'
```

### 4. Check Generated Schema

Examine the generated schema file for correctness:

```bash
cat schemas/example.io_Widget.json | jq '.'
```

Look for:
- Missing fields in `properties`
- Incorrectly required fields
- Wrong `type` annotations

### 5. Verify the Derive Macro Output

If the derive macro is generating unexpected code, check the expanded output:

```bash
cargo expand -p my-controller api::example::io::v1
```

This shows the generated wrapper struct and method implementations.
