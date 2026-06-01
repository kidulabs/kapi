pub mod meta_schema;
pub mod registry;

pub use meta_schema::{JsonSchemaValidator, SchemaValidationError, SchemaValidator};
pub use registry::SchemaRegistry;

use crate::store::ResourceKey;

pub const SCHEMA_KIND: &str = "Schema";
pub const SCHEMA_GROUP: &str = "kapi.io";
pub const SCHEMA_VERSION: &str = "v1";

pub fn schema_key() -> ResourceKey {
    ResourceKey {
        group: SCHEMA_GROUP.to_string(),
        version: SCHEMA_VERSION.to_string(),
        kind: SCHEMA_KIND.to_string(),
    }
}
