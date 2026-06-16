//! Stateless format validation for labels and annotations.
//!
//! This module provides pure functions for validating label and annotation
//! format. They have no dependencies on the store, event bus, schema registry,
//! or any I/O. They are callable from any layer (handler, service, tests).
//!
//! # Design principle
//!
//! Validation functions answer "is this request syntactically valid?" rather
//! than "does this request make sense given current state?" Stateful validation
//! (schema lookup, JSON Schema validation, OCC checks, deletion guards)
//! remains in the service layer.
//!
//! Regex patterns are compiled exactly once via `std::sync::LazyLock<Regex>`
//! and reused for the process lifetime.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::error::AppError;

/// Regex for DNS subdomain prefix format: lowercase alphanumeric with hyphens and dots.
static PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9]([-a-z0-9]*[a-z0-9])?(\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$").unwrap()
});

/// Regex for label name part: starts with alphanumeric, followed by `[-_.a-zA-Z0-9]*`.
static NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9][-_.a-zA-Z0-9]*$").unwrap());

/// Regex for label value: same format as label name, or empty.
static VALUE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9][-_.a-zA-Z0-9]*$").unwrap());

/// Validates a label key according to label validation rules.
/// Keys must be non-empty, max 256 chars, matching `[a-zA-Z0-9][-_.a-zA-Z0-9]*`
/// with optional `/` prefix separator (prefix: max 253 chars, DNS subdomain format).
pub fn validate_label_key(key: &str) -> Result<(), AppError> {
    if key.is_empty() {
        return Err(AppError::InvalidLabel("label key must not be empty".to_string()));
    }
    if key.len() > 256 {
        return Err(AppError::InvalidLabel(format!(
            "label key '{}' exceeds maximum length of 256 characters",
            key
        )));
    }

    let (_prefix, name) = if let Some(slash_pos) = key.find('/') {
        let prefix = &key[..slash_pos];
        let name = &key[slash_pos + 1..];
        if prefix.is_empty() {
            return Err(AppError::InvalidLabel(format!(
                "label key '{}' has empty prefix before '/'",
                key
            )));
        }
        if prefix.len() > 253 {
            return Err(AppError::InvalidLabel(format!(
                "label key '{}' prefix exceeds maximum length of 253 characters",
                key
            )));
        }
        // Validate prefix as DNS subdomain: lowercase alphanumeric, hyphens, dots
        if !PREFIX_RE.is_match(prefix) {
            return Err(AppError::InvalidLabel(format!(
                "label key '{}' has invalid prefix '{}' (must be a valid DNS subdomain)",
                key, prefix
            )));
        }
        (Some(prefix), name)
    } else {
        (None, key)
    };

    if name.is_empty() {
        return Err(AppError::InvalidLabel(format!(
            "label key '{}' has empty name after '/'",
            key
        )));
    }

    // Validate name part: starts with alphanumeric, followed by [-_.a-zA-Z0-9]*
    if !NAME_RE.is_match(name) {
        return Err(AppError::InvalidLabel(format!(
            "label key '{}' contains invalid characters (name part must match [a-zA-Z0-9][-_.a-zA-Z0-9]*)",
            key
        )));
    }

    Ok(())
}

/// Validates a label value according to label validation rules.
/// Values must be max 256 chars, matching `[a-zA-Z0-9][-_.a-zA-Z0-9]*` or empty string.
pub fn validate_label_value(key: &str, value: &str) -> Result<(), AppError> {
    if value.is_empty() {
        return Ok(()); // Empty values are allowed
    }
    if value.len() > 256 {
        return Err(AppError::InvalidLabel(format!(
            "label value for key '{}' exceeds maximum length of 256 characters",
            key
        )));
    }

    if !VALUE_RE.is_match(value) {
        return Err(AppError::InvalidLabel(format!(
            "label value '{}' for key '{}' contains invalid characters (must match [a-zA-Z0-9][-_.a-zA-Z0-9]* or be empty)",
            value, key
        )));
    }

    Ok(())
}

/// Validates all labels in a HashMap according to label validation rules.
/// Checks key format, value format, and length limits.
pub fn validate_labels(labels: &HashMap<String, String>) -> Result<(), AppError> {
    for (key, value) in labels {
        validate_label_key(key)?;
        validate_label_value(key, value)?;
    }
    Ok(())
}

/// Validates an annotation key: non-empty, max 256 chars, no character restrictions.
pub fn validate_annotation_key(key: &str) -> Result<(), AppError> {
    if key.is_empty() {
        return Err(AppError::InvalidAnnotation("annotation key must not be empty".to_string()));
    }
    if key.len() > 256 {
        return Err(AppError::InvalidAnnotation(format!(
            "annotation key '{}' exceeds maximum length of 256 characters",
            key
        )));
    }
    Ok(())
}

/// Validates all annotations: validates keys and total serialized size.
///
/// Key validation: non-empty, max 256 chars, no character restrictions.
/// Size validation: total serialized size must not exceed 256KB.
pub fn validate_annotations(annotations: &HashMap<String, String>) -> Result<(), AppError> {
    for key in annotations.keys() {
        validate_annotation_key(key)?;
    }

    // Check total serialized size
    let serialized_size =
        serde_json::to_string(annotations).map_err(|e| AppError::Internal(e.into()))?.len();
    if serialized_size > 256 * 1024 {
        return Err(AppError::InvalidAnnotation(format!(
            "total annotations size {serialized_size} bytes exceeds maximum of 256KB"
        )));
    }

    Ok(())
}

/// Validates a list of finalizers: max 20, each name must be label-key-shaped.
pub fn validate_finalizers(finalizers: &[String]) -> Result<(), AppError> {
    if finalizers.len() > 20 {
        return Err(AppError::InvalidFinalizer(format!(
            "too many finalizers: {} (max 20)",
            finalizers.len()
        )));
    }
    for finalizer in finalizers {
        validate_label_key(finalizer).map_err(|_| {
            AppError::InvalidFinalizer(format!("invalid finalizer name: '{finalizer}'"))
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_labels unit tests ---

    #[test]
    fn validate_labels_empty_map() {
        let labels = HashMap::new();
        assert!(validate_labels(&labels).is_ok());
    }

    #[test]
    fn validate_labels_valid_simple_keys() {
        let mut labels = HashMap::new();
        labels.insert("app".to_string(), "nginx".to_string());
        labels.insert("my-label".to_string(), "v1".to_string());
        labels.insert("label_name.v2".to_string(), "prod".to_string());
        assert!(validate_labels(&labels).is_ok());
    }

    #[test]
    fn validate_labels_valid_prefixed_keys() {
        let mut labels = HashMap::new();
        labels.insert("app.example.io/name".to_string(), "myapp".to_string());
        labels.insert("example.com/tier".to_string(), "frontend".to_string());
        assert!(validate_labels(&labels).is_ok());
    }

    #[test]
    fn validate_labels_empty_key_rejected() {
        let mut labels = HashMap::new();
        labels.insert("".to_string(), "value".to_string());
        assert!(validate_labels(&labels).is_err());
    }

    #[test]
    fn validate_labels_key_too_long() {
        let mut labels = HashMap::new();
        let long_key = "a".repeat(257);
        labels.insert(long_key, "value".to_string());
        assert!(validate_labels(&labels).is_err());
    }

    #[test]
    fn validate_labels_key_invalid_chars() {
        let mut labels = HashMap::new();
        labels.insert("invalid key!".to_string(), "value".to_string());
        assert!(validate_labels(&labels).is_err());
    }

    #[test]
    fn validate_labels_value_too_long() {
        let mut labels = HashMap::new();
        let long_value = "a".repeat(257);
        labels.insert("key".to_string(), long_value);
        assert!(validate_labels(&labels).is_err());
    }

    #[test]
    fn validate_labels_value_invalid_chars() {
        let mut labels = HashMap::new();
        labels.insert("key".to_string(), "invalid value!".to_string());
        assert!(validate_labels(&labels).is_err());
    }

    #[test]
    fn validate_labels_empty_value_allowed() {
        let mut labels = HashMap::new();
        labels.insert("key".to_string(), "".to_string());
        assert!(validate_labels(&labels).is_ok());
    }

    #[test]
    fn validate_labels_prefix_too_long() {
        let mut labels = HashMap::new();
        let long_prefix = "a".repeat(254);
        labels.insert(format!("{}/name", long_prefix), "value".to_string());
        assert!(validate_labels(&labels).is_err());
    }

    // --- validate_annotations unit tests ---

    #[test]
    fn validate_annotations_empty_map() {
        let annotations = HashMap::new();
        assert!(validate_annotations(&annotations).is_ok());
    }

    #[test]
    fn validate_annotations_valid_keys() {
        let mut annotations = HashMap::new();
        annotations.insert("description".to_string(), "my widget".to_string());
        annotations.insert("kapi.io/last-applied-config".to_string(), "{}".to_string());
        annotations.insert("example.com/path@v1".to_string(), "data".to_string());
        assert!(validate_annotations(&annotations).is_ok());
    }

    #[test]
    fn validate_annotations_empty_key_rejected() {
        let mut annotations = HashMap::new();
        annotations.insert("".to_string(), "value".to_string());
        assert!(validate_annotations(&annotations).is_err());
    }

    #[test]
    fn validate_annotations_key_too_long() {
        let mut annotations = HashMap::new();
        let long_key = "a".repeat(257);
        annotations.insert(long_key, "value".to_string());
        assert!(validate_annotations(&annotations).is_err());
    }

    #[test]
    fn validate_annotations_size_limit_exceeded() {
        let mut annotations = HashMap::new();
        let large_value = "x".repeat(256 * 1024); // > 256KB
        annotations.insert("key".to_string(), large_value);
        assert!(validate_annotations(&annotations).is_err());
    }

    #[test]
    fn validate_annotations_special_characters_accepted() {
        let mut annotations = HashMap::new();
        annotations.insert(
            "build-url".to_string(),
            "https://example.com/path?query=value&other=123".to_string(),
        );
        annotations.insert("config".to_string(), "{\"key\": \"value\"}".to_string());
        assert!(validate_annotations(&annotations).is_ok());
    }

    #[test]
    fn validate_annotations_empty_value_accepted() {
        let mut annotations = HashMap::new();
        annotations.insert("key".to_string(), "".to_string());
        assert!(validate_annotations(&annotations).is_ok());
    }

    // --- validate_finalizers unit tests ---

    #[test]
    fn validate_finalizers_empty_list() {
        let finalizers: Vec<String> = vec![];
        assert!(validate_finalizers(&finalizers).is_ok());
    }

    #[test]
    fn validate_finalizers_valid_names() {
        let finalizers = vec![
            "example.io/cleanup".to_string(),
            "kapi.io/finalizer".to_string(),
            "protection".to_string(),
        ];
        assert!(validate_finalizers(&finalizers).is_ok());
    }

    #[test]
    fn validate_finalizers_invalid_name() {
        let finalizers = vec!["invalid name with spaces".to_string()];
        assert!(matches!(validate_finalizers(&finalizers), Err(AppError::InvalidFinalizer(_))));
    }

    #[test]
    fn validate_finalizers_too_many() {
        let finalizers: Vec<String> = (0..21).map(|i| format!("finalizer-{}", i)).collect();
        assert!(matches!(
            validate_finalizers(&finalizers),
            Err(AppError::InvalidFinalizer(msg)) if msg.contains("max 20")
        ));
    }
}
