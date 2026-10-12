//! Reference resolution (`SPEC-004 §6`, §34) — Phase 1 work item `0002`.
//!
//! Resolves an intent-stage reference to a stable concept id before any state mutation. Ambiguous
//! (multiple matches) or missing references never resolve silently — they require clarification
//! (`SPEC-004 §6`).

use serde::{Deserialize, Serialize};

use crate::model::{CanonicalState, ConceptType, Status};

/// How a reference identifies its target (`SPEC-004 §6`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceType {
    ConceptId,
    NamespaceId,
    DisplayName,
    Alias,
    Selector,
    MinecraftId,
}

/// A reference emitted by the intent stage or stored in canonical form (`SPEC-004 §6`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    pub reference_type: ReferenceType,
    pub value: String,
}

impl Reference {
    /// Convenience constructor for a concept-id reference.
    pub fn concept_id(id: impl Into<String>) -> Self {
        Reference {
            reference_type: ReferenceType::ConceptId,
            value: id.into(),
        }
    }
    /// Convenience constructor for a display-name reference.
    pub fn display_name(name: impl Into<String>) -> Self {
        Reference {
            reference_type: ReferenceType::DisplayName,
            value: name.into(),
        }
    }
}

/// The outcome of resolving a reference.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    /// Resolved to a unique project concept (stable id).
    Concept(String),
    /// A vanilla Minecraft id, passed through (not a project concept).
    Vanilla(String),
    /// Zero or multiple matches — the caller must ask the user (`SPEC-004 §6`).
    NeedsClarification(String),
    /// No such target.
    NotFound(String),
}

/// A well-formed Minecraft id: `namespace:path`, lowercase `[a-z0-9_.-]` namespace and `[a-z0-9_]`
/// path segments separated by `/`. The path part becomes a file name in the generated mod (a recipe
/// is named after its result), so anything else — `..`, absolute paths, backslashes — is refused.
pub fn is_valid_minecraft_id(id: &str) -> bool {
    let Some((ns, path)) = id.split_once(':') else {
        return false;
    };
    let ns_ok = !ns.is_empty()
        && ns
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b));
    let path_ok = path.split('/').all(|seg| {
        !seg.is_empty()
            && seg
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    });
    ns_ok && path_ok
}

/// Resolve `reference` against `state`.
pub fn resolve(state: &CanonicalState, reference: &Reference) -> Resolution {
    match reference.reference_type {
        ReferenceType::ConceptId => match state.concepts.get(&reference.value) {
            Some(c) if c.status == Status::Active => Resolution::Concept(c.concept_id.clone()),
            _ => Resolution::NotFound(format!("no active concept with id {}", reference.value)),
        },
        ReferenceType::MinecraftId if is_valid_minecraft_id(&reference.value) => {
            Resolution::Vanilla(reference.value.clone())
        }
        ReferenceType::MinecraftId => Resolution::NotFound(format!(
            "`{}` is not a valid Minecraft id (expected e.g. `minecraft:diamond`)",
            reference.value
        )),
        ReferenceType::NamespaceId => unique_match(
            state,
            |c| c.namespace_id == reference.value,
            &reference.value,
        ),
        ReferenceType::DisplayName => unique_match(
            state,
            |c| c.display_name.eq_ignore_ascii_case(&reference.value),
            &reference.value,
        ),
        ReferenceType::Alias => unique_match(
            state,
            |c| {
                c.aliases
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(&reference.value))
            },
            &reference.value,
        ),
        ReferenceType::Selector => resolve_selector(state, &reference.value),
    }
}

/// Resolve a supported contextual selector (`SPEC-004 §6`).
fn resolve_selector(state: &CanonicalState, selector: &str) -> Resolution {
    match selector {
        "last_created_concept" => match &state.last_created {
            Some(id) => Resolution::Concept(id.clone()),
            None => Resolution::NotFound("no concept has been created yet".to_string()),
        },
        "last_modified_concept" => match &state.last_modified {
            Some(id) => Resolution::Concept(id.clone()),
            None => Resolution::NotFound("no concept has been modified yet".to_string()),
        },
        "only_weapon" => unique_of_type(state, ConceptType::Weapon, selector),
        "only_block" => unique_of_type(state, ConceptType::Block, selector),
        "only_item" => unique_of_type(state, ConceptType::Item, selector),
        other => Resolution::NotFound(format!("unsupported selector {other}")),
    }
}

fn unique_match(
    state: &CanonicalState,
    pred: impl Fn(&crate::model::Concept) -> bool,
    needle: &str,
) -> Resolution {
    let matches: Vec<&str> = state
        .active_concepts()
        .filter(|c| pred(c))
        .map(|c| c.concept_id.as_str())
        .collect();
    disambiguate(matches, needle)
}

fn unique_of_type(state: &CanonicalState, ty: ConceptType, needle: &str) -> Resolution {
    let matches: Vec<&str> = state
        .active_concepts()
        .filter(|c| c.concept_type == ty)
        .map(|c| c.concept_id.as_str())
        .collect();
    disambiguate(matches, needle)
}

fn disambiguate(matches: Vec<&str>, needle: &str) -> Resolution {
    match matches.as_slice() {
        [] => Resolution::NotFound(format!("no concept matches {needle}")),
        [one] => Resolution::Concept((*one).to_string()),
        many => Resolution::NeedsClarification(format!(
            "{needle} is ambiguous — {} concepts match",
            many.len()
        )),
    }
}

#[cfg(test)]
mod id_tests {
    use super::is_valid_minecraft_id;

    #[test]
    fn minecraft_ids_are_strict() {
        for ok in [
            "minecraft:diamond",
            "minecraft:oak_planks",
            "mce_world:ruby_blade",
            "a.b-c:x/y_z",
        ] {
            assert!(is_valid_minecraft_id(ok), "{ok}");
        }
        for bad in [
            "diamond",
            "minecraft:",
            ":diamond",
            "minecraft:../../tmp/pwn",
            "minecraft:/etc/passwd",
            "minecraft:a//b",
            "minecraft:a\\b",
            "minecraft:Diamond",
            "minecraft:a.b",
        ] {
            assert!(!is_valid_minecraft_id(bad), "{bad}");
        }
    }
}
