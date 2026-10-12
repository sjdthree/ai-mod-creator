//! Typed MCL data model (`SPEC-004 §7`–§18) — Phase 1 work item `0001`.
//!
//! Field names mirror `schemas/0.1.0/` so `serde_json::to_value(..)` validates against
//! `mce-schema`. Gameplay state serializes deterministically (serde_json sorts object keys and
//! [`CanonicalState`] uses ordered maps); bookkeeping used only for selector resolution is marked
//! `#[serde(skip)]` so it never affects the canonical hash (`SPEC-005 §22`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::reference::Reference;

/// A concept's high-level type (`SPEC-004 §9`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConceptType {
    Item,
    Weapon,
    Tool,
    Food,
    Block,
}

/// Active/deleted status (`SPEC-004 §8`). Deleted concepts remain in history but are not generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Active,
    Deleted,
}

/// A concept document's inner object (`SPEC-004 §8`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Concept {
    pub concept_id: String,
    pub namespace_id: String,
    pub concept_type: ConceptType,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub subtype: Option<String>,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// Typed per-subtype in the schema; held as a JSON object here and schema-validated.
    #[serde(default)]
    pub properties: Map<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub appearance_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub status: Status,
}

/// A supported recipe type (`SPEC-004 §15`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecipeType {
    Shaped,
    Shapeless,
    Smelting,
    Blasting,
}

/// A recipe document's inner object (`SPEC-004 §15`). References are stored in canonical form
/// (resolved to concept ids or vanilla minecraft ids) by the state manager.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub recipe_id: String,
    pub namespace_id: String,
    pub recipe_type: RecipeType,
    /// Shaped: up to three rows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pattern: Vec<String>,
    /// Shaped: symbol → ingredient reference.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub key: BTreeMap<String, Reference>,
    /// Shapeless: ingredient references.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ingredients: Vec<Reference>,
    /// Smelting/blasting: single input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingredient: Option<Reference>,
    pub result: Reference,
    #[serde(default = "one")]
    pub count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experience: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooking_time_ticks: Option<i64>,
    pub status: Status,
}

fn one() -> u32 {
    1
}

/// The canonical project state held by the state manager.
///
/// `concepts`/`recipes` are [`BTreeMap`]s so serialization order is stable. The `*_order` /`last_*`
/// fields are selector bookkeeping only and are excluded from serialization and hashing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalState {
    pub schema_version: String,
    pub name: String,
    pub namespace: String,
    pub concepts: BTreeMap<String, Concept>,
    #[serde(default)]
    pub recipes: BTreeMap<String, Recipe>,

    #[serde(skip)]
    pub creation_order: Vec<String>,
    #[serde(skip)]
    pub last_created: Option<String>,
    #[serde(skip)]
    pub last_modified: Option<String>,
}

impl CanonicalState {
    /// A new empty project state.
    pub fn new(name: impl Into<String>, namespace: impl Into<String>) -> Self {
        CanonicalState {
            schema_version: mce_schema::SCHEMA_VERSION.to_string(),
            name: name.into(),
            namespace: namespace.into(),
            concepts: BTreeMap::new(),
            recipes: BTreeMap::new(),
            creation_order: Vec::new(),
            last_created: None,
            last_modified: None,
        }
    }

    /// Canonical hash of the gameplay-relevant state (`SPEC-005 §22`, `SPEC-004 MCL-004`).
    ///
    /// The hash represents *generated-implementation* determinism, which is keyed by the
    /// deterministic `namespace_id` — not the internal stable ids (`concept_id`, `appearance_id`),
    /// which are ULIDs and therefore vary between runs. Those generated ids, selector bookkeeping,
    /// and timestamps are excluded so the same parent state + same normalized operations always
    /// hash the same.
    pub fn state_hash(&self) -> String {
        // Concepts, minus internal stable ids, ordered by their deterministic namespace_id.
        let mut concepts: Vec<Value> = self
            .concepts
            .values()
            .map(|c| {
                let mut v = serde_json::to_value(c).expect("concept serializes");
                if let Value::Object(o) = &mut v {
                    o.remove("concept_id");
                    o.remove("appearance_id");
                }
                v
            })
            .collect();
        concepts.sort_by(|a, b| {
            a.get("namespace_id")
                .and_then(Value::as_str)
                .cmp(&b.get("namespace_id").and_then(Value::as_str))
        });

        // Recipes, minus internal recipe_id, ordered by namespace_id.
        let mut recipes: Vec<Value> = self
            .recipes
            .values()
            .map(|r| {
                let mut v = serde_json::to_value(r).expect("recipe serializes");
                if let Value::Object(o) = &mut v {
                    o.remove("recipe_id");
                }
                v
            })
            .collect();
        recipes.sort_by(|a, b| {
            a.get("namespace_id")
                .and_then(Value::as_str)
                .cmp(&b.get("namespace_id").and_then(Value::as_str))
        });

        let view = serde_json::json!({
            "schema_version": self.schema_version,
            "name": self.name,
            "namespace": self.namespace,
            "concepts": concepts,
            "recipes": recipes,
        });
        mce_core::canonical_hash(&view)
    }

    /// Active concepts only (deleted concepts are retained but not generated).
    pub fn active_concepts(&self) -> impl Iterator<Item = &Concept> {
        self.concepts
            .values()
            .filter(|c| c.status == Status::Active)
    }

    /// Active recipes only.
    pub fn active_recipes(&self) -> impl Iterator<Item = &Recipe> {
        self.recipes.values().filter(|r| r.status == Status::Active)
    }
}
