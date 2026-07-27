/// Uniquely identifies a resource kind within the API server.
///
/// `ResourceKey` combines the three components of a kapi resource identifier:
/// `group`, `version`, and `kind`. It is used as a lookup key for schema
/// validation, event bus routing, and store operations.
///
/// # Example
///
/// ```
/// use kapi_core::ResourceKey;
///
/// let key = ResourceKey {
///     group: "example.io".to_string(),
///     version: "v1".to_string(),
///     kind: "Widget".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResourceKey {
    /// The API group (e.g. `"example.io"`). Use empty string for core resources.
    pub group: String,
    /// The API version within the group (e.g. `"v1"`, `"v2beta1"`).
    pub version: String,
    /// The resource kind (e.g. `"Widget"`, `"Schema"`, `"Namespace"`).
    pub kind: String,
}
