//! Canonical, deterministic hashing (`SPEC-005 §22`).
//!
//! The same logical value must hash identically regardless of key order, and variable metadata
//! (timestamps) must not affect the hash (`SPEC-004 §4`: timestamps are metadata and must not
//! affect deterministic generation).

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Metadata keys excluded from canonical hashing because they are intentionally variable.
const VARIABLE_METADATA_KEYS: &[&str] =
    &["created_at", "updated_at", "committed_at", "run_started_at"];

/// Compute a canonical `sha256:...` hash of a JSON value.
///
/// Object keys are sorted (serde_json without the `preserve_order` feature already stores object
/// keys sorted), and [`VARIABLE_METADATA_KEYS`] are stripped recursively before hashing.
pub fn canonical_hash(value: &Value) -> String {
    let canonical = strip_metadata(value);
    // With `preserve_order` disabled, serde_json serializes object keys in sorted order, giving a
    // stable byte string for a given logical value.
    let bytes = serde_json::to_vec(&canonical).expect("Value re-serialization cannot fail");
    format!("sha256:{}", sha256_hex(&bytes))
}

/// Lowercase hex sha256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    to_hex(&Sha256::digest(bytes))
}

/// Lowercase hex of `bytes` (sha2 0.11 digests no longer format with `{:x}`).
pub fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Return a copy of `value` with variable metadata keys removed recursively.
fn strip_metadata(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(k, _)| !VARIABLE_METADATA_KEYS.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), strip_metadata(v)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(strip_metadata).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn key_order_does_not_change_hash() {
        let a = json!({ "b": 2, "a": 1, "nested": { "y": 1, "x": 2 } });
        let b = json!({ "a": 1, "nested": { "x": 2, "y": 1 }, "b": 2 });
        assert_eq!(canonical_hash(&a), canonical_hash(&b));
    }

    #[test]
    fn metadata_does_not_change_hash() {
        let without = json!({ "revision": 1, "name": "Ruby Blade" });
        let with = json!({
            "revision": 1,
            "name": "Ruby Blade",
            "created_at": "2026-07-17T12:00:00Z",
            "updated_at": "2026-07-17T12:05:00Z"
        });
        assert_eq!(canonical_hash(&without), canonical_hash(&with));
    }

    #[test]
    fn different_content_changes_hash() {
        let a = json!({ "durability": 1200 });
        let b = json!({ "durability": 1500 });
        assert_ne!(canonical_hash(&a), canonical_hash(&b));
    }

    #[test]
    fn hash_is_prefixed() {
        assert!(canonical_hash(&json!({})).starts_with("sha256:"));
    }
}
