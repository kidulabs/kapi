pub use crate::store::ResourceKey;

use std::collections::HashMap;

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContinueToken(pub String);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ListOptions {
    pub limit: Option<usize>,
    pub continue_token: Option<ContinueToken>,
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
#[derive(Debug, Clone)]
pub enum FieldSelector {
    NameEquals(String),
}

// LabelRequirement represents a single label matching condition
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
}

impl WatchFilter {
    // Returns true if the event should be delivered to a watcher with this filter
    // All matches everything; FieldSelector delegates to field-level comparison;
    // LabelSelector delegates to label-level comparison
    pub fn matches(&self, event: &WatchEvent) -> bool {
        match self {
            WatchFilter::All => true,
            WatchFilter::FieldSelector(fs) => match fs {
                FieldSelector::NameEquals(name) => event.object.metadata.name == *name,
            },
            WatchFilter::LabelSelector(ls) => ls.matches(&event.object.metadata.labels),
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum WatchEventType {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WatchEvent {
    pub event_type: WatchEventType,
    pub object: StoredObject,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserData {
    pub value: serde_json::Value,
}

// Schema data struct for type-safe access to Schema registration payloads
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaData {
    pub target_group: String,
    pub target_version: String,
    pub target_kind: String,
    pub json_schema: serde_json::Value,
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
    pub data: UserData,
}

#[cfg(test)]
mod tests {
    use super::*;

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
                data: UserData {
                    value: serde_json::json!({}),
                },
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
                data: UserData {
                    value: serde_json::json!({}),
                },
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
                data: UserData {
                    value: serde_json::json!({}),
                },
            },
        };
        assert!(!filter.matches(&event));
    }
}
