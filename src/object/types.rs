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

// WatchFilter determines which events a watcher receives
// Used by EventBus for predicate routing: publish() only delivers to matching watchers
#[derive(Debug, Clone)]
pub enum WatchFilter {
    All,
    FieldSelector(FieldSelector),
}

impl WatchFilter {
    // Returns true if the event should be delivered to a watcher with this filter
    // All matches everything; FieldSelector delegates to field-level comparison
    pub fn matches(&self, event: &WatchEvent) -> bool {
        match self {
            WatchFilter::All => true,
            WatchFilter::FieldSelector(fs) => match fs {
                FieldSelector::NameEquals(name) => event.object.metadata.name == *name,
            },
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
}
