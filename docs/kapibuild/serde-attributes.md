# Serde Attributes and Schema Generation

## Overview

schemars respects most `#[serde(...)]` attributes when generating JSON Schema.
This means you control both serialization behavior and API validation from a
single set of annotations. This document covers the most commonly used serde
attributes and how they affect the generated schema.

## rename

Changes the field name in both serialization and the JSON Schema:

```rust
pub struct WidgetSpec {
    #[serde(rename = "colorCode")]
    pub color: String,

    #[serde(rename = "numReplicas")]
    pub replicas: u32,
}
```

Generated schema:

```json
{
    "colorCode": { "type": "string" },
    "numReplicas": { "type": "integer" }
}
```

### rename_all

Applied at the struct level to rename all fields:

```rust
#[serde(rename_all = "camelCase")]
pub struct WidgetSpec {
    pub display_name: String,   // → "displayName"
    pub created_at: String,     // → "createdAt"
}
```

Common `rename_all` values:

| Value           | Example           |
|-----------------|-------------------|
| `"camelCase"`   | `myField`         |
| `"snake_case"`  | `my_field`        |
| `"kebab-case"`  | `my-field`        |
| `"UPPERCASE"`   | `MY_FIELD`        |
| `"lowercase"`   | `my_field`        |
| `"PascalCase"`  | `MyField`         |
| `"SCREAMING_SNAKE_CASE"` | `MY_FIELD` |

## skip

Removes a field from both serialization and schema:

```rust
pub struct WidgetSpec {
    pub name: String,

    #[serde(skip)]
    pub internal_cache: HashMap<String, String>,
}
```

The `internal_cache` field will not be serialized, deserialized, or validated.
It does not appear in the generated schema.

## skip_serializing_if

Controls whether a field is omitted during serialization when a condition is met.
This does **not** affect the schema — the field is still present in validation:

```rust
pub struct WidgetSpec {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}
```

The generated schema always includes these fields (as optional), but the server
won't emit them in responses when they are empty/None.

## default

Provides a default value when the field is missing from incoming JSON.
Also makes the field optional in the JSON Schema:

```rust
pub struct WidgetSpec {
    pub name: String,

    #[serde(default = "default_color")]
    pub color: String,

    #[serde(default)]
    pub replicas: u32,
}

fn default_color() -> String {
    "blue".to_string()
}
```

Generated schema:

```json
{
    "properties": {
        "name": { "type": "string" },
        "color": { "type": "string" },
        "replicas": { "type": "integer" }
    },
    "required": ["name"]
}
```

Both `color` and `replicas` are optional in the schema because they have defaults.
`name` is required (no default, not wrapped in `Option`).

## flatten

Inlines fields from another struct into the current schema:

```rust
pub struct Metadata {
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
}

pub struct WidgetSpec {
    pub name: String,

    #[serde(flatten)]
    pub meta: Metadata,
}
```

Generated schema:

```json
{
    "properties": {
        "name": { "type": "string" },
        "labels": {
            "type": "object",
            "additionalProperties": { "type": "string" }
        },
        "annotations": {
            "type": "object",
            "additionalProperties": { "type": "string" }
        }
    },
    "required": ["name", "labels", "annotations"]
}
```

The `meta` field is flattened away — its fields appear directly at the top level.

## deny_unknown_fields

Rejects JSON with unknown fields. This is applied at the struct level:

```rust
#[serde(deny_unknown_fields)]
pub struct WidgetSpec {
    pub name: String,
    pub color: String,
}
```

With this attribute, the server will reject any request with fields not defined
in the struct. This is useful for strict API contracts.

## untagged and tag for Enums

### untagged

```rust
#[serde(untagged)]
pub enum Value {
    String(String),
    Number(f64),
}
```

### tag (internally tagged)

```rust
#[serde(tag = "type")]
pub enum Message {
    Text { content: String },
    Image { url: String },
}
```

Generated schema uses `oneOf` for untagged enums and `discriminator` for tagged enums.

## Practical Example

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedWidgetSpec {
    /// Name of the widget (required).
    pub name: String,

    /// Optional description (omitted from response if None).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Number of replicas (defaults to 1).
    #[serde(default = "one")]
    pub replicas: u32,

    /// Tags for filtering (omitted from response if empty).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Arbitrary config data.
    #[serde(default, flatten)]
    pub config: HashMap<String, String>,
}

fn one() -> u32 { 1 }
```
