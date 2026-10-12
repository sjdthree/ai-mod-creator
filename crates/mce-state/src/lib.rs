//! Project State Manager (`SPEC-003 §8`) — Phase 1.
//!
//! Owns canonical project state, candidate transactions, history, rollback, IDs, references, and
//! commit behavior. A candidate becomes canonical only after required verification succeeds
//! (`SPEC-004 §31`).
//!
//! Prohibited (`SPEC-003 §8`): must not interpret English, generate implementation, run
//! Minecraft, or mark unverified state as verified.

pub mod manager;
pub mod model;
pub mod namespace;
pub mod reference;
pub mod requirements;

pub use manager::{ProjectStateManager, Transaction, TxStatus};
pub use model::{CanonicalState, Concept, ConceptType, Recipe, RecipeType, Status};
pub use reference::{resolve, Reference, ReferenceType, Resolution};
pub use requirements::{requirements_for, VerificationLevel, VerificationRequirement};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_ruby_blade() -> serde_json::Value {
        json!({
            "operation": "create_concept",
            "payload": {
                "concept_type": "weapon",
                "subtype": "sword",
                "display_name": "Ruby Blade",
                "namespace_hint": "ruby_blade",
                "aliases": ["ruby sword"],
                "properties": { "attack_damage": 9.0, "durability": 1200 }
            }
        })
    }

    #[test]
    fn create_commit_makes_concept_canonical() {
        let mut sm = ProjectStateManager::new("Ruby World", "mce_ruby_world");
        let tx = sm.open_candidate(&[create_ruby_blade()]).unwrap();
        // Not canonical until committed.
        assert!(sm.canonical().concepts.is_empty());

        let committed = sm.commit(&tx.transaction_id, true).unwrap();
        assert_eq!(committed.status, TxStatus::Committed);
        assert_eq!(sm.canonical().active_concepts().count(), 1);

        let concept = sm.canonical().concepts.values().next().unwrap();
        assert_eq!(concept.display_name, "Ruby Blade");
        assert_eq!(concept.namespace_id, "mce_ruby_world:ruby_blade");
        assert!(concept.concept_id.starts_with("concept-"));
        // weapon default (SPEC-004 §35).
        assert_eq!(concept.properties.get("max_stack_size"), Some(&json!(1)));
    }

    #[test]
    fn mutating_transaction_carries_verification_requirements() {
        // SPEC-004 §32: every mutating operation generates at least one verification requirement.
        let mut sm = ProjectStateManager::new("Ruby World", "mce_ruby_world");
        let tx = sm.open_candidate(&[create_ruby_blade()]).unwrap();
        assert!(!tx.verification_requirements.is_empty());
        assert!(tx
            .verification_requirements
            .iter()
            .any(|r| r.assertion == "registry_exists"));
    }

    #[test]
    fn failed_verification_leaves_prior_state_unchanged() {
        // COMP-STATE-004: a failed candidate transaction leaves the verified parent unchanged.
        let mut sm = ProjectStateManager::new("Ruby World", "mce_ruby_world");
        let tx1 = sm.open_candidate(&[create_ruby_blade()]).unwrap();
        sm.commit(&tx1.transaction_id, true).unwrap();
        let hash_before = sm.canonical().state_hash();

        let tx2 = sm
            .open_candidate(&[json!({
                "operation": "create_concept",
                "payload": { "concept_type": "item", "display_name": "Scrap" }
            })])
            .unwrap();
        sm.commit(&tx2.transaction_id, false).unwrap(); // verification failed

        assert_eq!(
            sm.canonical().state_hash(),
            hash_before,
            "canonical must be unchanged"
        );
        assert_eq!(sm.canonical().active_concepts().count(), 1);
    }

    #[test]
    fn candidate_state_hash_is_deterministic() {
        // Same parent state + same operations -> same candidate_state_hash (SPEC-005 §22).
        let mut a = ProjectStateManager::new("Ruby World", "mce_ruby_world");
        let mut b = ProjectStateManager::new("Ruby World", "mce_ruby_world");
        let ops = [create_ruby_blade()];
        let tx_a = a.open_candidate(&ops).unwrap();
        let tx_b = b.open_candidate(&ops).unwrap();
        assert_eq!(tx_a.candidate_state_hash, tx_b.candidate_state_hash);
    }

    #[test]
    fn update_resolves_by_display_name_and_merges() {
        let mut sm = ProjectStateManager::new("Ruby World", "mce_ruby_world");
        let tx = sm.open_candidate(&[create_ruby_blade()]).unwrap();
        sm.commit(&tx.transaction_id, true).unwrap();

        let up = sm
            .open_candidate(&[json!({
                "operation": "update_concept",
                "payload": {
                    "target": { "reference_type": "display_name", "value": "Ruby Blade" },
                    "changes": { "properties": { "durability": 1500 } }
                }
            })])
            .unwrap();
        sm.commit(&up.transaction_id, true).unwrap();

        let concept = sm.canonical().concepts.values().next().unwrap();
        assert_eq!(concept.properties.get("durability"), Some(&json!(1500)));
        assert_eq!(concept.properties.get("attack_damage"), Some(&json!(9.0)));
    }

    #[test]
    fn revert_restores_prior_committed_state() {
        // FR-006: revert to an earlier verified transaction.
        let mut sm = ProjectStateManager::new("Ruby World", "mce_ruby_world");
        let tx1 = sm.open_candidate(&[create_ruby_blade()]).unwrap();
        let c1 = sm.commit(&tx1.transaction_id, true).unwrap();
        let hash_at_tx1 = c1.candidate_state_hash.clone();

        let tx2 = sm
            .open_candidate(&[json!({
                "operation": "update_concept",
                "payload": {
                    "target": { "reference_type": "selector", "value": "only_weapon" },
                    "changes": { "properties": { "durability": 1500 } }
                }
            })])
            .unwrap();
        sm.commit(&tx2.transaction_id, true).unwrap();
        assert_ne!(sm.canonical().state_hash(), hash_at_tx1);

        let revert = sm.revert(&tx1.transaction_id).unwrap();
        assert_eq!(revert.candidate_state_hash, hash_at_tx1);
        sm.commit(&revert.transaction_id, true).unwrap();
        assert_eq!(sm.canonical().state_hash(), hash_at_tx1);
    }

    #[test]
    fn add_recipe_resolves_references_and_stores() {
        let mut sm = ProjectStateManager::new("Ruby World", "mce_ruby_world");
        // ruby ingredient + ruby blade result
        let setup = sm
            .open_candidate(&[
                json!({"operation":"create_concept","payload":{"concept_type":"item","display_name":"Ruby","namespace_hint":"ruby"}}),
                create_ruby_blade(),
            ])
            .unwrap();
        sm.commit(&setup.transaction_id, true).unwrap();

        let tx = sm
            .open_candidate(&[json!({
                "operation": "add_recipe",
                "payload": { "recipe": {
                    "recipe_type": "shaped",
                    "pattern": [" R ", " R ", " S "],
                    "key": {
                        "R": { "reference_type": "display_name", "value": "Ruby" },
                        "S": { "reference_type": "minecraft_id", "value": "minecraft:stick" }
                    },
                    "result": { "reference_type": "display_name", "value": "Ruby Blade" },
                    "count": 1
                }}
            })])
            .unwrap();
        // recipe verification requirement generated
        assert!(tx
            .verification_requirements
            .iter()
            .any(|r| r.assertion == "recipe_produces"));
        sm.commit(&tx.transaction_id, true).unwrap();

        assert_eq!(sm.canonical().active_recipes().count(), 1);
        let recipe = sm.canonical().recipes.values().next().unwrap();
        assert_eq!(recipe.namespace_id, "mce_ruby_world:ruby_blade");
        // display-name references were normalized to concept ids / vanilla ids
        assert_eq!(recipe.result.reference_type, ReferenceType::ConceptId);
        assert!(recipe.result.value.starts_with("concept-"));
        let stick = &recipe.key["S"];
        assert_eq!(stick.reference_type, ReferenceType::MinecraftId);
    }

    #[test]
    fn delete_rejects_when_referenced() {
        let mut sm = ProjectStateManager::new("Ruby World", "mce_ruby_world");
        // ruby ingredient
        let t1 = sm
            .open_candidate(&[json!({
                "operation": "create_concept",
                "payload": { "concept_type": "item", "display_name": "Ruby" }
            })])
            .unwrap();
        sm.commit(&t1.transaction_id, true).unwrap();
        let ruby_id = sm.canonical().concepts.keys().next().unwrap().clone();

        // a weapon whose repair_material references the ruby id
        let t2 = sm
            .open_candidate(&[json!({
                "operation": "create_concept",
                "payload": {
                    "concept_type": "weapon",
                    "display_name": "Ruby Blade",
                    "properties": {
                        "attack_damage": 9.0,
                        "durability": 1200,
                        "repair_material": { "reference_type": "concept_id", "value": ruby_id }
                    }
                }
            })])
            .unwrap();
        sm.commit(&t2.transaction_id, true).unwrap();

        // deleting the ruby must be rejected (default reject_if_referenced) without mutating state.
        let hash_before = sm.canonical().state_hash();
        let err = sm.open_candidate(&[json!({
            "operation": "delete_concept",
            "payload": { "target": { "reference_type": "concept_id", "value": ruby_id } }
        })]);
        assert!(err.is_err());
        assert_eq!(sm.canonical().state_hash(), hash_before);
    }
}
