//! Controller-runtime SDK for writing controllers that watch kapi resources
//! and reconcile desired state.
//!
//! Provides a [`Reconciler`] trait, [`Controller`] for orchestrating watch +
//! reconcile loops, a [`WorkQueue`] with deduplication and backoff, and
//! standalone finalizer helper functions.

pub mod controller;
pub mod finalizer;
pub mod manager;
pub mod reconciler;
pub mod workqueue;
