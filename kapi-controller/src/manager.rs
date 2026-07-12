//! Manager — orchestrates multiple controllers in one process.
//!
//! A [`Manager`] owns a shared [`KapiClient`] and shutdown signal, and
//! coordinates the lifecycle of multiple [`Controller`] instances.

use std::sync::Arc;

use kapi_client::client::KapiClient;
use kapi_core::ResourceKey;
use tokio::sync::broadcast;
use tokio::task::JoinSet;

use crate::controller::Controller;
use crate::reconciler::Reconciler;

// ---------------------------------------------------------------------------
// Manager
// ---------------------------------------------------------------------------

/// Orchestrates multiple controllers in a single process.
///
/// The [`Manager`] owns a shared [`KapiClient`] and a shutdown broadcast
/// channel.  Controllers are registered via [`controller_for`](Self::controller_for),
/// which returns a [`ControllerBuilder`] for configuration.
///
/// When [`start`](Self::start) is called, all registered controllers are
/// started and the manager waits for a shutdown signal.
pub struct Manager {
    client: KapiClient,
    controllers: Vec<ControllerHandle>,
    shutdown_tx: broadcast::Sender<()>,
}

impl Manager {
    /// Creates a new manager with the given shared API client.
    pub fn new(client: KapiClient) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Manager { client, controllers: Vec::new(), shutdown_tx }
    }

    /// Returns a builder for configuring a controller for the given resource
    /// key.
    pub fn controller_for(&mut self, key: ResourceKey) -> ControllerBuilder<'_> {
        ControllerBuilder { manager: self, key, reconciler: None, namespace: None }
    }

    /// Returns a clone of the shutdown sender, useful for programmatic
    /// shutdown in tests or custom signal handling.
    pub fn shutdown_sender(&self) -> broadcast::Sender<()> {
        self.shutdown_tx.clone()
    }

    /// Starts all registered controllers and waits for shutdown.
    ///
    /// Spawns a signal handler (SIGTERM, SIGINT), then starts each controller
    /// as a separate tokio task.  The manager runs until a shutdown signal is
    /// received, then waits up to 30 seconds for all controllers to exit
    /// gracefully.  If the grace period expires, the process exits with code 1.
    pub async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let count = self.controllers.len();
        tracing::info!("starting manager with {} controllers", count);

        let shutdown_tx = self.shutdown_tx.clone();

        // Spawn signal handler for graceful shutdown.
        let sig_tx = self.shutdown_tx.clone();
        tokio::spawn(async move {
            signal_handler(sig_tx).await;
        });

        // Start all controllers as tasks in a JoinSet.
        let mut tasks: JoinSet<String> = JoinSet::new();
        for handle in self.controllers {
            let kind = handle.key.kind.clone();
            let mut controller =
                Controller::new(handle.key, handle.reconciler, self.client.clone());
            if let Some(ns) = handle.namespace {
                controller = controller.namespace(ns);
            }
            controller = controller.shutdown_signal(shutdown_tx.subscribe());

            tasks.spawn(async move {
                controller.start().await;
                kind
            });
        }

        // Wait for either:
        //   1. All controllers to exit on their own (e.g. fatal error), or
        //   2. A shutdown signal (SIGTERM/SIGINT) to arrive.
        let mut shutdown_rx = shutdown_tx.subscribe();
        let all_exited = async {
            while let Some(result) = tasks.join_next().await {
                match result {
                    Ok(kind) => tracing::info!("controller for {kind} exited"),
                    Err(e) => tracing::warn!("controller task panicked: {e}"),
                }
            }
        };

        tokio::select! {
            _ = all_exited => {
                tracing::info!("all controllers exited");
                return Ok(());
            }
            _ = Self::recv_shutdown(&mut shutdown_rx) => {
                tracing::info!("shutdown signal received, starting 30s grace period");
            }
        }

        // Grace period: wait up to 30 seconds for remaining controllers to finish.
        let grace = tokio::time::Duration::from_secs(30);
        let remaining = async {
            while let Some(result) = tasks.join_next().await {
                match result {
                    Ok(kind) => tracing::info!("controller for {kind} exited"),
                    Err(e) => tracing::warn!("controller task panicked: {e}"),
                }
            }
        };

        match tokio::time::timeout(grace, remaining).await {
            Ok(()) => Ok(()),
            Err(_) => {
                tracing::warn!("shutdown grace period exceeded, force-exiting");
                std::process::exit(1);
            }
        }
    }

    /// Awaits the shutdown broadcast, treating both a clean receive and a
    /// closed channel as "shutdown requested".  Lagged receives are skipped.
    async fn recv_shutdown(rx: &mut broadcast::Receiver<()>) {
        loop {
            match rx.recv().await {
                Ok(()) | Err(broadcast::error::RecvError::Closed) => return,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ControllerHandle (internal)
// ---------------------------------------------------------------------------

struct ControllerHandle {
    key: ResourceKey,
    reconciler: Arc<dyn Reconciler>,
    namespace: Option<String>,
}

// ---------------------------------------------------------------------------
// ControllerBuilder
// ---------------------------------------------------------------------------

/// Builder for configuring a controller before registering it with the
/// [`Manager`].
pub struct ControllerBuilder<'a> {
    manager: &'a mut Manager,
    key: ResourceKey,
    reconciler: Option<Arc<dyn Reconciler>>,
    namespace: Option<String>,
}

impl<'a> ControllerBuilder<'a> {
    /// Sets the reconciler for this controller.
    pub fn reconcile_with(mut self, reconciler: impl Reconciler + 'static) -> Self {
        self.reconciler = Some(Arc::new(reconciler));
        self
    }

    /// Restricts this controller to watch only objects in the given namespace.
    pub fn namespace(mut self, ns: impl Into<String>) -> Self {
        self.namespace = Some(ns.into());
        self
    }

    /// Registers the controller with the manager.
    ///
    /// # Panics
    ///
    /// Panics if [`reconcile_with`](Self::reconcile_with) was not called before
    /// calling this method.
    pub fn register(self) {
        let reconciler = self.reconciler.expect(
            "ControllerBuilder::register called without setting a reconciler via reconcile_with",
        );
        self.manager.controllers.push(ControllerHandle {
            key: self.key,
            reconciler,
            namespace: self.namespace,
        });
    }
}

// ---------------------------------------------------------------------------
// Signal handling
// ---------------------------------------------------------------------------

/// Listens for SIGTERM and SIGINT and sends the shutdown signal.
#[cfg(unix)]
async fn signal_handler(shutdown_tx: broadcast::Sender<()>) {
    use tokio::signal::unix::{SignalKind, signal};

    let mut sigterm =
        signal(SignalKind::terminate()).expect("failed to create SIGTERM signal handler");
    let mut sigint =
        signal(SignalKind::interrupt()).expect("failed to create SIGINT signal handler");

    tokio::select! {
        _ = sigterm.recv() => {}
        _ = sigint.recv() => {}
    }

    tracing::info!("shutdown signal received, stopping controllers");
    let _ = shutdown_tx.send(());
}

/// Non-unix signal handler — just stays pending (shutdown must be triggered
/// programmatically via [`Manager::shutdown_sender`]).
#[cfg(not(unix))]
async fn signal_handler(_shutdown_tx: broadcast::Sender<()>) {
    // On non-unix platforms, signal handling is unavailable.
    // Shutdown can be triggered via Manager::shutdown_sender().
    std::future::pending::<()>().await
}
