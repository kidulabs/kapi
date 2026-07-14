# Validation Rules

## Overview

kapi uses [schemars](https://docs.rs/schemars/) to generate JSON Schema from Rust types
at compile time. The kapi server uses the generated schema to validate all incoming
API requests automatically. This document covers how Rust type annotations translate
to validation rules.

## How Validation Works

1. **At compile time**: schemars generates a JSON Schema from your Rust struct
2. **At registration time**: the schema is stored server-side when you create a Schema resource
3. **At request time**: the server validates every CREATE/UPDATE against the stored schema

```text
Rust struct ──schemars──→ JSON Schema ──register──→ kapi server ──validate──→ API requests
```

## String Validation

### Length Constraints

Use `#[schemars(length(min = ..., max = ...))]` on `String` fields:

```rust
pub struct PersonSpec {
    /// Name must be between 1 and 64 characters.
    #[schemars(length(min = 1, max = 64))]
    pub name: String,

    /// Optional description, max 256 characters.
    #[schemars(length(max = 256))]
    pub description: String,
}
```

Generated schema:

```json
{
    "name": {
        "type": "string",
        "minLength": 1,
        "maxLength": 64
    },
    "description": {
        "type": "string",
        "maxLength": 256
    }
}
```

### Pattern Constraints

Use `#[schemars(pattern = "...")]` to enforce regex patterns:

```rust
pub struct ConfigSpec {
    /// Must be a valid DNS-1123 label (lowercase alphanumeric + hyphens).
    #[schemars(pattern = "^[a-z0-9]([a-z0-9-]*[a-z0-9])?$")]
    pub name: String,

    /// Must be a valid email address.
    #[schemars(pattern = "^[^@]+@[^@]+\\.[^@]+$")]
    pub contact_email: String,
}
```

Generated schema:

```json
{
    "name": {
        "type": "string",
        "pattern": "^[a-z0-9]([a-z0-9-]*[a-z0-9])?$"
    },
    "contact_email": {
        "type": "string",
        "pattern": "^[^@]+@[^@]+\\.[^@]+$"
    }
}
```

## Numeric Validation

### Range Constraints

Use `#[schemars(range(min = ..., max = ..., exclusive_min = ..., exclusive_max = ...))]`
on numeric fields:

```rust
pub struct ResourceLimitsSpec {
    /// CPU cores, between 0.1 and 64 (inclusive).
    #[schemars(range(min = 0.1, max = 64.0))]
    pub cpu: f64,

    /// Memory in MB, between 1 and 65536 (inclusive).
    #[schemars(range(min = 1, max = 65536))]
    pub memory_mb: u32,

    /// Replicas, must be at least 1.
    #[schemars(range(min = 1))]
    pub replicas: u32,

    /// Port number, exclusive range 1024-65535.
    #[schemars(range(min = 1024, exclusive_max = 65536))]
    pub port: u16,
}
```

Generated schema:

```json
{
    "cpu": { "type": "number", "minimum": 0.1, "maximum": 64.0 },
    "memory_mb": { "type": "integer", "minimum": 1, "maximum": 65536 },
    "replicas": { "type": "integer", "minimum": 1 },
    "port": { "type": "integer", "minimum": 1024, "exclusiveMaximum": 65536 }
}
```

### Multiple Of

```rust
pub struct BatchSpec {
    /// Must be a multiple of 10.
    #[schemars(multiple_of = 10.0)]
    pub batch_size: u32,
}
```

## Required Fields

Fields without `Option<T>` or `#[serde(default)]` are automatically required:

```rust
pub struct RequiredFieldsSpec {
    pub required_field: String,       // Required — must be present
    pub optional_field: Option<String>, // Optional — can be null or missing
    #[serde(default)]
    pub defaulted_field: String,      // Optional — defaults to "" if missing
}
```

Generated schema:

```json
{
    "required_field": { "type": "string" },
    "optional_field": { "type": "string" },
    "defaulted_field": { "type": "string" }
}
```

The `required` array in the schema will include `"required_field"` but not
`"optional_field"` or `"defaulted_field"`.

## Enum Validation

Rust enums generate JSON Schema `enum` or `oneOf` depending on variant types:

```rust
pub enum Color {
    Red,
    Green,
    Blue,
}

pub enum Condition {
    Ready,
    Unknown,
}
```

Generated schema:

```json
{
    "color": { "type": "string", "enum": ["Red", "Green", "Blue"] },
    "condition": { "type": "string", "enum": ["Ready", "Unknown"] }
}
```

## Nested Objects

Nested structs generate nested schemas recursively:

```rust
pub struct Address {
    pub street: String,
    pub city: String,
}

pub struct PersonSpec {
    pub name: String,
    pub address: Address,
    pub tags: Vec<String>,
}
```

Generated schema includes `$defs` for nested types:

```json
{
    "type": "object",
    "properties": {
        "name": { "type": "string" },
        "address": { "$ref": "#/$defs/Address" },
        "tags": { "type": "array", "items": { "type": "string" } }
    },
    "$defs": {
        "Address": {
            "type": "object",
            "properties": {
                "street": { "type": "string" },
                "city": { "type": "string" }
            }
        }
    }
}
```

## Testing Validation

After creating your schema and generating the JSON schema file, you can inspect it:

```bash
cat schemas/example.io_Widget.json
```

The kapi server will enforce these rules automatically — no server-side validation
code needed.
