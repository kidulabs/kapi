# Design: Fix kapibuild schema generation for crates.io installs

## Problem Summary

The temporary helper project created by `kapibuild api generate` depends on `kapi-core` via a path dependency resolved at compile time using `env!("CARGO_MANIFEST_DIR")`. When kapibuild is installed from crates.io, this path points to the cargo registry cache where `kapi-core` doesn't exist.

## Root Cause Analysis

Analysis of `generate.rs` revealed that the helper project's dependency on `kapi-core` is **unnecessary**:

```
Helper binary's runtime call graph:

fn main()
  └─► Widget::schema_data()
        ├─► schemars::schema_for!(WidgetSpec)     ← needs: schemars + WidgetSpec
        ├─► schemars::schema_for!(WidgetStatus)   ← needs: schemars + WidgetStatus  
        └─► builds serde_json::Map                 ← needs: serde_json

No ObjectMeta. No ResourceKey. No kapi_core at runtime.
```

The `kapi-core` dependency exists only to make the generated wrapper code compile — the `ObjectMeta` and `ResourceKey` types are referenced in the wrapper struct and `key()` method, but `key()` is never called and `ObjectMeta` is never constructed at runtime.

## Solution: Strip kapi-core from Helper

Remove the unnecessary `kapi-core` dependency from the temporary helper project entirely.

### What Changes

| File/Function | Change |
|---|---|
| `write_helper_cargo_toml()` | Remove `kapi-core = { path = "..." }` line |
| `write_helper_main_rs()` | Remove `use kapi_core::{ObjectMeta, ResourceKey}` |
| `generate_wrapper_code()` | Remove `metadata: ObjectMeta` field and `key()` method |
| `workspace_root()` | Delete entirely |
| `create_helper_project()` | Remove `workspace_root` parameter |

### What Doesn't Change

- `generate_type_file()` — generates `types/` files in the user's project, which legitimately use `kapi_core` types
- `prepare_resource_module()` — strips kapi_controller/kapi_derive imports from api files
- Schema output format — identical JSON structure
- CLI interface — same `kapibuild api generate` command

### Helper's Cargo.toml (After)

```toml
[package]
name = "kapi-schema-helper"
version = "0.1.0"
edition = "2024"

[dependencies]
schemars = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### Generated Wrapper Code (After)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Widget {
    pub spec: WidgetSpec,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<WidgetStatus>,
}

impl Widget {
    pub fn schema_data() -> serde_json::Value {
        let spec_schema = schemars::schema_for!(WidgetSpec);
        let mut map = serde_json::Map::new();
        map.insert("targetGroup".into(), ...);
        map.insert("targetVersion".into(), ...);
        map.insert("targetKind".into(), ...);
        map.insert("scope".into(), ...);
        map.insert("specSchema".into(), serde_json::to_value(spec_schema).unwrap());
        // status schema if present
        serde_json::Value::Object(map)
    }
}
```

## Alternatives Considered

| Option | Description | Why Rejected |
|---|---|---|
| `cargo metadata` runtime discovery | Run `cargo metadata` to find kapi-core path | Adds subprocess cost, still fragile |
| kapi-core as crates.io dep | Use `kapi-core = "0.2"` instead of path | Couples kapibuild release to kapi-core release |
| Binary in target project | Generate bin in user's project, run from there | Larger change, target project must compile |

## Constraints

- User-defined spec/status structs must not reference `kapi_core` types (already the implicit contract — `prepare_resource_module()` embeds api source directly)
- No changes to schema output format
- No changes to CLI interface
