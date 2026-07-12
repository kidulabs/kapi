//! Controller orchestrator — watch + reconcile loops.
//!
//! A [`Controller`] watches a resource kind via SSE, enqueues changed objects
//! into a [`WorkQueue`](crate::workqueue::WorkQueue), and runs a reconcile
//! loop that invokes the user-provided [`Reconciler`](crate::reconciler::Reconciler).

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use kapi_client::client::KapiClient;
use kapi_client::error::ClientError;
use kapi_core::{ListOptions, ResourceKey, WatchEvent, WatchEventType, WatchFilter};
use tokio::sync::broadcast;

use crate::reconciler::{ReconcileContext, ReconcileRequest, Reconciler};
use crate::workqueue::{QueueKey, WorkQueue};

// ---------------------------------------------------------------------------
// Controller
// ---------------------------------------------------------------------------

/// Orchestrates a watch-then-reconcile loop for a single resource kind.
///
/// ## Example
///
/// ```ignore
/// let controller = Controller::new(key, Arc::new(MyReconciler), client)
///     .namespace("default")
///     .shutdown_signal(rx);
///
/// controller.start().await;
/// ```
pub struct Controller {
    key: ResourceKey,
    namespace: Option<String>,
    watch_filter: WatchFilter,
    reconciler: Arc<dyn Reconciler>,
    client: KapiClient,
    work_queue: Arc<WorkQueue>,
    shutdown_rx: Option<broadcast::Receiver<()>>,
}

impl Controller {
    /// Creates a new controller for the given resource key.
    pub fn new(key: ResourceKey, reconciler: Arc<dyn Reconciler>, client: KapiClient) -> Self {
        Controller {
            key,
            namespace: None,
            watch_filter: WatchFilter::All,
            reconciler,
            client,
            work_queue: Arc::new(WorkQueue::new()),
            shutdown_rx: None,
        }
    }

    /// Restricts the controller to watch only objects in this namespace.
    ///
    /// When set, the watch URL uses the namespaced path
    /// (`/apis/{g}/{v}/namespaces/{ns}/{kind}`) and reconnects list the same
    /// way.
    pub fn namespace(mut self, ns: impl Into<String>) -> Self {
        self.namespace = Some(ns.into());
        self
    }

    /// Sets a watch filter (label selector, field selector, etc.).
    ///
    /// Combined with the namespace (if set), both are applied on reconnects.
    pub fn watch_filter(mut self, filter: WatchFilter) -> Self {
        self.watch_filter = filter;
        self
    }

    /// Provides an optional shutdown signal.
    ///
    /// When the sender broadcasts `()`, the watch task exits and the reconcile
    /// loop finishes its current item and then exits.
    pub fn shutdown_signal(mut self, rx: broadcast::Receiver<()>) -> Self {
        self.shutdown_rx = Some(rx);
        self
    }

    // ------------------------------------------------------------------
    // Start
    // ------------------------------------------------------------------

    /// Starts the controller.
    ///
    /// Spawns a background watch task and runs the reconcile loop on the
    /// current task.  Returns when the shutdown signal is received.
    pub async fn start(&self) {
        // Clone shared state for the background watch task.
        let watch_queue = self.work_queue.clone();
        let watch_client = self.client.clone();
        let watch_key = self.key.clone();
        let watch_ns = self.namespace.clone();
        let watch_filter = self.watch_filter.clone();
        let mut watch_shutdown = self.shutdown_rx.as_ref().map(|rx| rx.resubscribe());

        // Spawn watch task.
        tokio::spawn(async move {
            // Outer reconnect loop.
            loop {
                // Open the watch stream.
                let mut stream = match watch_client
                    .watch(&watch_key, watch_ns.as_deref(), &watch_filter)
                    .await
                {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("failed to open watch stream: {e}");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };

                // Read events from the stream until it ends or errors.
                'stream: loop {
                    let shutdown = shutdown_or_pending(&mut watch_shutdown);

                    tokio::select! {
                        event = stream.next() => {
                            match event {
                                Some(Ok(ev)) => {
                                    if !should_enqueue(&ev) {
                                        continue;
                                    }
                                    let qk = QueueKey::new(
                                        ev.object.key,
                                        ev.object.metadata.name,
                                        ev.object.metadata.namespace,
                                    );
                                    watch_queue.add(qk).await;
                                }
                                Some(Err(e)) => {
                                    tracing::warn!("watch stream error: {e}");
                                    break 'stream; // reconnect
                                }
                                None => {
                                    tracing::warn!("watch stream ended, reconnecting...");
                                    break 'stream; // reconnect
                                }
                            }
                        }
                        _ = shutdown => return, // shutdown received
                    }
                }

                // Reconnect: list all objects (within scope) and enqueue
                // every key so we don't miss changes.
                match watch_client
                    .list(
                        &watch_key,
                        watch_ns.as_deref(),
                        &watch_filter_to_list_options(&watch_filter),
                    )
                    .await
                {
                    Ok(response) => {
                        for obj in response.items {
                            let qk =
                                QueueKey::new(obj.key, obj.metadata.name, obj.metadata.namespace);
                            watch_queue.add(qk).await;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("list failed during reconnect: {e}");
                    }
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        // Reconcile loop (runs on the current task).
        let mut reconcile_shutdown = self.shutdown_rx.as_ref().map(|rx| rx.resubscribe());

        loop {
            let shutdown = shutdown_or_pending(&mut reconcile_shutdown);

            tokio::select! {
                key = self.work_queue.get() => {
                    self.reconcile_one(key).await;
                }
                _ = shutdown => break,
            }
        }
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Fetches the object identified by `key` and runs the reconciler.
    async fn reconcile_one(&self, item: QueueKey) {
        let result = self.client.get(&item.key, item.namespace.as_deref(), &item.name).await;

        match result {
            Ok(_) => {
                let ctx = ReconcileContext {
                    request: ReconcileRequest {
                        key: item.key.clone(),
                        name: item.name.clone(),
                        namespace: item.namespace.clone(),
                    },
                    client: self.client.clone(),
                };

                match self.reconciler.reconcile(ctx).await {
                    Ok(reconcile_result) => {
                        self.work_queue.done(item.clone(), true).await;

                        if let Some(duration) = reconcile_result.requeue_after {
                            self.work_queue.requeue_after(item, duration).await;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            kind = %item.key.kind,
                            name = %item.name,
                            error = %e,
                            "reconciliation failed",
                        );
                        self.work_queue.done(item, false).await;
                    }
                }
            }
            Err(e) => {
                // 404 = object was deleted before we could fetch it, skip.
                if matches!(&e, ClientError::ApiError { status: 404, .. }) {
                    tracing::warn!(
                        kind = %item.key.kind,
                        name = %item.name,
                        "object not found, skipping",
                    );
                    self.work_queue.done(item, true).await;
                } else {
                    tracing::warn!(
                        kind = %item.key.kind,
                        name = %item.name,
                        error = %e,
                        "failed to fetch object",
                    );
                    self.work_queue.done(item, false).await;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Converts a [`WatchFilter`] into [`ListOptions`] for use in the reconnect
/// list call.
///
/// * `WatchFilter::All` → default options (no filtering).
/// * `WatchFilter::FieldSelector(fs)` → sets `field_selector`.
/// * `WatchFilter::LabelSelector(ls)` → sets `label_selector`.
/// * `WatchFilter::Namespace(_)` → ignored (namespace is handled via the URL).
/// * `WatchFilter::And(a, b)` → merges the two sides (first wins for each field).
fn watch_filter_to_list_options(filter: &WatchFilter) -> ListOptions {
    match filter {
        WatchFilter::All => ListOptions::default(),
        WatchFilter::FieldSelector(fs) => {
            ListOptions { field_selector: Some(fs.clone()), ..Default::default() }
        }
        WatchFilter::LabelSelector(ls) => {
            ListOptions { label_selector: Some(ls.clone()), ..Default::default() }
        }
        WatchFilter::Namespace(_) => ListOptions::default(),
        WatchFilter::And(a, b) => {
            let opts_a = watch_filter_to_list_options(a);
            let opts_b = watch_filter_to_list_options(b);
            ListOptions {
                field_selector: opts_a.field_selector.or(opts_b.field_selector),
                label_selector: opts_a.label_selector.or(opts_b.label_selector),
                ..Default::default()
            }
        }
    }
}

/// Returns `true` when the event should be enqueued for reconciliation.
///
/// [`StatusModified`](WatchEventType::StatusModified) events are filtered out
/// because status-only changes typically don't need full reconciliation.
pub fn should_enqueue(event: &WatchEvent) -> bool {
    !matches!(event.event_type, WatchEventType::StatusModified)
}

/// Returns a future that resolves when the shutdown signal fires,
/// or a future that never resolves when no shutdown was configured.
async fn shutdown_or_pending(rx: &mut Option<broadcast::Receiver<()>>) {
    if let Some(rx) = rx {
        loop {
            match rx.recv().await {
                Ok(()) | Err(broadcast::error::RecvError::Closed) => return,
                // Lagged means we missed some messages — keep waiting for the
                // next one.
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    } else {
        std::future::pending::<()>().await
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconciler::ReconcileResult;
    use kapi_core::{ObjectMeta, SystemMetadata, WatchEventType};
    use serde_json::Value;

    // -- should_enqueue ---------------------------------------------------

    #[test]
    fn test_should_enqueue_added() {
        let event = make_event(WatchEventType::Added);
        assert!(should_enqueue(&event));
    }

    #[test]
    fn test_should_enqueue_modified() {
        let event = make_event(WatchEventType::Modified);
        assert!(should_enqueue(&event));
    }

    #[test]
    fn test_should_enqueue_deleted() {
        let event = make_event(WatchEventType::Deleted);
        assert!(should_enqueue(&event));
    }

    #[test]
    fn test_should_enqueue_status_modified_filtered() {
        let event = make_event(WatchEventType::StatusModified);
        assert!(!should_enqueue(&event));
    }

    // -- Controller builder methods ---------------------------------------

    #[test]
    fn test_controller_new_defaults() {
        let key = test_key();
        let client = KapiClient::new("http://localhost:8080").unwrap();
        let reconciler = Arc::new(NoopReconciler);
        let ctrl = Controller::new(key, reconciler, client);

        assert!(ctrl.namespace.is_none());
        assert!(matches!(ctrl.watch_filter, WatchFilter::All));
        assert!(ctrl.shutdown_rx.is_none());
    }

    #[test]
    fn test_controller_builder_namespace() {
        let key = test_key();
        let client = KapiClient::new("http://localhost:8080").unwrap();
        let ctrl =
            Controller::new(key.clone(), Arc::new(NoopReconciler), client).namespace("my-ns");

        assert_eq!(ctrl.namespace, Some("my-ns".into()));
    }

    #[test]
    fn test_controller_builder_watch_filter() {
        let key = test_key();
        let client = KapiClient::new("http://localhost:8080").unwrap();
        let filter =
            WatchFilter::FieldSelector(kapi_core::FieldSelector::NameEquals("target".into()));
        let ctrl =
            Controller::new(key.clone(), Arc::new(NoopReconciler), client).watch_filter(filter);

        // Can't compare WatchFilter directly (contains Box), so spot-check
        // via string representation.
        let debug = format!("{:?}", ctrl.watch_filter);
        assert!(debug.contains("FieldSelector"));
    }

    #[test]
    fn test_controller_builder_shutdown_signal() {
        let key = test_key();
        let client = KapiClient::new("http://localhost:8080").unwrap();
        let (tx, rx) = broadcast::channel::<()>(1);
        let ctrl =
            Controller::new(key.clone(), Arc::new(NoopReconciler), client).shutdown_signal(rx);

        assert!(ctrl.shutdown_rx.is_some());
        // Sender still alive — we can send a signal.
        let _ = tx;
    }

    // -- ReconcileRequest from StoredObject -------------------------------

    #[test]
    fn test_controller_reconcile_request_construction() {
        let obj = make_stored_object("test-obj", Some("default"));
        let request = ReconcileRequest {
            key: obj.key.clone(),
            name: obj.metadata.name.clone(),
            namespace: obj.metadata.namespace.clone(),
        };

        assert_eq!(request.key, obj.key);
        assert_eq!(request.name, "test-obj");
        assert_eq!(request.namespace, Some("default".into()));
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn test_key() -> ResourceKey {
        ResourceKey { group: "example.io".into(), version: "v1".into(), kind: "Widget".into() }
    }

    fn make_stored_object(name: &str, namespace: Option<&str>) -> kapi_core::StoredObject {
        kapi_core::StoredObject {
            key: test_key(),
            metadata: ObjectMeta {
                name: name.into(),
                namespace: namespace.map(String::from),
                labels: Default::default(),
                annotations: Default::default(),
                finalizers: Vec::new(),
            },
            system: SystemMetadata::initial(),
            spec: Value::Null,
            status: None,
        }
    }

    fn make_event(event_type: WatchEventType) -> WatchEvent {
        WatchEvent { event_type, object: make_stored_object("test", Some("default")) }
    }

    /// A reconciler that always succeeds (no requeue).
    struct NoopReconciler;

    #[async_trait::async_trait]
    impl Reconciler for NoopReconciler {
        async fn reconcile(
            &self,
            _ctx: ReconcileContext,
        ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>> {
            Ok(Default::default())
        }
    }
}
