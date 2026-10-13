//! MCL JSON Schemas and validation (`SPEC-004`).
//!
//! Schemas live under `schemas/0.1.0/` and are embedded at compile time so validation needs no
//! filesystem or network access (`SPEC-001` NFR-001). This crate is a Layer-1 deterministic
//! component (`SPEC-005 §3`): it runs on every commit and fails deterministically.

use std::sync::OnceLock;

use amc_core::{AmcError, Component};
use jsonschema::JSONSchema;
use serde_json::Value;

/// The MCL schema version these schemas define.
pub const SCHEMA_VERSION: &str = "0.1.0";

/// The MCL document/message kinds that have a schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentType {
    Envelope,
    Operation,
    Concept,
    Recipe,
    Appearance,
    Behavior,
    Transaction,
}

// Embedded schema sources. Paths are relative to this file (crates/amc-schema/src/).
const ENVELOPE_SRC: &str = include_str!("../../../schemas/0.1.0/envelope.schema.json");
const OPERATION_SRC: &str = include_str!("../../../schemas/0.1.0/operation.schema.json");
const CONCEPT_SRC: &str = include_str!("../../../schemas/0.1.0/concept.schema.json");
const RECIPE_SRC: &str = include_str!("../../../schemas/0.1.0/recipe.schema.json");
const APPEARANCE_SRC: &str = include_str!("../../../schemas/0.1.0/appearance.schema.json");
const BEHAVIOR_SRC: &str = include_str!("../../../schemas/0.1.0/behavior.schema.json");
const TRANSACTION_SRC: &str = include_str!("../../../schemas/0.1.0/transaction.schema.json");

/// A compiled set of every MCL schema.
pub struct Schemas {
    envelope: JSONSchema,
    operation: JSONSchema,
    concept: JSONSchema,
    recipe: JSONSchema,
    appearance: JSONSchema,
    behavior: JSONSchema,
    transaction: JSONSchema,
}

impl Schemas {
    /// Compile all embedded schemas. Returns an error naming the offending schema if any fails
    /// to compile (which would be a build-time authoring bug).
    pub fn load() -> Result<Self, String> {
        Ok(Self {
            envelope: compile("envelope", ENVELOPE_SRC)?,
            operation: compile("operation", OPERATION_SRC)?,
            concept: compile("concept", CONCEPT_SRC)?,
            recipe: compile("recipe", RECIPE_SRC)?,
            appearance: compile("appearance", APPEARANCE_SRC)?,
            behavior: compile("behavior", BEHAVIOR_SRC)?,
            transaction: compile("transaction", TRANSACTION_SRC)?,
        })
    }

    fn validator(&self, doc_type: DocumentType) -> &JSONSchema {
        match doc_type {
            DocumentType::Envelope => &self.envelope,
            DocumentType::Operation => &self.operation,
            DocumentType::Concept => &self.concept,
            DocumentType::Recipe => &self.recipe,
            DocumentType::Appearance => &self.appearance,
            DocumentType::Behavior => &self.behavior,
            DocumentType::Transaction => &self.transaction,
        }
    }
}

fn compile(name: &str, src: &str) -> Result<JSONSchema, String> {
    let schema: Value =
        serde_json::from_str(src).map_err(|e| format!("{name} schema is not valid JSON: {e}"))?;
    // Draft is auto-detected from the schema's `$schema` (draft 2020-12).
    JSONSchema::compile(&schema).map_err(|e| format!("{name} schema failed to compile: {e}"))
}

/// The process-wide compiled schema set, initialized on first use.
fn schemas() -> &'static Schemas {
    static SCHEMAS: OnceLock<Schemas> = OnceLock::new();
    SCHEMAS.get_or_init(|| Schemas::load().expect("embedded MCL schemas must compile"))
}

/// Validate a JSON instance against the schema for `doc_type`.
///
/// Returns `Ok(())` when valid, or an [`AmcError::Validation`] owned by the schema component and
/// carrying every validation message (`SPEC-004 §33`).
pub fn validate(doc_type: DocumentType, instance: &Value) -> Result<(), AmcError> {
    let validator = schemas().validator(doc_type);
    match validator.validate(instance) {
        Ok(()) => Ok(()),
        Err(errors) => {
            let messages: Vec<String> = errors
                .map(|e| format!("{} (at {})", e, e.instance_path))
                .collect();
            Err(AmcError::validation(Component::Schema, messages.join("; ")))
        }
    }
}

/// Convenience: whether an instance is valid against `doc_type`'s schema.
pub fn is_valid(doc_type: DocumentType, instance: &Value) -> bool {
    schemas().validator(doc_type).is_valid(instance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn all_embedded_schemas_compile() {
        Schemas::load().expect("all MCL schemas should compile");
    }

    #[test]
    fn validates_spec_create_concept_example() {
        // SPEC-004 §20 — the Ruby Blade create_concept operation.
        let op = json!({
            "schema_version": "0.1.0",
            "operation": "create_concept",
            "request_id": "req-001",
            "payload": {
                "concept_type": "weapon",
                "subtype": "sword",
                "display_name": "Ruby Blade",
                "namespace_hint": "ruby_blade",
                "description": "A durable ruby sword.",
                "aliases": ["ruby sword"],
                "properties": {
                    "attack_damage": 9.0,
                    "attack_speed": -2.4,
                    "durability": 1200,
                    "enchantability": 14,
                    "max_stack_size": 1
                },
                "appearance": {
                    "template": "sword",
                    "material_theme": "ruby",
                    "style_tags": ["polished"]
                }
            }
        });
        validate(DocumentType::Operation, &op).expect("spec create_concept example must validate");
    }

    #[test]
    fn validates_spec_shaped_recipe_example() {
        // SPEC-004 §15 — the shaped Ruby Blade recipe document.
        let recipe = json!({
            "schema_version": "0.1.0",
            "document_type": "recipe",
            "document_id": "recipe-ruby-blade",
            "revision": 1,
            "recipe": {
                "recipe_id": "recipe-ruby-blade",
                "namespace_id": "amc_ruby_world:ruby_blade",
                "recipe_type": "shaped",
                "pattern": [" R ", " R ", " S "],
                "key": {
                    "R": { "reference_type": "concept_id", "value": "concept-ruby" },
                    "S": { "reference_type": "minecraft_id", "value": "minecraft:stick" }
                },
                "result": { "reference_type": "concept_id", "value": "concept-ruby-blade" },
                "count": 1,
                "status": "active"
            }
        });
        validate(DocumentType::Recipe, &recipe).expect("spec shaped recipe example must validate");
    }

    #[test]
    fn rejects_weapon_with_stack_size_greater_than_one() {
        // SPEC-004 §11 — a weapon's max_stack_size must equal 1.
        let op = json!({
            "schema_version": "0.1.0",
            "operation": "create_concept",
            "payload": {
                "concept_type": "weapon",
                "subtype": "sword",
                "display_name": "Bad Blade",
                "properties": { "max_stack_size": 2 }
            }
        });
        assert!(
            validate(DocumentType::Operation, &op).is_err(),
            "a weapon with max_stack_size 2 must be rejected"
        );
    }

    #[test]
    fn rejects_unknown_operation() {
        let op = json!({
            "schema_version": "0.1.0",
            "operation": "summon_dragon",
            "payload": {}
        });
        assert!(!is_valid(DocumentType::Operation, &op));
    }
}
