pub use crate::store::ResourceKey;

use std::collections::HashMap;

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContinueToken(pub String);

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ListOptions {
    pub limit: Option<usize>,
    pub continue_token: Option<ContinueToken>,
    pub field_selector: Option<FieldSelector>,
    pub label_selector: Option<LabelSelector>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ListResponse {
    pub items: Vec<StoredObject>,
    pub continue_token: Option<ContinueToken>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

// FieldSelector implements field-based filtering
// Currently supports only metadata.name (fieldSelector=metadata.name=<value>)
// Extensible: NameNotEquals, NameIn variants can be added later
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FieldSelector {
    NameEquals(String),
}

// LabelRequirement represents a single label matching condition
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LabelRequirement {
    Equals { key: String, value: String },
    NotEquals { key: String, value: String },
    Exists { key: String },
    NotExists { key: String },
}

impl LabelRequirement {
    /// Returns true if the requirement matches the given labels.
    pub fn matches(&self, labels: &HashMap<String, String>) -> bool {
        match self {
            LabelRequirement::Equals { key, value } => labels.get(key).is_some_and(|v| v == value),
            LabelRequirement::NotEquals { key, value } => labels.get(key) != Some(value),
            LabelRequirement::Exists { key } => labels.contains_key(key),
            LabelRequirement::NotExists { key } => !labels.contains_key(key),
        }
    }
}

// LabelSelector contains a set of label requirements that must all match (AND semantics)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LabelSelector {
    pub requirements: Vec<LabelRequirement>,
}

impl LabelSelector {
    /// Returns true if all requirements match the given labels.
    /// An empty selector matches all objects.
    pub fn matches(&self, labels: &HashMap<String, String>) -> bool {
        self.requirements.iter().all(|req| req.matches(labels))
    }
}

// WatchFilter determines which events a watcher receives
// Used by EventBus for predicate routing: publish() only delivers to matching watchers
#[derive(Debug, Clone)]
pub enum WatchFilter {
    All,
    FieldSelector(FieldSelector),
    LabelSelector(LabelSelector),
    And(Box<WatchFilter>, Box<WatchFilter>),
}

impl WatchFilter {
    // Returns true if the event should be delivered to a watcher with this filter
    // All matches everything; FieldSelector delegates to field-level comparison;
    // LabelSelector delegates to label-level comparison;
    // And requires both sub-filters to match (short-circuit)
    pub fn matches(&self, event: &WatchEvent) -> bool {
        match self {
            WatchFilter::All => true,
            WatchFilter::FieldSelector(fs) => match fs {
                FieldSelector::NameEquals(name) => event.object.metadata.name == *name,
            },
            WatchFilter::LabelSelector(ls) => ls.matches(&event.object.metadata.labels),
            WatchFilter::And(a, b) => a.matches(event) && b.matches(event),
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum WatchEventType {
    Added,
    Modified,
    Deleted,
    StatusModified,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WatchEvent {
    pub event_type: WatchEventType,
    pub object: StoredObject,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpecData {
    pub value: serde_json::Value,
}

// Schema data struct for type-safe access to Schema registration payloads
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaData {
    pub target_group: String,
    pub target_version: String,
    pub target_kind: String,
    pub spec_schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectMeta {
    pub name: String,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMetadata {
    pub resource_version: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredObject {
    pub key: ResourceKey,
    pub metadata: ObjectMeta,
    pub system: SystemMetadata,
    pub spec: SpecData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<SpecData>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_event(name: &str) -> WatchEvent {
        WatchEvent {
            event_type: WatchEventType::Added,
            object: StoredObject {
                key: ResourceKey {
                    group: "test.io".into(),
                    version: "v1".into(),
                    kind: "Test".into(),
                },
                metadata: ObjectMeta {
                    name: name.into(),
                    labels: HashMap::new(),
                },
                system: SystemMetadata {
                    resource_version: 1,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                },
                spec: SpecData {
                    value: serde_json::json!({}),
                },
                status: None,
            },
        }
    }

    #[test]
    fn watch_filter_all_matches_all_events() {
        assert!(WatchFilter::All.matches(&make_event("foo")));
        assert!(WatchFilter::All.matches(&make_event("bar")));
    }

    #[test]
    fn watch_filter_field_selector_name_equals_matches_correct_name() {
        let filter = WatchFilter::FieldSelector(FieldSelector::NameEquals("target".into()));
        assert!(filter.matches(&make_event("target")));
    }

    #[test]
    fn watch_filter_field_selector_name_equals_rejects_wrong_name() {
        let filter = WatchFilter::FieldSelector(FieldSelector::NameEquals("target".into()));
        assert!(!filter.matches(&make_event("other")));
    }

    #[test]
    fn field_selector_name_equals_equality() {
        let fs = FieldSelector::NameEquals("foo".into());
        // Match same name
        let event = make_event("foo");
        assert!(WatchFilter::FieldSelector(fs).matches(&event));
    }

    // LabelRequirement::matches() tests

    #[test]
    fn label_requirement_equals_matches() {
        let req = LabelRequirement::Equals {
            key: "app".into(),
            value: "nginx".into(),
        };
        let mut labels = HashMap::new();
        labels.insert("app".into(), "nginx".into());
        labels.insert("env".into(), "prod".into());
        assert!(req.matches(&labels));
    }

    #[test]
    fn label_requirement_equals_different_value() {
        let req = LabelRequirement::Equals {
            key: "app".into(),
            value: "nginx".into(),
        };
        let mut labels = HashMap::new();
        labels.insert("app".into(), "apache".into());
        assert!(!req.matches(&labels));
    }

    #[test]
    fn label_requirement_equals_key_missing() {
        let req = LabelRequirement::Equals {
            key: "app".into(),
            value: "nginx".into(),
        };
        let mut labels = HashMap::new();
        labels.insert("env".into(), "prod".into());
        assert!(!req.matches(&labels));
    }

    #[test]
    fn label_requirement_not_equals_different_value() {
        let req = LabelRequirement::NotEquals {
            key: "env".into(),
            value: "prod".into(),
        };
        let mut labels = HashMap::new();
        labels.insert("env".into(), "staging".into());
        assert!(req.matches(&labels));
    }

    #[test]
    fn label_requirement_not_equals_same_value() {
        let req = LabelRequirement::NotEquals {
            key: "env".into(),
            value: "prod".into(),
        };
        let mut labels = HashMap::new();
        labels.insert("env".into(), "prod".into());
        assert!(!req.matches(&labels));
    }

    #[test]
    fn label_requirement_not_equals_key_missing() {
        let req = LabelRequirement::NotEquals {
            key: "env".into(),
            value: "prod".into(),
        };
        let mut labels = HashMap::new();
        labels.insert("app".into(), "nginx".into());
        assert!(req.matches(&labels));
    }

    #[test]
    fn label_requirement_exists_key_present() {
        let req = LabelRequirement::Exists { key: "gpu".into() };
        let mut labels = HashMap::new();
        labels.insert("gpu".into(), "true".into());
        assert!(req.matches(&labels));
    }

    #[test]
    fn label_requirement_exists_key_present_empty_value() {
        let req = LabelRequirement::Exists { key: "gpu".into() };
        let mut labels = HashMap::new();
        labels.insert("gpu".into(), "".into());
        assert!(req.matches(&labels));
    }

    #[test]
    fn label_requirement_exists_key_missing() {
        let req = LabelRequirement::Exists { key: "gpu".into() };
        let mut labels = HashMap::new();
        labels.insert("app".into(), "nginx".into());
        assert!(!req.matches(&labels));
    }

    #[test]
    fn label_requirement_not_exists_key_missing() {
        let req = LabelRequirement::NotExists {
            key: "experimental".into(),
        };
        let mut labels = HashMap::new();
        labels.insert("app".into(), "nginx".into());
        assert!(req.matches(&labels));
    }

    #[test]
    fn label_requirement_not_exists_key_present() {
        let req = LabelRequirement::NotExists {
            key: "experimental".into(),
        };
        let mut labels = HashMap::new();
        labels.insert("experimental".into(), "true".into());
        assert!(!req.matches(&labels));
    }

    // LabelSelector::matches() tests

    #[test]
    fn label_selector_empty_matches_all() {
        let selector = LabelSelector {
            requirements: vec![],
        };
        let labels = HashMap::new();
        assert!(selector.matches(&labels));

        let mut labels2 = HashMap::new();
        labels2.insert("app".into(), "nginx".into());
        assert!(selector.matches(&labels2));
    }

    #[test]
    fn label_selector_single_requirement() {
        let selector = LabelSelector {
            requirements: vec![LabelRequirement::Equals {
                key: "app".into(),
                value: "nginx".into(),
            }],
        };
        let mut labels = HashMap::new();
        labels.insert("app".into(), "nginx".into());
        assert!(selector.matches(&labels));

        let mut labels2 = HashMap::new();
        labels2.insert("app".into(), "apache".into());
        assert!(!selector.matches(&labels2));
    }

    #[test]
    fn label_selector_multiple_requirements_and_semantics() {
        let selector = LabelSelector {
            requirements: vec![
                LabelRequirement::Equals {
                    key: "app".into(),
                    value: "nginx".into(),
                },
                LabelRequirement::Exists { key: "env".into() },
            ],
        };
        let mut labels = HashMap::new();
        labels.insert("app".into(), "nginx".into());
        labels.insert("env".into(), "prod".into());
        assert!(selector.matches(&labels));

        let mut labels2 = HashMap::new();
        labels2.insert("app".into(), "nginx".into());
        assert!(!selector.matches(&labels2));
    }

    // WatchFilter::LabelSelector tests

    #[test]
    fn watch_filter_label_selector_matches_event_labels() {
        let selector = LabelSelector {
            requirements: vec![LabelRequirement::Equals {
                key: "app".into(),
                value: "nginx".into(),
            }],
        };
        let filter = WatchFilter::LabelSelector(selector);
        let mut labels = HashMap::new();
        labels.insert("app".into(), "nginx".into());
        let event = WatchEvent {
            event_type: WatchEventType::Added,
            object: StoredObject {
                key: ResourceKey {
                    group: "test.io".into(),
                    version: "v1".into(),
                    kind: "Test".into(),
                },
                metadata: ObjectMeta {
                    name: "test".into(),
                    labels,
                },
                system: SystemMetadata {
                    resource_version: 1,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                },
                spec: SpecData {
                    value: serde_json::json!({}),
                },
                status: None,
            },
        };
        assert!(filter.matches(&event));
    }

    #[test]
    fn watch_filter_label_selector_does_not_match_event_labels() {
        let selector = LabelSelector {
            requirements: vec![LabelRequirement::Equals {
                key: "app".into(),
                value: "nginx".into(),
            }],
        };
        let filter = WatchFilter::LabelSelector(selector);
        let mut labels = HashMap::new();
        labels.insert("app".into(), "apache".into());
        let event = WatchEvent {
            event_type: WatchEventType::Added,
            object: StoredObject {
                key: ResourceKey {
                    group: "test.io".into(),
                    version: "v1".into(),
                    kind: "Test".into(),
                },
                metadata: ObjectMeta {
                    name: "test".into(),
                    labels,
                },
                system: SystemMetadata {
                    resource_version: 1,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                },
                spec: SpecData {
                    value: serde_json::json!({}),
                },
                status: None,
            },
        };
        assert!(!filter.matches(&event));
    }

    // WatchFilter::And combinator tests

    #[test]
    fn watch_filter_and_both_match() {
        let field = WatchFilter::FieldSelector(FieldSelector::NameEquals("target".into()));
        let mut labels = HashMap::new();
        labels.insert("app".into(), "nginx".into());
        let label = WatchFilter::LabelSelector(LabelSelector {
            requirements: vec![LabelRequirement::Equals {
                key: "app".into(),
                value: "nginx".into(),
            }],
        });
        let combined = WatchFilter::And(Box::new(field), Box::new(label));
        let event = WatchEvent {
            event_type: WatchEventType::Added,
            object: StoredObject {
                key: ResourceKey {
                    group: "test.io".into(),
                    version: "v1".into(),
                    kind: "Test".into(),
                },
                metadata: ObjectMeta {
                    name: "target".into(),
                    labels,
                },
                system: SystemMetadata {
                    resource_version: 1,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                },
                spec: SpecData {
                    value: serde_json::json!({}),
                },
                status: None,
            },
        };
        assert!(combined.matches(&event));
    }

    #[test]
    fn watch_filter_and_first_fails() {
        let field = WatchFilter::FieldSelector(FieldSelector::NameEquals("target".into()));
        let label = WatchFilter::LabelSelector(LabelSelector {
            requirements: vec![LabelRequirement::Equals {
                key: "app".into(),
                value: "nginx".into(),
            }],
        });
        let combined = WatchFilter::And(Box::new(field), Box::new(label));
        let event = make_event("other"); // name doesn't match field selector
        assert!(!combined.matches(&event));
    }

    #[test]
    fn watch_filter_and_second_fails() {
        let field = WatchFilter::FieldSelector(FieldSelector::NameEquals("target".into()));
        let mut labels = HashMap::new();
        labels.insert("app".into(), "apache".into()); // doesn't match label selector
        let label = WatchFilter::LabelSelector(LabelSelector {
            requirements: vec![LabelRequirement::Equals {
                key: "app".into(),
                value: "nginx".into(),
            }],
        });
        let combined = WatchFilter::And(Box::new(field), Box::new(label));
        let event = WatchEvent {
            event_type: WatchEventType::Added,
            object: StoredObject {
                key: ResourceKey {
                    group: "test.io".into(),
                    version: "v1".into(),
                    kind: "Test".into(),
                },
                metadata: ObjectMeta {
                    name: "target".into(),
                    labels,
                },
                system: SystemMetadata {
                    resource_version: 1,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                },
                spec: SpecData {
                    value: serde_json::json!({}),
                },
                status: None,
            },
        };
        assert!(!combined.matches(&event));
    }

    #[test]
    fn watch_filter_and_nested() {
        // And(And(a, b), c) should work
        let a = WatchFilter::FieldSelector(FieldSelector::NameEquals("target".into()));
        let mut labels = HashMap::new();
        labels.insert("app".into(), "nginx".into());
        labels.insert("env".into(), "prod".into());
        let b = WatchFilter::LabelSelector(LabelSelector {
            requirements: vec![LabelRequirement::Equals {
                key: "app".into(),
                value: "nginx".into(),
            }],
        });
        let c = WatchFilter::LabelSelector(LabelSelector {
            requirements: vec![LabelRequirement::Equals {
                key: "env".into(),
                value: "prod".into(),
            }],
        });
        let inner = WatchFilter::And(Box::new(a), Box::new(b));
        let combined = WatchFilter::And(Box::new(inner), Box::new(c));
        let event = WatchEvent {
            event_type: WatchEventType::Added,
            object: StoredObject {
                key: ResourceKey {
                    group: "test.io".into(),
                    version: "v1".into(),
                    kind: "Test".into(),
                },
                metadata: ObjectMeta {
                    name: "target".into(),
                    labels,
                },
                system: SystemMetadata {
                    resource_version: 1,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                },
                spec: SpecData {
                    value: serde_json::json!({}),
                },
                status: None,
            },
        };
        assert!(combined.matches(&event));
    }

    // --- Status subresource tests ---

    #[test]
    fn stored_object_serializes_with_status() {
        let obj = StoredObject {
            key: ResourceKey {
                group: "example.io".to_string(),
                version: "v1".to_string(),
                kind: "Widget".to_string(),
            },
            metadata: ObjectMeta {
                name: "test".to_string(),
                labels: HashMap::new(),
            },
            system: SystemMetadata {
                resource_version: 1,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            spec: SpecData {
                value: json!({"color": "blue"}),
            },
            status: Some(SpecData {
                value: json!({"phase": "Running"}),
            }),
        };
        let serialized = serde_json::to_string(&obj).unwrap();
        assert!(serialized.contains("\"status\""));
        assert!(serialized.contains("\"phase\""));
        assert!(serialized.contains("\"Running\""));
    }

    #[test]
    fn stored_object_serializes_without_status() {
        let obj = StoredObject {
            key: ResourceKey {
                group: "example.io".to_string(),
                version: "v1".to_string(),
                kind: "Widget".to_string(),
            },
            metadata: ObjectMeta {
                name: "test".to_string(),
                labels: HashMap::new(),
            },
            system: SystemMetadata {
                resource_version: 1,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            spec: SpecData {
                value: json!({"color": "blue"}),
            },
            status: None,
        };
        let serialized = serde_json::to_string(&obj).unwrap();
        // status field should be omitted when None
        assert!(!serialized.contains("\"status\""));
    }

    #[test]
    fn stored_object_deserializes_with_status() {
        let json = json!({
            "key": {"group": "example.io", "version": "v1", "kind": "Widget"},
            "metadata": {"name": "test", "labels": {}},
            "system": {"resourceVersion": 1, "createdAt": "2024-01-01T00:00:00Z", "updatedAt": "2024-01-01T00:00:00Z"},
            "spec": {"value": {"color": "blue"}},
            "status": {"value": {"phase": "Running"}}
        });
        let obj: StoredObject = serde_json::from_value(json).unwrap();
        assert!(obj.status.is_some());
        assert_eq!(obj.status.unwrap().value, json!({"phase": "Running"}));
    }

    #[test]
    fn stored_object_deserializes_without_status() {
        let json = json!({
            "key": {"group": "example.io", "version": "v1", "kind": "Widget"},
            "metadata": {"name": "test", "labels": {}},
            "system": {"resourceVersion": 1, "createdAt": "2024-01-01T00:00:00Z", "updatedAt": "2024-01-01T00:00:00Z"},
            "spec": {"value": {"color": "blue"}}
        });
        let obj: StoredObject = serde_json::from_value(json).unwrap();
        assert!(obj.status.is_none());
    }

    #[test]
    fn schema_data_with_status_schema() {
        let json = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": {"type": "object"},
            "statusSchema": {"type": "object", "properties": {"phase": {"type": "string"}}}
        });
        let data: SchemaData = serde_json::from_value(json).unwrap();
        assert!(data.status_schema.is_some());
        assert_eq!(data.status_schema.unwrap()["type"], "object");
    }

    #[test]
    fn schema_data_without_status_schema() {
        let json = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": {"type": "object"}
        });
        let data: SchemaData = serde_json::from_value(json).unwrap();
        assert!(data.status_schema.is_none());
    }

    #[test]
    fn schema_data_serializes_status_schema_as_camel_case() {
        let data = SchemaData {
            target_group: "example.io".to_string(),
            target_version: "v1".to_string(),
            target_kind: "Widget".to_string(),
            spec_schema: json!({"type": "object"}),
            status_schema: Some(json!({"type": "object"})),
        };
        let serialized = serde_json::to_string(&data).unwrap();
        assert!(serialized.contains("\"statusSchema\""));
    }

    #[test]
    fn watch_event_type_status_modified() {
        let event_type = WatchEventType::StatusModified;
        // Verify it serializes correctly
        let serialized = serde_json::to_string(&event_type).unwrap();
        assert!(serialized.contains("StatusModified"));
    }
}
