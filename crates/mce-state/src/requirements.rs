//! Verification-requirement generation (`SPEC-004 §32`, `SPEC-005 §16`) — Phase 1 work item
//! `0006`.
//!
//! Every mutating operation produces one or more verification requirements, so nothing can be
//! committed without a defined check (`SPEC-004 §32`). The verification harness (`mce-verify`,
//! Phase 3) consumes these; it may add further mandatory derived tests, but this module
//! guarantees the minimum set.

use serde::Serialize;
use serde_json::Value;

use crate::model::CanonicalState;
use crate::reference::{resolve, Reference, Resolution};

/// The level at which a requirement is checked (`SPEC-005 §15`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationLevel {
    Static,
    Build,
    GameTest,
    Persistence,
    ClientSmoke,
}

/// A single required check attached to a transaction (`SPEC-004 §32`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VerificationRequirement {
    /// The concept (or other target) the assertion is about, as a stable reference.
    pub target: Option<String>,
    /// Assertion type from the MVP set (`SPEC-004 §32`).
    pub assertion: String,
    /// Optional property name for `property_equals` assertions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,
    pub level: VerificationLevel,
    pub required: bool,
}

impl VerificationRequirement {
    fn registry(target: &str) -> Self {
        Self {
            target: Some(target.to_string()),
            assertion: "registry_exists".to_string(),
            property: None,
            level: VerificationLevel::GameTest,
            required: true,
        }
    }

    fn property(target: &str, property: &str) -> Self {
        Self {
            target: Some(target.to_string()),
            assertion: "property_equals".to_string(),
            property: Some(property.to_string()),
            level: VerificationLevel::GameTest,
            required: true,
        }
    }

    fn simple(target: &str, assertion: &str, level: VerificationLevel) -> Self {
        Self {
            target: Some(target.to_string()),
            assertion: assertion.to_string(),
            property: None,
            level,
            required: true,
        }
    }
}

/// Generate the minimum required verification requirements for a normalized operation, resolved
/// against the resulting candidate state. Returns an empty vec for non-mutating operations
/// (clarify/unsupported/test/revert), and a non-empty set for every mutating operation.
pub fn requirements_for(operation: &Value, state: &CanonicalState) -> Vec<VerificationRequirement> {
    let op = operation
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("");
    let payload = operation.get("payload").cloned().unwrap_or(Value::Null);

    match op {
        "create_concept" => create_requirements(&payload, state),
        "update_concept" => target_property_requirements(&payload, state),
        "delete_concept" => resolve_target(&payload, state)
            .map(|id| {
                vec![VerificationRequirement::simple(
                    &id,
                    "registry_absent",
                    VerificationLevel::GameTest,
                )]
            })
            .unwrap_or_default(),
        "add_recipe" => {
            // Resolve the recipe result (if present) so the assertion targets the produced item.
            let target = payload
                .pointer("/recipe/result")
                .cloned()
                .and_then(|v| serde_json::from_value::<Reference>(v).ok())
                .and_then(|r| match resolve(state, &r) {
                    Resolution::Concept(id) => Some(id),
                    _ => None,
                });
            vec![VerificationRequirement {
                target,
                assertion: "recipe_produces".to_string(),
                property: None,
                level: VerificationLevel::GameTest,
                required: true,
            }]
        }
        _ => Vec::new(),
    }
}

fn create_requirements(payload: &Value, state: &CanonicalState) -> Vec<VerificationRequirement> {
    let concept_type = payload
        .get("concept_type")
        .and_then(Value::as_str)
        .unwrap_or("");
    // A create normalizes to a new concept; resolve it by display name against the candidate.
    let target = payload
        .get("display_name")
        .and_then(Value::as_str)
        .and_then(
            |name| match resolve(state, &Reference::display_name(name)) {
                Resolution::Concept(id) => Some(id),
                _ => None,
            },
        );
    let target = match target {
        Some(id) => id,
        None => return vec![],
    };

    let mut reqs = vec![VerificationRequirement::registry(&target)];
    let props = payload.get("properties").and_then(Value::as_object);
    match concept_type {
        "weapon" => {
            reqs.push(VerificationRequirement::property(&target, "durability"));
            reqs.push(VerificationRequirement::property(&target, "attack_damage"));
            reqs.push(VerificationRequirement::simple(
                &target,
                "weapon_damages",
                VerificationLevel::GameTest,
            ));
        }
        "block" => {
            reqs.push(VerificationRequirement::simple(
                &target,
                "block_places",
                VerificationLevel::GameTest,
            ));
            reqs.push(VerificationRequirement::simple(
                &target,
                "block_drops",
                VerificationLevel::GameTest,
            ));
        }
        _ => {
            // For any concept with declared numeric properties, assert each is preserved.
            if let Some(props) = props {
                for key in props.keys() {
                    reqs.push(VerificationRequirement::property(&target, key));
                }
            }
        }
    }
    reqs
}

fn target_property_requirements(
    payload: &Value,
    state: &CanonicalState,
) -> Vec<VerificationRequirement> {
    let Some(id) = resolve_target(payload, state) else {
        return vec![];
    };
    let mut reqs = vec![VerificationRequirement::registry(&id)];
    if let Some(changed) = payload
        .pointer("/changes/properties")
        .and_then(Value::as_object)
    {
        for key in changed.keys() {
            reqs.push(VerificationRequirement::property(&id, key));
        }
    }
    reqs
}

fn resolve_target(payload: &Value, state: &CanonicalState) -> Option<String> {
    let reference: Reference = serde_json::from_value(payload.get("target")?.clone()).ok()?;
    match resolve(state, &reference) {
        Resolution::Concept(id) => Some(id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectStateManager;
    use serde_json::json;

    fn state_with_weapon() -> (CanonicalState, Value) {
        let mut sm = ProjectStateManager::new("Ruby World", "mce_ruby_world");
        let op = json!({
            "operation": "create_concept",
            "payload": {
                "concept_type": "weapon", "subtype": "sword", "display_name": "Ruby Blade",
                "properties": { "attack_damage": 9.0, "durability": 1200 }
            }
        });
        let tx = sm.open_candidate(std::slice::from_ref(&op)).unwrap();
        sm.commit(&tx.transaction_id, true).unwrap();
        (sm.canonical().clone(), op)
    }

    #[test]
    fn create_weapon_generates_required_assertions() {
        let (state, op) = state_with_weapon();
        let reqs = requirements_for(&op, &state);
        let assertions: Vec<&str> = reqs.iter().map(|r| r.assertion.as_str()).collect();
        assert!(assertions.contains(&"registry_exists"));
        assert!(assertions.contains(&"weapon_damages"));
        assert!(reqs.iter().any(
            |r| r.assertion == "property_equals" && r.property.as_deref() == Some("durability")
        ));
    }

    #[test]
    fn mutating_op_never_yields_empty_requirements() {
        let (state, op) = state_with_weapon();
        assert!(!requirements_for(&op, &state).is_empty());
    }

    #[test]
    fn non_mutating_op_yields_no_requirements() {
        let (state, _) = state_with_weapon();
        let clarify = json!({ "operation": "clarify_request", "payload": {} });
        assert!(requirements_for(&clarify, &state).is_empty());
    }
}
