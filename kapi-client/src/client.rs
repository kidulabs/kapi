//! Core HTTP client for the kapi API server.
//!
//! [`KapiClient`] provides type-safe methods for all standard API operations:
//! list, get, create, update, delete, status sub-resources, and watch (SSE).

use std::pin::Pin;

use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use serde_json::Value;

use kapi_core::{
    FieldSelector, LabelRequirement, LabelSelector, ListOptions, ListResponse, ObjectMeta,
    ResourceKey, StoredObject, WatchEvent, WatchFilter,
};

use crate::error::ClientError;

/// A client for interacting with a kapi API server.
///
/// All HTTP methods construct URLs from the provided `base_url`, resource key,
/// optional namespace, and path suffix.  The client is agnostic to scoping rules —
/// it builds whatever URL you ask for.
///
/// # Errors
///
/// Every fallible method returns [`ClientError`]:
/// - [`ClientError::HttpError`] for transport-level failures.
/// - [`ClientError::ApiError`] when the server responds with a non-2xx status
///   and a structured JSON error body.
/// - [`ClientError::SerializationError`] for JSON parse/serialize failures.
/// - [`ClientError::StreamError`] for SSE stream parse errors.
#[derive(Debug, Clone)]
pub struct KapiClient {
    client: reqwest::Client,
    base_url: String,
}

impl KapiClient {
    /// Creates a new client targeting the given server base URL.
    ///
    /// The `base_url` should be the scheme + host + optional port, e.g.
    /// `"http://localhost:8080"`.  A trailing slash is stripped if present.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::HttpError`] if the underlying HTTP client cannot
    /// be initialised (e.g. TLS backend failure).
    pub fn new(base_url: &str) -> Result<Self, ClientError> {
        let client = reqwest::Client::builder().build()?;
        Ok(KapiClient { client, base_url: base_url.trim_end_matches('/').to_string() })
    }

    // ------------------------------------------------------------------
    // URL construction
    // ------------------------------------------------------------------

    /// Builds the full request URL for a given resource.
    ///
    /// * **Cluster-scoped** (namespace is `None`):
    ///   `{base_url}/apis/{group}/{version}/{kind}{path_suffix}`
    /// * **Namespace-scoped** (namespace is `Some(ns)`):
    ///   `{base_url}/apis/{group}/{version}/namespaces/{ns}/{kind}{path_suffix}`
    ///
    /// `path_suffix` may be empty or start with `"/name"` or `"/name/status"`.
    pub fn build_url(
        &self,
        key: &ResourceKey,
        namespace: Option<&str>,
        path_suffix: &str,
    ) -> String {
        let prefix = format!("{}/apis/{}/{}", self.base_url, key.group, key.version);
        match namespace {
            Some(ns) => format!("{}/namespaces/{}/{}{}", prefix, ns, key.kind, path_suffix),
            None => format!("{}/{}{}", prefix, key.kind, path_suffix),
        }
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Checks the response status and either returns the response for further
    /// processing or deserialises the error body into [`ClientError::ApiError`].
    async fn check_response(
        &self,
        response: reqwest::Response,
    ) -> Result<reqwest::Response, ClientError> {
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status().as_u16();
        let body: Value = response.json().await.map_err(|e| {
            // If we can't even parse the error body, wrap it as a serialization error.
            ClientError::StreamError(format!(
                "failed to parse error response (status {status}): {e}"
            ))
        })?;
        let code = body.get("code").and_then(Value::as_str).unwrap_or("Unknown").to_string();
        let message =
            body.get("error").and_then(Value::as_str).unwrap_or("Unknown error").to_string();
        Err(ClientError::ApiError { status, code, message })
    }

    /// Deserialises a successful response into the target type `T`.
    async fn parse_response<T: DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, ClientError> {
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    // ------------------------------------------------------------------
    // CRUD methods
    // ------------------------------------------------------------------

    /// Lists objects of a given kind.
    ///
    /// **HTTP:** `GET /apis/{group}/{version}/{kind}` (cluster) or
    /// `GET /apis/{group}/{version}/namespaces/{ns}/{kind}` (namespaced)
    /// with optional query parameters `limit`, `continue`, `fieldSelector`,
    /// `labelSelector`.
    pub async fn list(
        &self,
        key: &ResourceKey,
        namespace: Option<&str>,
        opts: &ListOptions,
    ) -> Result<ListResponse, ClientError> {
        let url = self.build_url(key, namespace, "");
        let mut req = self.client.get(&url);

        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(limit) = opts.limit {
            params.push(("limit", limit.to_string()));
        }
        if let Some(ref ct) = opts.continue_token {
            params.push(("continue", ct.0.clone()));
        }
        if let Some(ref fs) = opts.field_selector {
            params.push(("fieldSelector", field_selector_to_string(fs)));
        }
        if let Some(ref ls) = opts.label_selector {
            params.push(("labelSelector", label_selector_to_string(ls)));
        }
        if !params.is_empty() {
            req = req.query(&params);
        }

        let response = req.send().await?;
        self.parse_response(response).await
    }

    /// Retrieves a single object by name.
    ///
    /// **HTTP:** `GET /apis/{group}/{version}/{kind}/{name}` (cluster) or
    /// `GET /apis/{group}/{version}/namespaces/{ns}/{kind}/{name}` (namespaced).
    pub async fn get(
        &self,
        key: &ResourceKey,
        namespace: Option<&str>,
        name: &str,
    ) -> Result<StoredObject, ClientError> {
        let url = self.build_url(key, namespace, &format!("/{name}"));
        let response = self.client.get(&url).send().await?;
        self.parse_response(response).await
    }

    /// Creates a new object.
    ///
    /// The request body is `{ "metadata": meta, "spec": spec }` — NOT a full
    /// `StoredObject`.  The server assigns system metadata.
    ///
    /// **HTTP:** `POST /apis/{group}/{version}/{kind}` (cluster) or
    /// `POST /apis/{group}/{version}/namespaces/{ns}/{kind}` (namespaced).
    pub async fn create(
        &self,
        key: &ResourceKey,
        namespace: Option<&str>,
        meta: &ObjectMeta,
        spec: &Value,
    ) -> Result<StoredObject, ClientError> {
        let url = self.build_url(key, namespace, "");
        let body = serde_json::json!({ "metadata": meta, "spec": spec });
        let response = self.client.post(&url).json(&body).send().await?;
        self.parse_response(response).await
    }

    /// Creates a new Schema object.
    ///
    /// Schema registration expects the schema fields (`targetKind`, `targetGroup`,
    /// `targetVersion`, etc.) at the top level of the body alongside `metadata`.
    /// This differs from regular object creation which wraps data in a `spec` field.
    ///
    /// **HTTP:** `POST /apis/kapi.io/v1/Schema`
    pub async fn create_schema(
        &self,
        meta: &ObjectMeta,
        schema_data: &Value,
    ) -> Result<StoredObject, ClientError> {
        let key = ResourceKey {
            group: "kapi.io".to_string(),
            version: "v1".to_string(),
            kind: "Schema".to_string(),
        };
        let url = self.build_url(&key, None, "");
        // Merge metadata and schema fields at top level.
        let mut body = schema_data.as_object().cloned().unwrap_or_default();
        body.insert("metadata".to_string(), serde_json::to_value(meta)?);
        let response = self.client.post(&url).json(&Value::Object(body)).send().await?;
        self.parse_response(response).await
    }

    /// Replaces an existing object (full update).
    ///
    /// Sends the full `StoredObject` as JSON.  The URL is derived from
    /// `obj.key` and `obj.metadata.name`.
    ///
    /// **HTTP:** `PUT /apis/{group}/{version}/{kind}/{name}` (cluster) or
    /// `PUT /apis/{group}/{version}/namespaces/{ns}/{kind}/{name}` (namespaced).
    pub async fn update(
        &self,
        namespace: Option<&str>,
        obj: &StoredObject,
    ) -> Result<StoredObject, ClientError> {
        let url = self.build_url(&obj.key, namespace, &format!("/{}", obj.metadata.name));
        let response = self.client.put(&url).json(obj).send().await?;
        self.parse_response(response).await
    }

    /// Deletes an object by name and returns the deleted object.
    ///
    /// **HTTP:** `DELETE /apis/{group}/{version}/{kind}/{name}` (cluster) or
    /// `DELETE /apis/{group}/{version}/namespaces/{ns}/{kind}/{name}` (namespaced).
    pub async fn delete(
        &self,
        key: &ResourceKey,
        namespace: Option<&str>,
        name: &str,
    ) -> Result<StoredObject, ClientError> {
        let url = self.build_url(key, namespace, &format!("/{name}"));
        let response = self.client.delete(&url).send().await?;
        self.parse_response(response).await
    }

    // ------------------------------------------------------------------
    // Status sub-resource methods
    // ------------------------------------------------------------------

    /// Retrieves the status of an object.
    ///
    /// Returns `None` when the object has no status set (server returns `null`).
    ///
    /// **HTTP:** `GET /apis/{group}/{version}/{kind}/{name}/status` (cluster) or
    /// `GET /apis/{group}/{version}/namespaces/{ns}/{kind}/{name}/status` (namespaced).
    pub async fn get_status(
        &self,
        key: &ResourceKey,
        namespace: Option<&str>,
        name: &str,
    ) -> Result<Option<Value>, ClientError> {
        let url = self.build_url(key, namespace, &format!("/{name}/status"));
        let response = self.client.get(&url).send().await?;
        let body: Value = self.parse_response(response).await?;
        // Server returns the status value directly (not wrapped in {"status": ...}).
        // A JSON null means no status is set.
        if body.is_null() { Ok(None) } else { Ok(Some(body)) }
    }

    /// Updates the status sub-resource of an object.
    ///
    /// The request body is `{ "status": status_value }`.  Returns the full
    /// updated object.
    ///
    /// **HTTP:** `PUT /apis/{group}/{version}/{kind}/{name}/status` (cluster) or
    /// `PUT /apis/{group}/{version}/namespaces/{ns}/{kind}/{name}/status` (namespaced).
    pub async fn update_status(
        &self,
        key: &ResourceKey,
        namespace: Option<&str>,
        name: &str,
        status: &Value,
    ) -> Result<StoredObject, ClientError> {
        let url = self.build_url(key, namespace, &format!("/{name}/status"));
        let body = serde_json::json!({ "status": status });
        let response = self.client.put(&url).json(&body).send().await?;
        self.parse_response(response).await
    }

    // ------------------------------------------------------------------
    // Watch (SSE stream)
    // ------------------------------------------------------------------

    /// Opens a long-lived watch connection and returns a stream of watch events.
    ///
    /// The returned stream yields [`WatchEvent`] items as they arrive over the
    /// SSE connection.  When the server closes the connection the stream
    /// terminates with `None`.
    ///
    /// The `filter` parameter is serialised into query parameters:
    /// - [`WatchFilter::LabelSelector`] → `labelSelector=...`
    /// - [`WatchFilter::FieldSelector`] → `fieldSelector=...`
    /// - [`WatchFilter::Namespace`] is **not** added to query params (use the
    ///   `namespace` function parameter instead).
    /// - [`WatchFilter::And`] combines the parameters of both children.
    ///
    /// **HTTP:** `GET /apis/{group}/{version}/{kind}?watch=true` (cluster) or
    /// `GET /apis/{group}/{version}/namespaces/{ns}/{kind}?watch=true` (namespaced).
    pub async fn watch(
        &self,
        key: &ResourceKey,
        namespace: Option<&str>,
        filter: &WatchFilter,
    ) -> Result<
        Pin<Box<dyn futures_util::Stream<Item = Result<WatchEvent, ClientError>> + Send>>,
        ClientError,
    > {
        let url = self.build_url(key, namespace, "");
        let mut req = self.client.get(&url).query(&[("watch", "true")]);

        let pairs = watch_filter_to_query_pairs(filter);
        if !pairs.is_empty() {
            req = req.query(&pairs);
        }

        let response = req.send().await?;

        // Check for HTTP error before consuming the body stream.
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body: Value = response.json().await.map_err(|e| {
                ClientError::StreamError(format!(
                    "failed to parse error response (status {status}): {e}"
                ))
            })?;
            let code = body.get("code").and_then(Value::as_str).unwrap_or("Unknown").to_string();
            let message =
                body.get("error").and_then(Value::as_str).unwrap_or("Unknown error").to_string();
            return Err(ClientError::ApiError { status, code, message });
        }

        let event_stream = response.bytes_stream().eventsource();

        let stream = event_stream.filter_map(|event_result| {
            async move {
                match event_result {
                    Ok(event) if event.event == "message" => {
                        match serde_json::from_str::<WatchEvent>(&event.data) {
                            Ok(ev) => Some(Ok(ev)),
                            Err(e) => Some(Err(ClientError::SerializationError(e))),
                        }
                    }
                    Ok(_) => {
                        // Skip non-"message" events (e.g. keep-alive comments).
                        None
                    }
                    Err(e) => Some(Err(ClientError::StreamError(e.to_string()))),
                }
            }
        });

        Ok(Box::pin(stream))
    }

    // ------------------------------------------------------------------
    // Schema helpers
    // ------------------------------------------------------------------

    /// Lists all registered schemas.
    ///
    /// Convenience wrapper around `list()` for the built-in `Schema` kind
    /// (group `kapi.io`, version `v1`, cluster-scoped).
    ///
    /// **HTTP:** `GET /apis/kapi.io/v1/Schema`
    pub async fn list_schemas(&self) -> Result<Vec<StoredObject>, ClientError> {
        let key = ResourceKey {
            group: "kapi.io".to_string(),
            version: "v1".to_string(),
            kind: "Schema".to_string(),
        };
        let opts = ListOptions::default();
        let response = self.list(&key, None, &opts).await?;
        Ok(response.items)
    }
}

// ------------------------------------------------------------------
// Serialisation helpers for selectors
// ------------------------------------------------------------------

/// Converts a [`FieldSelector`] into its query-string representation.
///
/// * [`FieldSelector::NameEquals(n)`] → `"metadata.name={n}"`
pub fn field_selector_to_string(fs: &FieldSelector) -> String {
    match fs {
        FieldSelector::NameEquals(name) => format!("metadata.name={name}"),
    }
}

/// Converts a [`LabelSelector`] into its query-string representation.
///
/// Requirements are comma-joined with AND semantics:
/// - `Equals` → `key=value`
/// - `NotEquals` → `key!=value`
/// - `Exists` → `key`
/// - `NotExists` → `!key`
pub fn label_selector_to_string(ls: &LabelSelector) -> String {
    ls.requirements
        .iter()
        .map(|req| match req {
            LabelRequirement::Equals { key, value } => format!("{key}={value}"),
            LabelRequirement::NotEquals { key, value } => format!("{key}!={value}"),
            LabelRequirement::Exists { key } => key.clone(),
            LabelRequirement::NotExists { key } => format!("!{key}"),
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Extracts query-string parameter pairs from a [`WatchFilter`].
///
/// Only [`WatchFilter::LabelSelector`] and [`WatchFilter::FieldSelector`]
/// produce parameters.  [`WatchFilter::All`] and [`WatchFilter::Namespace`]
/// produce nothing — namespace is already encoded in the URL path.
pub fn watch_filter_to_query_pairs(filter: &WatchFilter) -> Vec<(&'static str, String)> {
    match filter {
        WatchFilter::All | WatchFilter::Namespace(_) => vec![],
        WatchFilter::FieldSelector(fs) => {
            vec![("fieldSelector", field_selector_to_string(fs))]
        }
        WatchFilter::LabelSelector(ls) => {
            vec![("labelSelector", label_selector_to_string(ls))]
        }
        WatchFilter::And(a, b) => {
            let mut pairs = watch_filter_to_query_pairs(a);
            pairs.extend(watch_filter_to_query_pairs(b));
            pairs
        }
    }
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use kapi_core::LabelSelector;
    use serde_json::json;

    // --- URL construction ---

    #[test]
    fn build_url_cluster_scoped() {
        let client = KapiClient::new("http://localhost:8080").unwrap();
        let key =
            ResourceKey { group: "example.io".into(), version: "v1".into(), kind: "Widget".into() };
        let url = client.build_url(&key, None, "");
        assert_eq!(url, "http://localhost:8080/apis/example.io/v1/Widget");
    }

    #[test]
    fn build_url_namespace_scoped() {
        let client = KapiClient::new("http://localhost:8080").unwrap();
        let key =
            ResourceKey { group: "example.io".into(), version: "v1".into(), kind: "Widget".into() };
        let url = client.build_url(&key, Some("default"), "");
        assert_eq!(url, "http://localhost:8080/apis/example.io/v1/namespaces/default/Widget");
    }

    #[test]
    fn build_url_with_name() {
        let client = KapiClient::new("http://localhost:8080").unwrap();
        let key =
            ResourceKey { group: "example.io".into(), version: "v1".into(), kind: "Widget".into() };
        let url = client.build_url(&key, Some("prod"), "/my-widget");
        assert_eq!(
            url,
            "http://localhost:8080/apis/example.io/v1/namespaces/prod/Widget/my-widget"
        );
    }

    #[test]
    fn build_url_strips_trailing_slash_from_base() {
        let client = KapiClient::new("http://localhost:8080/").unwrap();
        let key =
            ResourceKey { group: "example.io".into(), version: "v1".into(), kind: "Widget".into() };
        let url = client.build_url(&key, None, "");
        assert_eq!(url, "http://localhost:8080/apis/example.io/v1/Widget");
    }

    #[test]
    fn build_url_with_status_suffix() {
        let client = KapiClient::new("http://localhost:8080").unwrap();
        let key =
            ResourceKey { group: "example.io".into(), version: "v1".into(), kind: "Widget".into() };
        let url = client.build_url(&key, None, "/my-widget/status");
        assert_eq!(url, "http://localhost:8080/apis/example.io/v1/Widget/my-widget/status");
    }

    // --- Field selector serialisation ---

    #[test]
    fn field_selector_name_equals() {
        let fs = FieldSelector::NameEquals("my-resource".into());
        assert_eq!(field_selector_to_string(&fs), "metadata.name=my-resource");
    }

    // --- Label selector serialisation ---

    #[test]
    fn label_selector_equals() {
        let ls = LabelSelector {
            requirements: vec![LabelRequirement::Equals {
                key: "app".into(),
                value: "nginx".into(),
            }],
        };
        assert_eq!(label_selector_to_string(&ls), "app=nginx");
    }

    #[test]
    fn label_selector_not_equals() {
        let ls = LabelSelector {
            requirements: vec![LabelRequirement::NotEquals {
                key: "env".into(),
                value: "prod".into(),
            }],
        };
        assert_eq!(label_selector_to_string(&ls), "env!=prod");
    }

    #[test]
    fn label_selector_exists() {
        let ls =
            LabelSelector { requirements: vec![LabelRequirement::Exists { key: "gpu".into() }] };
        assert_eq!(label_selector_to_string(&ls), "gpu");
    }

    #[test]
    fn label_selector_not_exists() {
        let ls = LabelSelector {
            requirements: vec![LabelRequirement::NotExists { key: "experimental".into() }],
        };
        assert_eq!(label_selector_to_string(&ls), "!experimental");
    }

    #[test]
    fn label_selector_multiple_and() {
        let ls = LabelSelector {
            requirements: vec![
                LabelRequirement::Equals { key: "app".into(), value: "nginx".into() },
                LabelRequirement::NotEquals { key: "env".into(), value: "prod".into() },
                LabelRequirement::Exists { key: "gpu".into() },
                LabelRequirement::NotExists { key: "legacy".into() },
            ],
        };
        assert_eq!(label_selector_to_string(&ls), "app=nginx,env!=prod,gpu,!legacy");
    }

    // --- Watch filter to query pairs ---

    #[test]
    fn watch_filter_all_yields_no_params() {
        assert!(watch_filter_to_query_pairs(&WatchFilter::All).is_empty());
    }

    #[test]
    fn watch_filter_namespace_yields_no_params() {
        let pairs = watch_filter_to_query_pairs(&WatchFilter::Namespace("default".into()));
        assert!(pairs.is_empty());
    }

    #[test]
    fn watch_filter_label_selector_yields_label_selector_param() {
        let ls = LabelSelector {
            requirements: vec![LabelRequirement::Equals {
                key: "app".into(),
                value: "nginx".into(),
            }],
        };
        let pairs = watch_filter_to_query_pairs(&WatchFilter::LabelSelector(ls));
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "labelSelector");
        assert_eq!(pairs[0].1, "app=nginx");
    }

    #[test]
    fn watch_filter_field_selector_yields_field_selector_param() {
        let fs = FieldSelector::NameEquals("target".into());
        let pairs = watch_filter_to_query_pairs(&WatchFilter::FieldSelector(fs));
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "fieldSelector");
        assert_eq!(pairs[0].1, "metadata.name=target");
    }

    #[test]
    fn watch_filter_and_combines_params() {
        let fs = WatchFilter::FieldSelector(FieldSelector::NameEquals("target".into()));
        let ls = LabelSelector {
            requirements: vec![LabelRequirement::Equals {
                key: "app".into(),
                value: "nginx".into(),
            }],
        };
        let combined = WatchFilter::And(Box::new(fs), Box::new(WatchFilter::LabelSelector(ls)));
        let pairs = watch_filter_to_query_pairs(&combined);
        assert_eq!(pairs.len(), 2);
        // Order depends on And(a, b) traversal: a first, then b.
        assert_eq!(pairs[0].0, "fieldSelector");
        assert_eq!(pairs[1].0, "labelSelector");
    }

    // --- Error response parsing (unit, no HTTP) ---

    #[test]
    fn api_error_from_json_body() {
        let body = json!({
            "error": "something went wrong",
            "code": "Conflict",
            "details": {}
        });
        let status: u16 = 409;
        let code = body["code"].as_str().unwrap().to_string();
        let message = body["error"].as_str().unwrap().to_string();
        let err = ClientError::ApiError { status, code, message };
        let msg = format!("{err}");
        assert!(msg.contains("409"));
        assert!(msg.contains("Conflict"));
        assert!(msg.contains("something went wrong"));
    }

    #[test]
    fn api_error_from_minimal_json_body() {
        let status: u16 = 500;
        let err = ClientError::ApiError {
            status,
            code: "Unknown".to_string(),
            message: "internal error".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("500"));
        assert!(msg.contains("internal error"));
    }
}
