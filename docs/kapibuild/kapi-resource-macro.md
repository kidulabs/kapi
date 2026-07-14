# KapiResource Derive Macro

## Overview

The `#[derive(KapiResource)]` proc-macro generates the boilerplate wrapper struct and
associated methods for a kapi resource type. It converts a user-defined **spec** struct
into a complete resource definition with metadata, key, and JSON schema generation.

## Attributes

All configuration is passed via the `#[kapi(...)]` helper attribute:

| Attribute  | Required | Default        | Description                                  |
|------------|----------|----------------|----------------------------------------------|
| `group`    | Yes      | —              | API group (e.g. `"example.io"`)              |
| `version`  | Yes      | —              | API version (e.g. `"v1"`)                    |
| `kind`     | Yes      | —              | Resource kind (e.g. `"Widget"`)              |
| `scope`    | No       | `"Namespaced"` | `"Namespaced"` or `"Cluster"`                |
| `status`   | No       | —              | Status type name (e.g. `"WidgetStatus"`)     |

### Validation

- `scope` must be exactly `"Namespaced"` or `"Cluster"` — anything else is a compile error.

## Generated Code

Given a spec struct:

```rust
#[derive(KapiResource, Serialize, Deserialize, JsonSchema)]
#[kapi(group = "example.io", version = "v1", kind = "Widget")]
pub struct WidgetSpec {
    pub color: String,
    pub replicas: u32,
}
```

The macro generates:

### 1. Wrapper Struct

```rust
/// Generated wrapper struct for the resource.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Widget {
    pub metadata: kapi_core::ObjectMeta,
    pub spec: WidgetSpec,
}
```

The wrapper struct is named after the `kind` attribute (e.g., `Widget`). It always
contains a `metadata` field of type `kapi_core::ObjectMeta` and a `spec` field of
the user's spec type. If a `status` type is specified, an optional `status` field
is included:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub status: Option<WidgetStatus>,
```

### 2. `key()` Method

```rust
impl Widget {
    pub fn key() -> kapi_core::ResourceKey {
        kapi_core::ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        }
    }
}
```

Returns a `ResourceKey` identifying this resource type. Used by controllers and
the API client to reference the resource.

### 3. `schema_data()` Method

```rust
impl Widget {
    pub fn schema_data() -> serde_json::Value {
        let spec_schema = schemars::schema_for!(WidgetSpec);
        let mut schema = serde_json::json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "scope": "Namespaced",
            "specSchema": spec_schema,
        });
        schema
    }
}
```

Generates the full `SchemaData` JSON payload for registering this resource with the
kapi server. The spec struct (and status struct, if present) must derive
`schemars::JsonSchema`.

## Complete Examples

### Without Status

```rust
use kapi_derive::KapiResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, KapiResource, Serialize, Deserialize, JsonSchema)]
#[kapi(group = "example.io", version = "v1", kind = "Widget")]
pub struct WidgetSpec {
    pub color: String,
    pub replicas: u32,
}
```

### With Status

```rust
use kapi_derive::KapiResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, KapiResource, Serialize, Deserialize, JsonSchema)]
#[kapi(group = "example.io", version = "v1", kind = "Gadget", status = "GadgetStatus")]
pub struct GadgetSpec {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GadgetStatus {
    pub phase: String,
}
```

### Cluster-Scoped

```rust
use kapi_derive::KapiResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, KapiResource, Serialize, Deserialize, JsonSchema)]
#[kapi(group = "example.io", version = "v1", kind = "ClusterWidget", scope = "Cluster")]
pub struct ClusterWidgetSpec {
    pub data: String,
}
```

## How to Use Generated Types

### Registering with the Server

```rust
let schema_data = Widget::schema_data();
let schema_value: serde_json::Value = schema_data;
// POST /apis/kapi.io/v1/Schema with schema_value
```

### Creating an Object

```rust
let widget = Widget {
    metadata: ObjectMeta {
        name: "my-widget".to_string(),
        namespace: Some("default".to_string()),
        labels: HashMap::new(),
        annotations: HashMap::new(),
        finalizers: Vec::new(),
    },
    spec: WidgetSpec {
        color: "blue".to_string(),
        replicas: 3,
    },
};
let json = serde_json::to_value(&widget)?;
// POST /apis/example.io/v1/namespaces/default/Widget with json
```

### Using in a Controller

```rust
fn reconcile(obj: &StoredObject) -> Result<()> {
    // Deserialize the spec
    let spec: WidgetSpec = serde_json::from_value(obj.spec.clone())?;
    println!("Color: {}, Replicas: {}", spec.color, spec.replicas);
    Ok(())
}
```
