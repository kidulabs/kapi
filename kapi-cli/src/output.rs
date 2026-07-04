//! Output formatting for the kapi CLI.
//!
//! Provides functions to format [`StoredObject`]s and [`WatchEvent`]s as
//! human-readable tables, JSON, or YAML.

use chrono::{DateTime, Utc};
use kapi_client::{StoredObject, WatchEvent, WatchEventType};

/// Formats a single object as a table row.
///
/// - **Namespaced** scope: `NAME  NAMESPACE  AGE`
/// - **Cluster** scope: `NAME  AGE`
pub fn format_table(obj: &StoredObject, scope: &str) -> String {
    let name = &obj.metadata.name;
    let age = format_age(&obj.system.created_at);

    if scope == "Namespaced" {
        let ns = obj.metadata.namespace.as_deref().unwrap_or("default");
        format!("{name:<30} {ns:<20} {age:<10}")
    } else {
        format!("{name:<30} {age:<10}")
    }
}

/// Formats a list of objects as a table with headers.
///
/// Appends a trailing newline so the caller can write the result directly.
pub fn format_table_list(items: &[StoredObject], scope: &str) -> String {
    let mut result = String::new();

    if scope == "Namespaced" {
        result.push_str(&format!("{:<30} {:<20} {:<10}\n", "NAME", "NAMESPACE", "AGE"));
    } else {
        result.push_str(&format!("{:<30} {:<10}\n", "NAME", "AGE"));
    }

    for obj in items {
        result.push_str(&format_table(obj, scope));
        result.push('\n');
    }
    result
}

/// Formats a single object as pretty-printed JSON.
pub fn format_json(obj: &StoredObject) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(obj)
}

/// Formats a single object as YAML.
pub fn format_yaml(obj: &StoredObject) -> Result<String, serde_yaml::Error> {
    serde_yaml::to_string(obj)
}

/// Formats a `chrono::Duration` into a human-readable relative time string.
///
/// Examples: `"5s"`, `"2m"`, `"1h"`, `"3d"`.
pub fn format_age(created_at: &DateTime<Utc>) -> String {
    let duration = Utc::now() - *created_at;
    let total_seconds = duration.num_seconds();

    if total_seconds < 60 {
        format!("{total_seconds}s")
    } else if total_seconds < 3600 {
        format!("{}m", total_seconds / 60)
    } else if total_seconds < 86400 {
        format!("{}h", total_seconds / 3600)
    } else {
        format!("{}d", total_seconds / 86400)
    }
}

/// Formats a watch event as a table row.
///
/// Columns: `EVENT_TYPE  NAME  [NAMESPACE]  AGE`
pub fn format_watch_event(event: &WatchEvent, scope: &str) -> String {
    let event_type = match event.event_type {
        WatchEventType::Added => "ADDED",
        WatchEventType::Modified => "MODIFIED",
        WatchEventType::Deleted => "DELETED",
        WatchEventType::StatusModified => "STATUS_MODIFIED",
    };

    let name = &event.object.metadata.name;
    let age = format_age(&event.object.system.created_at);

    if scope == "Namespaced" {
        let ns = event.object.metadata.namespace.as_deref().unwrap_or("default");
        format!("{event_type:<20} {name:<30} {ns:<20} {age:<10}")
    } else {
        format!("{event_type:<20} {name:<30} {age:<10}")
    }
}
