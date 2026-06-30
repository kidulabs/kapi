pub mod meta_schema;
pub mod registry;

pub use meta_schema::{JsonSchemaValidator, SchemaValidationError, SchemaValidator};
pub use registry::SchemaRegistry;

use crate::store::ResourceKey;

pub const SCHEMA_KIND: &str = "Schema";
pub const SCHEMA_GROUP: &str = "kapi.io";
pub const SCHEMA_VERSION: &str = "v1";

// Namespace resource constants.
// Namespace is a built-in core type registered at server startup.
// It is always cluster-scoped (uses group/version above) and has
// `kind: "Namespace"`. The `"default"` namespace is auto-created at
// startup and protected from deletion.
pub const NAMESPACE_KIND: &str = "Namespace";
pub const NAMESPACE_GROUP: &str = "kapi.io";
pub const NAMESPACE_VERSION: &str = "v1";
pub const DEFAULT_NAMESPACE: &str = "default";

/// Scope value for namespaced resources.
pub const SCOPE_NAMESPACED: &str = "Namespaced";

/// Scope value for cluster-scoped resources.
pub const SCOPE_CLUSTER: &str = "Cluster";

/// Returns the [`ResourceKey`] for the built-in Namespace resource.
///
/// Cluster-scoped — all Namespace operations pass `namespace: None`
/// to the store, regardless of any namespace in the URL path.
pub fn namespace_key() -> ResourceKey {
    ResourceKey {
        group: NAMESPACE_GROUP.to_string(),
        version: NAMESPACE_VERSION.to_string(),
        kind: NAMESPACE_KIND.to_string(),
    }
}

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
