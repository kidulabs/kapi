//! Finalizer state machine — pure decision logic for object deletion lifecycle.
//!
//! Separates decision from execution: these functions read state and return
//! decisions, while the service layer executes via store transactions.

use chrono::Utc;

use crate::object::types::{ObjectMeta, StoredObject};
use crate::store::TransactionOp;

/// Action to take after evaluating a delete request.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum DeleteAction {
    /// No finalizers — hard delete the object.
    HardDeleted,
    /// Finalizers present, no deletion_timestamp yet — mark for deletion.
    MarkedForDeletion,
    /// Already has deletion_timestamp — idempotent no-op.
    IdempotentNoOp,
}

/// Evaluates what action to take for a delete request.
///
/// State machine transitions:
/// - Empty finalizers → HardDeleted (remove immediately)
/// - Non-empty finalizers, deletion_timestamp already set → IdempotentNoOp
/// - Non-empty finalizers, no deletion_timestamp → MarkedForDeletion
pub fn evaluate_delete(existing: &StoredObject) -> DeleteAction {
    if existing.metadata.finalizers.is_empty() {
        DeleteAction::HardDeleted
    } else if existing.system.deletion_timestamp.is_some() {
        DeleteAction::IdempotentNoOp
    } else {
        DeleteAction::MarkedForDeletion
    }
}

/// Executes the delete action as a [`TransactionOp`].
pub fn execute_delete(action: DeleteAction, existing: &StoredObject) -> TransactionOp {
    match action {
        DeleteAction::HardDeleted => TransactionOp::Delete,
        DeleteAction::MarkedForDeletion => {
            let mut marked = existing.clone();
            marked.system.deletion_timestamp = Some(Utc::now());
            TransactionOp::Apply(marked)
        }
        DeleteAction::IdempotentNoOp => TransactionOp::Apply(existing.clone()),
    }
}

/// Result of evaluating an update on an object that may be under deletion.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FinalizerDecision {
    /// Update is allowed.
    Allow,
    /// Update rejected — non-finalizer fields changed during deletion.
    RejectBeingDeleted,
}

/// Evaluates whether an update is allowed when the object may be under deletion.
///
/// If `deletion_timestamp` is set:
/// - Only finalizer modifications are allowed (not spec, labels, annotations)
/// - No new finalizers can be added (only removal)
///
/// If `deletion_timestamp` is not set, the update is allowed unconditionally.
pub fn evaluate_update(existing: &StoredObject, incoming: &ObjectMeta) -> FinalizerDecision {
    if existing.system.deletion_timestamp.is_none() {
        return FinalizerDecision::Allow;
    }

    // Check that only finalizers changed (name, labels, annotations unchanged, finalizers differ)
    if existing.metadata.name != incoming.name
        || existing.metadata.labels != incoming.labels
        || existing.metadata.annotations != incoming.annotations
        || existing.metadata.finalizers == incoming.finalizers
    {
        return FinalizerDecision::RejectBeingDeleted;
    }

    // Check no new finalizers were added
    for f in &incoming.finalizers {
        if !existing.metadata.finalizers.contains(f) {
            return FinalizerDecision::RejectBeingDeleted;
        }
    }

    FinalizerDecision::Allow
}

/// Returns true if the update should trigger a hard delete
/// (object is being deleted and finalizers became empty).
pub fn should_hard_delete(existing: &StoredObject, incoming_finalizers: &[String]) -> bool {
    existing.system.deletion_timestamp.is_some() && incoming_finalizers.is_empty()
}
