//! Core reconciler types — the [`Reconciler`] trait and supporting types.
//!
//! Controller implementations provide a [`Reconciler`] that receives
//! [`ReconcileContext`] and returns [`ReconcileResult`] indicating whether
//! the object should be re-queued.

use std::time::Duration;

use kapi_client::client::KapiClient;
use kapi_core::ResourceKey;

/// Identifies a single object that needs reconciliation.
///
/// Carries enough information to fetch the object from the API server.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ReconcileRequest {
    pub key: ResourceKey,
    pub name: String,
    pub namespace: Option<String>,
}

/// The outcome of a single reconciliation attempt.
///
/// The presence of [`requeue_after`](Self::requeue_after) indicates whether
/// the object should be re-queued:
/// - `None` — don't requeue
/// - `Some(Duration::ZERO)` — requeue immediately
/// - `Some(duration)` — requeue after the specified delay
#[derive(Debug, Clone, Default)]
pub struct ReconcileResult {
    /// Optional delay before the object is re-queued.
    /// `None` means don't requeue. `Some(Duration::ZERO)` means requeue immediately.
    pub requeue_after: Option<Duration>,
}

/// Context provided to [`Reconciler::reconcile`], containing the request and
/// an API client for interacting with the server.
#[derive(Debug)]
pub struct ReconcileContext {
    pub request: ReconcileRequest,
    pub client: KapiClient,
}

/// Trait implemented by controller logic.
///
/// Every reconciler is invoked with a [`ReconcileContext`] containing the
/// object identity and an API client.  The reconciler returns
/// [`ReconcileResult`] indicating whether to re-queue and with what delay.
///
/// # Errors
///
/// Return an error to signal a transient failure — the controller will
/// automatically re-queue with exponential backoff.
#[async_trait::async_trait]
pub trait Reconciler: Send + Sync {
    async fn reconcile(
        &self,
        ctx: ReconcileContext,
    ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> ResourceKey {
        ResourceKey { group: "example.io".into(), version: "v1".into(), kind: "Widget".into() }
    }

    #[test]
    fn test_reconcile_request_construction() {
        let key = test_key();
        let req = ReconcileRequest {
            key: key.clone(),
            name: "my-widget".into(),
            namespace: Some("default".into()),
        };
        assert_eq!(req.key, key);
        assert_eq!(req.name, "my-widget");
        assert_eq!(req.namespace, Some("default".into()));
    }

    #[test]
    fn test_reconcile_request_cluster_scoped() {
        let req =
            ReconcileRequest { key: test_key(), name: "cluster-resource".into(), namespace: None };
        assert_eq!(req.name, "cluster-resource");
        assert!(req.namespace.is_none());
    }

    #[test]
    fn test_reconcile_result_default() {
        let result = ReconcileResult::default();
        assert!(result.requeue_after.is_none());
    }

    #[test]
    fn test_reconcile_context_construction() {
        let client = KapiClient::new("http://localhost:8080").unwrap();
        let request = ReconcileRequest { key: test_key(), name: "test".into(), namespace: None };
        let ctx = ReconcileContext { request, client: client.clone() };
        assert_eq!(ctx.request.name, "test");
    }
}
