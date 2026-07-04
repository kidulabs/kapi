//! Schema resolution for the kapi CLI.
//!
//! [`SchemaResolver`] caches all registered schemas and resolves user-provided
//! kind strings (with optional `group/kind` syntax) to [`ResourceKey`] + scope.

use kapi_client::{ResourceKey, SchemaData, StoredObject};

use crate::error::CliError;

/// Resolves kind strings to resource keys and scopes using cached schemas.
pub struct SchemaResolver {
    parsed: Vec<(SchemaData, ResourceKey)>,
}

impl SchemaResolver {
    /// Fetches all schemas from the server and parses them.
    ///
    /// Makes a single HTTP request to list all schemas, then caches them
    /// in-memory for the lifetime of this resolver.
    pub async fn new(client: &kapi_client::client::KapiClient) -> Result<Self, CliError> {
        let schemas = client.list_schemas().await.map_err(CliError::ClientError)?;
        let parsed = Self::parse_schemas(&schemas)?;
        Ok(SchemaResolver { parsed })
    }

    /// Parses each schema's spec into a `SchemaData` struct.
    fn parse_schemas(schemas: &[StoredObject]) -> Result<Vec<(SchemaData, ResourceKey)>, CliError> {
        schemas
            .iter()
            .map(|obj| {
                let data: SchemaData = serde_json::from_value(obj.spec.clone())
                    .map_err(|e| CliError::FormatError(format!("invalid schema spec: {e}")))?;
                Ok((data, obj.key.clone()))
            })
            .collect()
    }

    /// Resolves a kind string to a `(ResourceKey, scope)` pair.
    ///
    /// Supports two formats:
    /// - `kind` (e.g. `"Widget"`): matches on `target_kind` only.
    /// - `group/kind` (e.g. `"example.io/Widget"`): matches on both `target_group`
    ///   and `target_kind`.
    ///
    /// Built-in kinds (`Schema`, `Namespace`) are resolved directly without
    /// consulting the schema registry.
    ///
    /// Returns an error when:
    /// - No matching schema is found (with a helpful hint message).
    /// - Multiple schemas match (ambiguity error listing all matches).
    pub fn resolve_kind(&self, kind_str: &str) -> Result<(ResourceKey, String), CliError> {
        let (group_filter, kind_filter) = if let Some((g, k)) = kind_str.split_once('/') {
            (Some(g), k)
        } else {
            (None, kind_str)
        };

        // Handle built-in kinds that are not in the schema registry.
        if group_filter.is_none() || group_filter.is_some_and(|g| g.eq_ignore_ascii_case("kapi.io"))
        {
            if kind_filter.eq_ignore_ascii_case("Schema") {
                return Ok((
                    ResourceKey {
                        group: "kapi.io".to_string(),
                        version: "v1".to_string(),
                        kind: "Schema".to_string(),
                    },
                    "Cluster".to_string(),
                ));
            }
            if kind_filter.eq_ignore_ascii_case("Namespace") {
                return Ok((
                    ResourceKey {
                        group: "kapi.io".to_string(),
                        version: "v1".to_string(),
                        kind: "Namespace".to_string(),
                    },
                    "Cluster".to_string(),
                ));
            }
        }

        let matched: Vec<&(SchemaData, ResourceKey)> = self
            .parsed
            .iter()
            .filter(|(data, _)| {
                data.target_kind.eq_ignore_ascii_case(kind_filter)
                    && group_filter.is_none_or(|g| data.target_group.eq_ignore_ascii_case(g))
            })
            .collect();

        if matched.is_empty() {
            return Err(CliError::SchemaNotFound { kind: kind_str.to_string() });
        }

        if matched.len() > 1 {
            let paths: Vec<String> = matched
                .iter()
                .map(|(data, _)| format!("{}/{}", data.target_group, data.target_kind))
                .collect();
            return Err(CliError::ResolutionError(format!(
                "ambiguous kind '{kind_str}': matches {} possibilities: {}",
                matched.len(),
                paths.join(", ")
            )));
        }

        let (data, _schema_key) = matched[0];
        let key = ResourceKey {
            group: data.target_group.clone(),
            version: data.target_version.clone(),
            kind: data.target_kind.clone(),
        };
        Ok((key, data.scope.clone()))
    }
}
