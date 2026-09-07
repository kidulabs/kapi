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
use kapi_core::{ApiError, ListOptions, ResourceKey, WatchEvent, WatchEventType, WatchFilter};
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
    /// Maximum duration for a single reconciliation pass (fetch + reconcile).
    reconcile_timeout: Duration,
}

impl Controller {
    /// Default reconcile timeout: 15 minutes.
    pub const DEFAULT_RECONCILE_TIMEOUT: Duration = Duration::from_secs(900);

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
            reconcile_timeout: Self::DEFAULT_RECONCILE_TIMEOUT,
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

    /// Sets the maximum duration for a single reconciliation pass (fetch +
    /// reconciler run). Defaults to [`DEFAULT_RECONCILE_TIMEOUT`](Self::DEFAULT_RECONCILE_TIMEOUT).
    ///
    /// When the timeout elapses, the reconciliation is aborted and the item
    /// is marked as failed, so it is re-queued with exponential backoff.
    pub fn reconcile_timeout(mut self, timeout: Duration) -> Self {
        self.reconcile_timeout = timeout;
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
        let watch_handle = tokio::spawn(async move {
            // Initial sync: list all existing objects (within scope) and
            // enqueue every key so pre-existing resources are reconciled even
            // if they never change while the watch stream is open. This runs
            // once before the first watch attempt, and happens regardless of
            // whether the watch stream can be opened.
            Self::list_and_enqueue_all(
                &watch_client,
                &watch_key,
                watch_ns.as_deref(),
                &watch_filter,
                &watch_queue,
            )
            .await;

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
                Self::list_and_enqueue_all(
                    &watch_client,
                    &watch_key,
                    watch_ns.as_deref(),
                    &watch_filter,
                    &watch_queue,
                )
                .await;

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        // Monitor the watch task — log if it panics instead of silently swallowing.
        tokio::spawn(async move {
            if let Err(e) = watch_handle.await {
                tracing::error!(?e, "watch task panicked");
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

    /// Lists all objects matching the scope (kind, namespace, filter) and
    /// enqueues every returned key into `queue`.
    ///
    /// Used both for the **initial sync** (before the first watch stream is
    /// opened, so existing resources are reconciled on startup) and on
    /// **reconnect** (so changes missed while disconnected are not lost).
    ///
    /// Failures are logged (warn) and swallowed — the caller retries on the
    /// next watch/reconnect cycle.
    async fn list_and_enqueue_all(
        client: &KapiClient,
        key: &ResourceKey,
        namespace: Option<&str>,
        filter: &WatchFilter,
        queue: &WorkQueue,
    ) {
        match client.list(key, namespace, &watch_filter_to_list_options(filter)).await {
            Ok(response) => {
                for obj in response.items {
                    let qk = QueueKey::new(obj.key, obj.metadata.name, obj.metadata.namespace);
                    queue.add(qk).await;
                }
            }
            Err(e) => {
                tracing::warn!("list failed while syncing existing objects: {e}");
            }
        }
    }

    /// Fetches the object identified by `key` and runs the reconciler.
    ///
    /// The entire operation (fetch + reconciler run) is bounded by
    /// [`reconcile_timeout`](Self::reconcile_timeout). When the timeout
    /// elapses the item is marked as failed so it is retried with exponential
    /// backoff.
    async fn reconcile_one(&self, item: QueueKey) {
        // Clone so `item` stays available for the timeout branch below.
        let inner_item = item.clone();

        match tokio::time::timeout(self.reconcile_timeout, async {
            let result = self
                .client
                .get(&inner_item.key, inner_item.namespace.as_deref(), &inner_item.name)
                .await;

            match result {
                Ok(_) => {
                    let ctx = ReconcileContext {
                        request: ReconcileRequest {
                            key: inner_item.key.clone(),
                            name: inner_item.name.clone(),
                            namespace: inner_item.namespace.clone(),
                        },
                        client: self.client.clone(),
                    };

                    match self.reconciler.reconcile(ctx).await {
                        Ok(reconcile_result) => {
                            self.work_queue.done(inner_item.clone(), true).await;

                            if let Some(duration) = reconcile_result.requeue_after {
                                self.work_queue.requeue_after(inner_item, duration).await;
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                kind = %inner_item.key.kind,
                                name = %inner_item.name,
                                error = %e,
                                "reconciliation failed",
                            );
                            self.work_queue.done(inner_item, false).await;
                        }
                    }
                }
                Err(e) => {
                    // 404 = object was deleted before we could fetch it, skip.
                    if matches!(&e, ClientError::Api(ApiError::NotFound { .. })) {
                        tracing::warn!(
                            kind = %inner_item.key.kind,
                            name = %inner_item.name,
                            "object not found, skipping",
                        );
                        self.work_queue.done(inner_item, true).await;
                    } else {
                        tracing::warn!(
                            kind = %inner_item.key.kind,
                            name = %inner_item.name,
                            error = %e,
                            "failed to fetch object",
                        );
                        self.work_queue.done(inner_item, false).await;
                    }
                }
            }
        })
        .await
        {
            Ok(()) => {} // inner body handled done()
            Err(_elapsed) => {
                tracing::error!(
                    kind = %item.key.kind,
                    name = %item.name,
                    timeout_secs = self.reconcile_timeout.as_secs(),
                    "reconciliation timed out",
                );
                self.work_queue.done(item, false).await;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Converts a [`WatchFilter`] into [`ListOptions`] for use in the list call
/// made during initial sync and reconnects.
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
    use kapi_core::{ListResponse, ObjectMeta, SystemMetadata, WatchEventType};
    use serde_json::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

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

    // -- Initial list on startup ------------------------------------------

    /// A reconciler that counts how many times it was invoked.
    struct CountingReconciler {
        count: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl Reconciler for CountingReconciler {
        async fn reconcile(
            &self,
            _ctx: ReconcileContext,
        ) -> Result<ReconcileResult, Box<dyn std::error::Error + Send + Sync>> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(Default::default())
        }
    }

    /// Spawns a minimal HTTP server that serves:
    /// - `GET .../{kind}` (list) → JSON `ListResponse` containing `items`
    /// - `GET .../{kind}?watch=true` → SSE headers, then holds the connection
    ///   open without ever sending events (watch stream stays pending)
    /// - `GET .../{kind}/{name}` (get) → JSON `StoredObject`
    ///
    /// Returns the base URL of the server.
    async fn spawn_mock_server(items: Vec<kapi_core::StoredObject>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let items = Arc::new(items);
        tokio::spawn(async move {
            loop {
                let (socket, _) = match listener.accept().await {
                    Ok(x) => x,
                    Err(_) => return,
                };
                let items = items.clone();
                tokio::spawn(async move { handle_mock_request(socket, items).await });
            }
        });
        format!("http://{addr}")
    }

    async fn handle_mock_request(mut socket: TcpStream, items: Arc<Vec<kapi_core::StoredObject>>) {
        // Read the request head (small requests fit in a few reads).
        let mut buf = Vec::with_capacity(4096);
        let mut chunk = [0u8; 1024];
        loop {
            let n = match socket.read(&mut chunk).await {
                Ok(0) | Err(_) => return,
                Ok(n) => n,
            };
            buf.extend_from_slice(&chunk[..n]);
            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let head = String::from_utf8_lossy(&buf);
        let path = head.split_whitespace().nth(1).unwrap_or_default();

        if path.contains("watch=true") {
            // Respond with SSE headers and hold the connection open — the
            // controller's watch stream stays pending with no events.
            let resp = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n";
            if socket.write_all(resp.as_bytes()).await.is_err() {
                return;
            }
            let _ = socket.flush().await;
            // Park forever, keeping the socket open.
            std::future::pending::<()>().await;
        }

        if path.ends_with(&format!("/{}", "Widget")) {
            // List request.
            let body = serde_json::to_string(&ListResponse {
                items: (*items).clone(),
                continue_token: None,
            })
            .unwrap();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(resp.as_bytes()).await;
            let _ = socket.flush().await;
            return;
        }

        // Get request: `.../Widget/{name}`.
        let name = path.rsplit('/').next().unwrap_or_default();
        let obj = items.iter().find(|o| o.metadata.name == name);
        let resp = match obj {
            Some(obj) => {
                let body = serde_json::to_string(obj).unwrap();
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
            }
            None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_string(),
        };
        let _ = socket.write_all(resp.as_bytes()).await;
        let _ = socket.flush().await;
    }

    /// Verifies that existing resources are listed and enqueued when the
    /// controller starts — i.e. before/during the watch stream opening —
    /// so pre-existing objects are reconciled even if they never change
    /// while the watch is open.
    #[tokio::test]
    async fn test_initial_list_on_startup() {
        let key = test_key();
        let items = vec![
            make_stored_object("obj-a", Some("default")),
            make_stored_object("obj-b", Some("default")),
        ];
        let base_url = spawn_mock_server(items).await;
        let client = KapiClient::new(&base_url).unwrap();

        let count = Arc::new(AtomicUsize::new(0));
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

        let controller =
            Controller::new(key, Arc::new(CountingReconciler { count: count.clone() }), client)
                .shutdown_signal(shutdown_rx);

        let handle = tokio::spawn(async move { controller.start().await });

        // The two pre-existing objects must be reconciled purely from the
        // initial list (the mock watch stream never emits any events).
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while count.load(Ordering::SeqCst) < 2 {
            assert!(
                tokio::time::Instant::now() < deadline,
                "controller did not reconcile pre-existing objects on startup"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        shutdown_tx.send(()).unwrap();
        handle.await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    /// Directly verifies the helper used for initial sync and reconnect:
    /// all listed objects are enqueued into the work queue.
    #[tokio::test]
    async fn test_list_and_enqueue_helper_populates_queue() {
        let key = test_key();
        let items = vec![
            make_stored_object("obj-a", Some("default")),
            make_stored_object("obj-b", Some("default")),
        ];
        let base_url = spawn_mock_server(items).await;
        let client = KapiClient::new(&base_url).unwrap();

        let queue = WorkQueue::new();
        assert_eq!(queue.len().await, 0);

        Controller::list_and_enqueue_all(&client, &key, None, &WatchFilter::All, &queue).await;

        assert_eq!(queue.len().await, 2);
        let first = queue.get().await;
        assert_eq!(first.name, "obj-a");
        let second = queue.get().await;
        assert_eq!(second.name, "obj-b");
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
