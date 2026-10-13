//! Transactions, candidate state, commit & rollback (`SPEC-004 §31`, §30) — Phase 1 work items
//! `0004`/`0005`.
//!
//! A candidate becomes canonical only after verification succeeds (`SPEC-004 §31`). A failed
//! candidate leaves the prior verified state byte-identical (`SPEC-001 NFR-003`). The same parent
//! state + same normalized operations always produce the same `candidate_state_hash`
//! (`SPEC-005 §22`).

use std::collections::HashMap;

use amc_core::{AmcError, Component, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::model::{CanonicalState, Concept, ConceptType, Recipe, RecipeType, Status};
use crate::namespace;
use crate::reference::{resolve, Reference, ReferenceType, Resolution};
use crate::requirements::{requirements_for, VerificationRequirement};

/// A transaction record (typed subset of `SPEC-004 §31`).
#[derive(Debug, Clone, Serialize)]
pub struct Transaction {
    pub transaction_id: String,
    pub parent_transaction_id: Option<String>,
    pub input_state_hash: String,
    pub candidate_state_hash: String,
    pub status: TxStatus,
    pub operations: Vec<Value>,
    /// Verification requirements generated for the mutating operations (`SPEC-004 §32`). Every
    /// mutating transaction has at least one.
    pub verification_requirements: Vec<VerificationRequirement>,
}

/// Transaction lifecycle statuses (`SPEC-004 §31`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TxStatus {
    Validated,
    Committed,
    VerificationFailed,
}

struct PendingTx {
    transaction: Transaction,
    state: CanonicalState,
}

struct CommittedRecord {
    transaction: Transaction,
    snapshot: CanonicalState,
}

/// Owns canonical state, the transaction log, and pending candidates.
pub struct ProjectStateManager {
    canonical: CanonicalState,
    tx_counter: u64,
    head: Option<String>,
    pending: HashMap<String, PendingTx>,
    history: Vec<CommittedRecord>,
}

impl ProjectStateManager {
    /// Create a manager for a new, empty project.
    pub fn new(name: impl Into<String>, namespace: impl Into<String>) -> Self {
        ProjectStateManager {
            canonical: CanonicalState::new(name, namespace),
            tx_counter: 0,
            head: None,
            pending: HashMap::new(),
            history: Vec::new(),
        }
    }

    /// Reconstruct a manager from a persisted snapshot (`SPEC-004 §31`). `committed_history` carries
    /// the `(transaction_id, snapshot)` of every previously committed transaction so cross-session
    /// `revert` (`SPEC-004 §30`, FR-006) can restore any verified state. Used by the CLI's `.amc/`
    /// project store.
    pub fn restore(
        canonical: CanonicalState,
        tx_counter: u64,
        head: Option<String>,
        committed_history: Vec<(String, CanonicalState)>,
    ) -> Self {
        let history = committed_history
            .into_iter()
            .map(|(transaction_id, snapshot)| CommittedRecord {
                transaction: Transaction {
                    transaction_id,
                    parent_transaction_id: None,
                    input_state_hash: String::new(),
                    candidate_state_hash: snapshot.state_hash(),
                    status: TxStatus::Committed,
                    operations: Vec::new(),
                    verification_requirements: Vec::new(),
                },
                snapshot,
            })
            .collect();
        ProjectStateManager {
            canonical,
            tx_counter,
            head,
            pending: HashMap::new(),
            history,
        }
    }

    /// The `(transaction_id, snapshot)` of every committed transaction, for persistence. Only
    /// committed records are returned — failed attempts are not revertible targets (`SPEC-004 §30`).
    pub fn committed_history(&self) -> Vec<(String, CanonicalState)> {
        self.history
            .iter()
            .filter(|r| r.transaction.status == TxStatus::Committed)
            .map(|r| (r.transaction.transaction_id.clone(), r.snapshot.clone()))
            .collect()
    }

    /// The current verified canonical state.
    pub fn canonical(&self) -> &CanonicalState {
        &self.canonical
    }

    /// The number of transactions opened so far — persisted so ids stay monotonic across sessions.
    pub fn tx_counter(&self) -> u64 {
        self.tx_counter
    }

    /// The id of the most recently committed transaction, if any.
    pub fn head(&self) -> Option<&str> {
        self.head.as_deref()
    }

    /// The candidate state of a pending transaction — the state that *would* become canonical if
    /// this transaction commits. Lets a caller generate + build + verify a candidate **before**
    /// committing, so only verified transactions ever touch canonical state (`SPEC-004 §31`).
    pub fn candidate_state(&self, transaction_id: &str) -> Option<&CanonicalState> {
        self.pending.get(transaction_id).map(|p| &p.state)
    }

    /// Apply validated operations to a clone of canonical state, producing a candidate
    /// transaction. Canonical state is **not** changed until [`commit`](Self::commit).
    pub fn open_candidate(&mut self, operations: &[Value]) -> Result<Transaction> {
        let input_state_hash = self.canonical.state_hash();
        let mut candidate = self.canonical.clone();
        for op in operations {
            apply_operation(&mut candidate, op)?;
        }

        // Generate verification requirements for each operation against the resulting candidate
        // (SPEC-004 §32): every mutating operation contributes at least one requirement.
        let verification_requirements: Vec<VerificationRequirement> = operations
            .iter()
            .flat_map(|op| requirements_for(op, &candidate))
            .collect();

        self.tx_counter += 1;
        let transaction = Transaction {
            transaction_id: format!("tx-{:06}", self.tx_counter),
            parent_transaction_id: self.head.clone(),
            input_state_hash,
            candidate_state_hash: candidate.state_hash(),
            status: TxStatus::Validated,
            operations: operations.to_vec(),
            verification_requirements,
        };
        self.pending.insert(
            transaction.transaction_id.clone(),
            PendingTx {
                transaction: transaction.clone(),
                state: candidate,
            },
        );
        Ok(transaction)
    }

    /// Commit a pending candidate. It becomes canonical only when `verified` is true; otherwise
    /// canonical state is left untouched (`SPEC-004 §31`, `SPEC-001 NFR-003`).
    pub fn commit(&mut self, transaction_id: &str, verified: bool) -> Result<Transaction> {
        let mut pending = self.pending.remove(transaction_id).ok_or_else(|| {
            AmcError::validation(
                Component::StateManager,
                format!("no pending transaction {transaction_id}"),
            )
        })?;

        if verified {
            self.canonical = pending.state;
            pending.transaction.status = TxStatus::Committed;
            self.head = Some(pending.transaction.transaction_id.clone());
            let snapshot = self.canonical.clone();
            self.history.push(CommittedRecord {
                transaction: pending.transaction.clone(),
                snapshot,
            });
        } else {
            pending.transaction.status = TxStatus::VerificationFailed;
            // canonical is deliberately left unchanged; record the failed attempt for traceability.
            let snapshot = self.canonical.clone();
            self.history.push(CommittedRecord {
                transaction: pending.transaction.clone(),
                snapshot,
            });
        }
        Ok(pending.transaction)
    }

    /// Revert to a previously committed transaction's state as a new candidate (`SPEC-004 §30`).
    /// Does not delete history; the reverted candidate must be verified before commit.
    pub fn revert(&mut self, target_transaction: &str) -> Result<Transaction> {
        let snapshot = self
            .history
            .iter()
            .find(|r| {
                r.transaction.transaction_id == target_transaction
                    && r.transaction.status == TxStatus::Committed
            })
            .map(|r| r.snapshot.clone())
            .ok_or_else(|| {
                AmcError::validation(
                    Component::StateManager,
                    format!("no committed transaction {target_transaction} to revert to"),
                )
            })?;

        let input_state_hash = self.canonical.state_hash();
        self.tx_counter += 1;
        let transaction = Transaction {
            transaction_id: format!("tx-{:06}", self.tx_counter),
            parent_transaction_id: self.head.clone(),
            input_state_hash,
            candidate_state_hash: snapshot.state_hash(),
            status: TxStatus::Validated,
            operations: vec![json!({ "operation": "revert_change",
                "payload": { "target_transaction": target_transaction } })],
            // The reverted candidate must still pass verification before commit (SPEC-004 §30);
            // its requirements are re-derived when the reverted state is regenerated in Phase 5.
            verification_requirements: Vec::new(),
        };
        self.pending.insert(
            transaction.transaction_id.clone(),
            PendingTx {
                transaction: transaction.clone(),
                state: snapshot,
            },
        );
        Ok(transaction)
    }
}

// ---- operation application ------------------------------------------------------------------

#[derive(Deserialize)]
struct CreatePayload {
    concept_type: ConceptType,
    #[serde(default)]
    subtype: Option<String>,
    display_name: String,
    #[serde(default)]
    namespace_hint: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    properties: Map<String, Value>,
}

#[derive(Deserialize)]
struct UpdatePayload {
    target: Reference,
    #[serde(default)]
    changes: Map<String, Value>,
}

#[derive(Deserialize)]
struct DeletePayload {
    target: Reference,
    #[serde(default)]
    dependency_policy: Option<String>,
}

#[derive(Deserialize)]
struct AddRecipePayload {
    recipe: RecipeInput,
}

#[derive(Deserialize)]
struct RecipeInput {
    recipe_type: RecipeType,
    #[serde(default)]
    pattern: Vec<String>,
    #[serde(default)]
    key: std::collections::BTreeMap<String, Reference>,
    #[serde(default)]
    ingredients: Vec<Reference>,
    #[serde(default)]
    ingredient: Option<Reference>,
    result: Reference,
    #[serde(default = "default_count")]
    count: u32,
    #[serde(default)]
    experience: Option<f64>,
    #[serde(default)]
    cooking_time_ticks: Option<i64>,
}

fn default_count() -> u32 {
    1
}

fn apply_add_recipe(state: &mut CanonicalState, payload: Value) -> Result<()> {
    let p: AddRecipePayload = parse(payload)?;
    let r = p.recipe;

    // Resolve every reference to canonical form (concept id or vanilla minecraft id).
    let result = resolve_ref_canonical(state, &r.result)?;
    let mut key = std::collections::BTreeMap::new();
    for (symbol, reference) in &r.key {
        key.insert(symbol.clone(), resolve_ref_canonical(state, reference)?);
    }
    let ingredients = r
        .ingredients
        .iter()
        .map(|reference| resolve_ref_canonical(state, reference))
        .collect::<Result<Vec<_>>>()?;
    let ingredient = match &r.ingredient {
        Some(reference) => Some(resolve_ref_canonical(state, reference)?),
        None => None,
    };

    // Name the recipe after its result's registry path.
    let result_path = registry_path_of(state, &result).ok_or_else(|| {
        AmcError::validation(
            Component::StateManager,
            "recipe result has no registry path",
        )
    })?;
    let recipe_id = amc_core::new_id("recipe");
    let namespace_id = format!("{}:{}", state.namespace, result_path);

    state.recipes.insert(
        recipe_id.clone(),
        Recipe {
            recipe_id,
            namespace_id,
            recipe_type: r.recipe_type,
            pattern: r.pattern,
            key,
            ingredients,
            ingredient,
            result,
            count: r.count,
            experience: r.experience,
            cooking_time_ticks: r.cooking_time_ticks,
            status: Status::Active,
        },
    );
    Ok(())
}

fn apply_remove_recipe(state: &mut CanonicalState, payload: Value) -> Result<()> {
    let p: DeletePayload = parse(payload)?;
    // Match a recipe by its namespace_id, or by its result concept.
    let target_id = match resolve(state, &p.target) {
        Resolution::Concept(id) => Some(id),
        _ => None,
    };
    let recipe_id = state
        .recipes
        .values()
        .find(|r| {
            r.status == Status::Active
                && (r.namespace_id == p.target.value
                    || target_id.as_deref() == Some(r.result.value.as_str()))
        })
        .map(|r| r.recipe_id.clone());

    match recipe_id {
        Some(id) => {
            if let Some(recipe) = state.recipes.get_mut(&id) {
                recipe.status = Status::Deleted;
            }
            Ok(())
        }
        None => Err(AmcError::validation(
            Component::StateManager,
            format!("no active recipe matches {}", p.target.value),
        )),
    }
}

/// Resolve a reference to canonical form: a project concept id or a vanilla minecraft id.
fn resolve_ref_canonical(state: &CanonicalState, reference: &Reference) -> Result<Reference> {
    match resolve(state, reference) {
        Resolution::Concept(id) => Ok(Reference::concept_id(id)),
        Resolution::Vanilla(v) => Ok(Reference {
            reference_type: ReferenceType::MinecraftId,
            value: v,
        }),
        Resolution::NeedsClarification(msg) | Resolution::NotFound(msg) => {
            Err(AmcError::validation(Component::StateManager, msg))
        }
    }
}

/// The registry path (portion after `namespace:`) for a canonical reference's target.
fn registry_path_of(state: &CanonicalState, reference: &Reference) -> Option<String> {
    match reference.reference_type {
        ReferenceType::ConceptId => state
            .concepts
            .get(&reference.value)
            .and_then(|c| c.namespace_id.split_once(':').map(|(_, p)| p.to_string())),
        ReferenceType::MinecraftId => reference.value.split_once(':').map(|(_, p)| p.to_string()),
        _ => None,
    }
}

fn apply_operation(state: &mut CanonicalState, op: &Value) -> Result<()> {
    let operation = op
        .get("operation")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            AmcError::validation(
                Component::StateManager,
                "operation missing `operation` field",
            )
        })?;
    let payload = op.get("payload").cloned().unwrap_or(Value::Null);

    match operation {
        "create_concept" => apply_create(state, payload),
        "update_concept" => apply_update(state, payload),
        "delete_concept" => apply_delete(state, payload),
        "add_recipe" => apply_add_recipe(state, payload),
        "remove_recipe" => apply_remove_recipe(state, payload),
        other => Err(AmcError::validation(
            Component::StateManager,
            format!("operation `{other}` is not supported by the state manager yet"),
        )),
    }
}

fn apply_create(state: &mut CanonicalState, payload: Value) -> Result<()> {
    let p: CreatePayload = parse(payload)?;
    let concept_id = amc_core::new_id("concept");
    let hint = p.namespace_hint.as_deref().unwrap_or(&p.display_name);
    let namespace_id = namespace::namespace_id(&state.namespace, hint);

    let mut properties = p.properties;
    apply_creation_defaults(p.concept_type, &mut properties);

    let concept = Concept {
        concept_id: concept_id.clone(),
        namespace_id,
        concept_type: p.concept_type,
        subtype: p.subtype,
        display_name: p.display_name,
        description: p.description,
        aliases: p.aliases,
        properties,
        appearance_id: None,
        tags: Vec::new(),
        status: Status::Active,
    };
    validate_concept_doc(&concept)?;

    state.concepts.insert(concept_id.clone(), concept);
    state.creation_order.push(concept_id.clone());
    state.last_created = Some(concept_id.clone());
    state.last_modified = Some(concept_id);
    Ok(())
}

fn apply_update(state: &mut CanonicalState, payload: Value) -> Result<()> {
    let p: UpdatePayload = parse(payload)?;
    let id = resolve_concept(state, &p.target)?;

    let concept = state.concepts.get(&id).expect("resolved id exists");
    let mut cval = serde_json::to_value(concept)?;
    if let Value::Object(cobj) = &mut cval {
        for (k, v) in &p.changes {
            if k == "properties" {
                merge_properties(cobj, v);
            } else if v.is_null() {
                cobj.remove(k);
            } else {
                cobj.insert(k.clone(), v.clone());
            }
        }
    }
    let updated: Concept = serde_json::from_value(cval)?;
    validate_concept_doc(&updated)?;

    state.concepts.insert(id.clone(), updated);
    state.last_modified = Some(id);
    Ok(())
}

fn apply_delete(state: &mut CanonicalState, payload: Value) -> Result<()> {
    let p: DeletePayload = parse(payload)?;
    let id = resolve_concept(state, &p.target)?;
    let policy = p
        .dependency_policy
        .as_deref()
        .unwrap_or("reject_if_referenced");

    if policy == "reject_if_referenced" {
        if let Some(referrer) = find_referrer(state, &id) {
            return Err(AmcError::validation(
                Component::StateManager,
                format!("cannot delete {id}: still referenced by {referrer}"),
            ));
        }
    }

    if let Some(concept) = state.concepts.get_mut(&id) {
        concept.status = Status::Deleted;
    }
    state.last_modified = Some(id);
    Ok(())
}

/// Apply gameplay-relevant defaults that must live in normalized state (`SPEC-004 §35`).
fn apply_creation_defaults(ty: ConceptType, properties: &mut Map<String, Value>) {
    if matches!(ty, ConceptType::Weapon | ConceptType::Tool) {
        properties
            .entry("max_stack_size".to_string())
            .or_insert(json!(1));
    }
}

fn merge_properties(cobj: &mut Map<String, Value>, incoming: &Value) {
    let Value::Object(incoming) = incoming else {
        return;
    };
    let props = cobj.entry("properties".to_string()).or_insert(json!({}));
    if let Value::Object(props) = props {
        for (k, v) in incoming {
            if v.is_null() {
                props.remove(k);
            } else {
                props.insert(k.clone(), v.clone());
            }
        }
    }
}

fn resolve_concept(state: &CanonicalState, reference: &Reference) -> Result<String> {
    match resolve(state, reference) {
        Resolution::Concept(id) => Ok(id),
        Resolution::Vanilla(v) => Err(AmcError::validation(
            Component::StateManager,
            format!("{v} is a vanilla id, not a modifiable project concept"),
        )),
        Resolution::NeedsClarification(msg) | Resolution::NotFound(msg) => {
            Err(AmcError::validation(Component::StateManager, msg))
        }
    }
}

/// Return the display name of any active concept whose serialized form references `id`.
fn find_referrer(state: &CanonicalState, id: &str) -> Option<String> {
    state
        .active_concepts()
        .filter(|c| c.concept_id != id)
        .find(|c| {
            serde_json::to_string(c)
                .map(|s| s.contains(id))
                .unwrap_or(false)
        })
        .map(|c| c.display_name.clone())
}

fn validate_concept_doc(concept: &Concept) -> Result<()> {
    let doc = json!({
        "schema_version": amc_schema::SCHEMA_VERSION,
        "document_type": "concept",
        "document_id": concept.concept_id,
        "revision": 1,
        "concept": concept,
    });
    amc_schema::validate(amc_schema::DocumentType::Concept, &doc)
}

fn parse<T: for<'de> Deserialize<'de>>(payload: Value) -> Result<T> {
    serde_json::from_value(payload).map_err(|e| {
        AmcError::validation(
            Component::StateManager,
            format!("invalid operation payload: {e}"),
        )
    })
}

#[cfg(test)]
mod rollback_tests {
    use super::*;

    fn create(name: &str, hint: &str) -> Value {
        json!({"operation":"create_concept","payload":{
            "concept_type":"item","display_name":name,"namespace_hint":hint}})
    }

    #[test]
    fn revert_survives_persist_and_restore() {
        // Commit tx1 (one concept), then tx2 (two concepts).
        let mut sm = ProjectStateManager::new("P", "amc_p");
        let t1 = sm.open_candidate(&[create("Ruby", "ruby")]).unwrap();
        sm.commit(&t1.transaction_id, true).unwrap();
        let t2 = sm
            .open_candidate(&[create("Sapphire", "sapphire")])
            .unwrap();
        sm.commit(&t2.transaction_id, true).unwrap();
        assert_eq!(sm.canonical().active_concepts().count(), 2);

        // Persist just what the .amc store keeps, then rebuild a fresh manager (new process).
        let restored = ProjectStateManager::restore(
            sm.canonical().clone(),
            sm.tx_counter(),
            sm.head().map(str::to_owned),
            sm.committed_history(),
        );

        // Revert to tx1 (one concept) and commit — cross-session revert works from persisted history.
        let mut restored = restored;
        let rt = restored.revert(&t1.transaction_id).unwrap();
        restored.commit(&rt.transaction_id, true).unwrap();
        assert_eq!(restored.canonical().active_concepts().count(), 1);
        assert!(restored
            .canonical()
            .active_concepts()
            .any(|c| c.display_name == "Ruby"));
    }
}
