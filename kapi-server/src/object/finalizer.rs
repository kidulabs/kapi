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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{DateTime, Utc};
    use serde_json::json;

    use crate::object::types::{ObjectMeta, StoredObject, SystemMetadata};
    use crate::store::{ResourceKey, TransactionOp};

    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_object(
        name: &str,
        finalizers: Vec<&str>,
        deletion_timestamp: Option<DateTime<Utc>>,
    ) -> StoredObject {
        StoredObject {
            key: ResourceKey {
                group: "test.io".into(),
                version: "v1".into(),
                kind: "Widget".into(),
            },
            metadata: ObjectMeta {
                name: name.into(),
                namespace: None,
                labels: HashMap::new(),
                annotations: HashMap::new(),
                finalizers: finalizers.into_iter().map(String::from).collect(),
            },
            system: SystemMetadata {
                resource_version: 1,
                generation: 1,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                deletion_timestamp,
            },
            spec: json!({}),
            status: None,
        }
    }

    fn make_meta(name: &str, finalizers: Vec<&str>) -> ObjectMeta {
        ObjectMeta {
            name: name.into(),
            namespace: None,
            labels: HashMap::new(),
            annotations: HashMap::new(),
            finalizers: finalizers.into_iter().map(String::from).collect(),
        }
    }

    // -----------------------------------------------------------------------
    // evaluate_delete
    // -----------------------------------------------------------------------

    #[test]
    fn evaluate_delete_empty_finalizers_returns_hard_deleted() {
        let obj = make_object("test", vec![], None);
        assert_eq!(evaluate_delete(&obj), DeleteAction::HardDeleted);
    }

    #[test]
    fn evaluate_delete_finalizers_with_deletion_timestamp_returns_idempotent() {
        let obj = make_object("test", vec!["f1"], Some(Utc::now()));
        assert_eq!(evaluate_delete(&obj), DeleteAction::IdempotentNoOp);
    }

    #[test]
    fn evaluate_delete_finalizers_without_deletion_timestamp_returns_marked() {
        let obj = make_object("test", vec!["f1"], None);
        assert_eq!(evaluate_delete(&obj), DeleteAction::MarkedForDeletion);
    }

    // -----------------------------------------------------------------------
    // execute_delete
    // -----------------------------------------------------------------------

    #[test]
    fn execute_delete_hard_deleted_returns_delete_op() {
        let obj = make_object("test", vec![], None);
        let op = execute_delete(DeleteAction::HardDeleted, &obj);
        assert!(matches!(op, TransactionOp::Delete));
    }

    #[test]
    fn execute_delete_marked_sets_deletion_timestamp() {
        let obj = make_object("test", vec!["f1"], None);
        let op = execute_delete(DeleteAction::MarkedForDeletion, &obj);
        match op {
            TransactionOp::Apply(marked) => {
                assert!(
                    marked.system.deletion_timestamp.is_some(),
                    "expected deletion_timestamp to be set"
                );
                // Remaining fields should be cloned
                assert_eq!(marked.key, obj.key);
                assert_eq!(marked.metadata.name, obj.metadata.name);
                assert_eq!(marked.metadata.finalizers, obj.metadata.finalizers);
                assert_eq!(marked.spec, obj.spec);
            }
            _ => panic!("expected TransactionOp::Apply"),
        }
    }

    #[test]
    fn execute_delete_idempotent_returns_cloned_object() {
        let obj = make_object("test", vec!["f1"], Some(Utc::now()));
        let op = execute_delete(DeleteAction::IdempotentNoOp, &obj);
        match op {
            TransactionOp::Apply(cloned) => {
                // Should be an identical clone (same deletion_timestamp)
                assert_eq!(cloned.system.deletion_timestamp, obj.system.deletion_timestamp);
                assert_eq!(cloned.metadata.finalizers, obj.metadata.finalizers);
            }
            _ => panic!("expected TransactionOp::Apply"),
        }
    }

    // -----------------------------------------------------------------------
    // evaluate_update
    // -----------------------------------------------------------------------

    #[test]
    fn evaluate_update_no_deletion_timestamp_allows() {
        let existing = make_object("test", vec!["f1"], None);
        let incoming = make_meta("changed-name", vec!["f1"]);
        assert_eq!(evaluate_update(&existing, &incoming), FinalizerDecision::Allow);
    }

    #[test]
    fn evaluate_update_deleting_object_name_change_rejected() {
        let existing = make_object("test", vec!["f1"], Some(Utc::now()));
        let incoming = make_meta("different-name", vec!["f1"]);
        assert_eq!(evaluate_update(&existing, &incoming), FinalizerDecision::RejectBeingDeleted);
    }

    #[test]
    fn evaluate_update_deleting_object_labels_change_rejected() {
        let existing = StoredObject {
            metadata: ObjectMeta {
                labels: {
                    let mut m = HashMap::new();
                    m.insert("env".into(), "prod".into());
                    m
                },
                ..make_object("test", vec!["f1"], Some(Utc::now())).metadata
            },
            ..make_object("test", vec!["f1"], Some(Utc::now()))
        };
        let mut incoming = make_meta("test", vec!["f1"]);
        incoming.labels.insert("env".into(), "dev".into());
        assert_eq!(evaluate_update(&existing, &incoming), FinalizerDecision::RejectBeingDeleted);
    }

    #[test]
    fn evaluate_update_deleting_object_annotations_change_rejected() {
        let existing = StoredObject {
            metadata: ObjectMeta {
                annotations: {
                    let mut m = HashMap::new();
                    m.insert("note".into(), "a".into());
                    m
                },
                ..make_object("test", vec!["f1"], Some(Utc::now())).metadata
            },
            ..make_object("test", vec!["f1"], Some(Utc::now()))
        };
        let mut incoming = make_meta("test", vec!["f1"]);
        incoming.annotations.insert("note".into(), "b".into());
        assert_eq!(evaluate_update(&existing, &incoming), FinalizerDecision::RejectBeingDeleted);
    }

    #[test]
    fn evaluate_update_deleting_object_same_finalizers_rejected() {
        // When finalizers haven't changed (but deletion_timestamp is set),
        // the update carries no meaningful change → rejected.
        let existing = make_object("test", vec!["f1", "f2"], Some(Utc::now()));
        let incoming = make_meta("test", vec!["f1", "f2"]);
        assert_eq!(evaluate_update(&existing, &incoming), FinalizerDecision::RejectBeingDeleted);
    }

    #[test]
    fn evaluate_update_deleting_object_new_finalizer_rejected() {
        let existing = make_object("test", vec!["f1"], Some(Utc::now()));
        let incoming = make_meta("test", vec!["f1", "f2"]);
        assert_eq!(evaluate_update(&existing, &incoming), FinalizerDecision::RejectBeingDeleted);
    }

    #[test]
    fn evaluate_update_deleting_object_removing_finalizers_allowed() {
        let existing = make_object("test", vec!["f1", "f2"], Some(Utc::now()));
        let incoming = make_meta("test", vec!["f1"]);
        assert_eq!(evaluate_update(&existing, &incoming), FinalizerDecision::Allow);
    }

    #[test]
    fn evaluate_update_deleting_object_removing_all_finalizers_allowed() {
        let existing = make_object("test", vec!["f1", "f2"], Some(Utc::now()));
        let incoming = make_meta("test", vec![]);
        assert_eq!(evaluate_update(&existing, &incoming), FinalizerDecision::Allow);
    }

    // -----------------------------------------------------------------------
    // should_hard_delete
    // -----------------------------------------------------------------------

    #[test]
    fn should_hard_delete_with_deletion_timestamp_and_empty_finalizers() {
        let obj = make_object("test", vec!["f1"], Some(Utc::now()));
        assert!(should_hard_delete(&obj, &[]));
    }

    #[test]
    fn should_hard_delete_with_deletion_timestamp_and_non_empty_finalizers() {
        let obj = make_object("test", vec!["f1"], Some(Utc::now()));
        assert!(!should_hard_delete(&obj, &["f1".to_string()]));
    }

    #[test]
    fn should_hard_delete_without_deletion_timestamp() {
        let obj = make_object("test", vec![], None);
        assert!(!should_hard_delete(&obj, &[]));
    }
}
