//! Namespace-ID generation (`SPEC-004 §5.2`).
//!
//! A Minecraft namespace id is `namespace:path`, lowercase ASCII with underscores, validated
//! against resource-location rules. The path is derived deterministically from a hint or the
//! display name; changing a display name does not change the namespace id (`SPEC-004 §5.2`).

/// Slugify arbitrary text into a legal resource-location path segment.
///
/// Lowercases, maps any character outside `[a-z0-9_]` to `_`, collapses runs of `_`, and trims
/// leading/trailing `_`. Returns `item` as a fallback if nothing legal remains.
pub fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_underscore = false;
    for ch in text.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '_'
        };
        if mapped == '_' {
            if !prev_underscore {
                out.push('_');
            }
            prev_underscore = true;
        } else {
            out.push(mapped);
            prev_underscore = false;
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "item".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Build a full namespace id `project_namespace:path`.
pub fn namespace_id(project_namespace: &str, path_hint: &str) -> String {
    format!("{project_namespace}:{}", slug(path_hint))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_normalizes() {
        assert_eq!(slug("Ruby Blade"), "ruby_blade");
        assert_eq!(slug("  Fancy--Sword!! "), "fancy_sword");
        assert_eq!(slug("已"), "item");
    }

    #[test]
    fn namespace_id_is_prefixed() {
        assert_eq!(
            namespace_id("amc_ruby_world", "Ruby Blade"),
            "amc_ruby_world:ruby_blade"
        );
    }
}
