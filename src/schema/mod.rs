pub mod meta_schema;
pub mod registry;

pub use meta_schema::{JsonSchemaValidator, SchemaValidationError, SchemaValidator};
pub use registry::SchemaRegistry;

use crate::store::ResourceKey;

pub const SCHEMA_KIND: &str = "Schema";
pub const SCHEMA_GROUP: &str = "kapi.io";
pub const SCHEMA_VERSION: &str = "v1";

/// Generates the cache key for a schema validator.
///
/// The key format is `"{kind}.{group}.{version}"`, which is used as both the
/// [`Schema`](crate::object::types::SchemaData) object's `metadata.name` and the
/// [`SchemaRegistry`](registry::SchemaRegistry) cache key. Including the version ensures
/// that schemas for the same kind and group but different API versions occupy independent
/// cache entries.
///
/// # Examples
///
/// ```
/// # use kapi::schema::schema_cache_key;
/// assert_eq!(schema_cache_key("Widget", "example.io", "v1"), "Widget.example.io.v1");
/// assert_eq!(schema_cache_key("Widget", "example.io", "v2"), "Widget.example.io.v2");
/// ```
pub fn schema_cache_key(kind: &str, group: &str, version: &str) -> String {
    format!("{}.{}.{}", kind, group, version)
}

pub fn schema_key() -> ResourceKey {
    ResourceKey {
        group: SCHEMA_GROUP.to_string(),
        version: SCHEMA_VERSION.to_string(),
        kind: SCHEMA_KIND.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_cache_key_format() {
        assert_eq!(schema_cache_key("Widget", "example.io", "v1"), "Widget.example.io.v1");
        assert_eq!(schema_cache_key("Widget", "example.io", "v2"), "Widget.example.io.v2");
        assert_eq!(schema_cache_key("Deployment", "apps", "v1beta1"), "Deployment.apps.v1beta1");
    }

    #[test]
    fn schema_cache_key_distinguishes_versions() {
        let v1 = schema_cache_key("Widget", "example.io", "v1");
        let v2 = schema_cache_key("Widget", "example.io", "v2");
        assert_ne!(v1, v2, "different versions must yield different keys");
    }
}
