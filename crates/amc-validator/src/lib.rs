//! Operation Validator (`SPEC-003 §7`, `SPEC-004 §33`) — Phase 1 work item `0003`.
//!
//! Runs the ordered validation sequence over proposed operations against the current verified
//! state, producing normalized operations plus a structured report. Stages 1–6 (JSON → operation
//! schema → reference resolution → property constraints → semantic constraints → capability) are
//! owned here; stages 7–8 (candidate-state schema + referential integrity) are enforced by the
//! state manager as it constructs the candidate (`amc-state`).
//!
//! Prohibited (`SPEC-003 §7`): the validator must not infer user intent, generate source code,
//! silently correct material intent, or commit state. Ambiguous references become clarifications;
//! unresolved relative terms and unsupported capabilities become errors/unsupported reports — the
//! validator never guesses.

use serde::Serialize;
use serde_json::{json, Map, Value};

use amc_state::{namespace, resolve, CanonicalState, Reference, Resolution, Status};

/// Failure-ownership tags (`SPEC-005 §17`).
mod owner {
    pub const SCHEMA: &str = "SCHEMA_VALIDATION";
    pub const REFERENCE: &str = "REFERENCE_RESOLUTION";
    pub const SEMANTIC: &str = "SEMANTIC_VALIDATION";
}

/// A single validation failure (`SPEC-004 §33` error shape).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ValidationError {
    pub code: String,
    pub operation_index: usize,
    pub path: String,
    pub message: String,
    pub owning_component: String,
    pub repairable: bool,
}

/// A required clarification — never mutates state (`SPEC-004 §27`, §37).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Clarification {
    pub operation_index: usize,
    pub reason_code: String,
    pub question: String,
}

/// An unsupported portion — never mutates state (`SPEC-004 §28`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Unsupported {
    pub operation_index: usize,
    pub reason_code: String,
    pub message: String,
}

/// The full validation result (`SPEC-003 §7` outputs).
#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    /// True only when there are no errors, clarifications, or unsupported portions — i.e. the
    /// normalized operations are safe to apply.
    pub ok: bool,
    pub normalized_operations: Vec<Value>,
    pub errors: Vec<ValidationError>,
    pub clarifications: Vec<Clarification>,
    pub unsupported: Vec<Unsupported>,
}

/// Validate and normalize proposed operations against `state`.
pub fn validate_operations(state: &CanonicalState, operations: &[Value]) -> ValidationReport {
    let mut report = ValidationReport {
        ok: false,
        normalized_operations: Vec::new(),
        errors: Vec::new(),
        clarifications: Vec::new(),
        unsupported: Vec::new(),
    };

    for (index, op) in operations.iter().enumerate() {
        match validate_op(state, index, op) {
            OpOutcome::Normalized(v) => report.normalized_operations.push(v),
            OpOutcome::Error(e) => report.errors.push(e),
            OpOutcome::Clarify(c) => report.clarifications.push(c),
            OpOutcome::Unsupported(u) => report.unsupported.push(u),
        }
    }

    report.ok = report.errors.is_empty()
        && report.clarifications.is_empty()
        && report.unsupported.is_empty();
    report
}

enum OpOutcome {
    Normalized(Value),
    Error(ValidationError),
    Clarify(Clarification),
    Unsupported(Unsupported),
}

fn err(
    index: usize,
    code: &str,
    path: &str,
    owner: &str,
    repairable: bool,
    message: impl Into<String>,
) -> OpOutcome {
    OpOutcome::Error(ValidationError {
        code: code.to_string(),
        operation_index: index,
        path: path.to_string(),
        owning_component: owner.to_string(),
        repairable,
        message: message.into(),
    })
}

fn validate_op(state: &CanonicalState, index: usize, op: &Value) -> OpOutcome {
    // Stage 1: structural.
    let Some(obj) = op.as_object() else {
        return err(
            index,
            "NOT_AN_OBJECT",
            "",
            owner::SCHEMA,
            false,
            "operation must be an object",
        );
    };
    let Some(operation) = obj.get("operation").and_then(Value::as_str) else {
        return err(
            index,
            "MISSING_OPERATION",
            "operation",
            owner::SCHEMA,
            false,
            "operation is missing the `operation` field",
        );
    };

    // Stage 2: operation schema.
    if let Err(e) = amc_schema::validate(amc_schema::DocumentType::Operation, op) {
        return err(
            index,
            "SCHEMA_INVALID",
            "",
            owner::SCHEMA,
            false,
            e.to_string(),
        );
    }

    let payload = obj.get("payload").cloned().unwrap_or(Value::Null);

    match operation {
        "create_concept" => validate_create(state, index, op, &payload),
        "update_concept" => validate_update(state, index, op, &payload),
        "delete_concept" => validate_delete(state, index, op, &payload),
        "add_recipe" => validate_add_recipe(state, index, op, &payload),
        "clarify_request" => OpOutcome::Clarify(Clarification {
            operation_index: index,
            reason_code: payload
                .get("reason_code")
                .and_then(Value::as_str)
                .unwrap_or("CLARIFICATION")
                .to_string(),
            question: payload
                .get("question")
                .and_then(Value::as_str)
                .unwrap_or("clarification required")
                .to_string(),
        }),
        "unsupported_request" => OpOutcome::Unsupported(Unsupported {
            operation_index: index,
            reason_code: payload
                .get("reason_code")
                .and_then(Value::as_str)
                .unwrap_or("UNSUPPORTED")
                .to_string(),
            message: payload
                .get("explanation")
                .and_then(Value::as_str)
                .unwrap_or("unsupported request")
                .to_string(),
        }),
        // Operations whose deep validation lands with their implementation in later phases pass
        // through with only structural + schema validation for now.
        _ => OpOutcome::Normalized(op.clone()),
    }
}

fn validate_create(state: &CanonicalState, index: usize, op: &Value, payload: &Value) -> OpOutcome {
    let concept_type = payload
        .get("concept_type")
        .and_then(Value::as_str)
        .unwrap_or("");
    let subtype = payload.get("subtype").and_then(Value::as_str);

    // Namespace uniqueness (SPEC-004 §7): a create whose identifier collides with an existing ACTIVE
    // concept generates duplicate Java (e.g. `STING` defined twice → the mod won't compile). Reject
    // with a clear message instead of letting the build fail cryptically — the user can rename or
    // refine the existing one.
    let display_name = payload
        .get("display_name")
        .and_then(Value::as_str)
        .unwrap_or("");
    let hint = payload
        .get("namespace_hint")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or(display_name);
    if !hint.is_empty() {
        let ns = namespace::namespace_id(&state.namespace, hint);
        let collides = state
            .concepts
            .values()
            .any(|c| c.status == Status::Active && c.namespace_id == ns);
        if collides {
            return OpOutcome::Error(ValidationError {
                code: "DUPLICATE_CONCEPT".to_string(),
                operation_index: index,
                path: "payload.display_name".to_string(),
                owning_component: owner::SEMANTIC.to_string(),
                repairable: false,
                message: format!(
                    "a creation named \"{display_name}\" already exists — pick a different name, or refine the existing one"
                ),
            });
        }
    }

    // Stage 6: capability.
    if !amc_generator::capability::is_concept_supported(concept_type, subtype) {
        return OpOutcome::Unsupported(Unsupported {
            operation_index: index,
            reason_code: "UNSUPPORTED_CONCEPT".to_string(),
            message: format!(
                "the generator cannot implement concept_type `{concept_type}`{}",
                subtype
                    .map(|s| format!(" subtype `{s}`"))
                    .unwrap_or_default()
            ),
        });
    }

    // Stages 4 & 5: property + semantic constraints.
    if let Some(props) = payload.get("properties").and_then(Value::as_object) {
        if let Some(e) = check_properties(index, concept_type, props) {
            return OpOutcome::Error(e);
        }
    }

    OpOutcome::Normalized(op.clone())
}

fn validate_update(state: &CanonicalState, index: usize, op: &Value, payload: &Value) -> OpOutcome {
    if payload.get("target").is_none() {
        return err(
            index,
            "MISSING_TARGET",
            "payload.target",
            owner::SCHEMA,
            false,
            "update_concept requires a target",
        );
    }
    let target = payload.get("target").unwrap();
    let (id, concept_type) = match resolve_target(state, index, target) {
        Ok(pair) => pair,
        Err(outcome) => return outcome,
    };

    // Stages 4 & 5 on the proposed property changes.
    if let Some(props) = payload
        .pointer("/changes/properties")
        .and_then(Value::as_object)
    {
        if let Some(e) = check_properties(index, &concept_type, props) {
            return OpOutcome::Error(e);
        }
    }

    OpOutcome::Normalized(normalize_target(op, &id))
}

fn validate_delete(state: &CanonicalState, index: usize, op: &Value, payload: &Value) -> OpOutcome {
    if payload.get("target").is_none() {
        return err(
            index,
            "MISSING_TARGET",
            "payload.target",
            owner::SCHEMA,
            false,
            "delete_concept requires a target",
        );
    }
    let target = payload.get("target").unwrap();
    match resolve_target(state, index, target) {
        Ok((id, _)) => OpOutcome::Normalized(normalize_target(op, &id)),
        Err(outcome) => outcome,
    }
}

/// A problem resolving a reference: needs clarification, or a hard error.
enum RefIssue {
    Clarify(String),
    Error(String),
}

/// Resolve a reference `Value` to its canonical form (concept-id or minecraft-id reference).
fn canonical_ref(state: &CanonicalState, value: &Value) -> std::result::Result<Value, RefIssue> {
    let reference: Reference = serde_json::from_value(value.clone())
        .map_err(|e| RefIssue::Error(format!("bad reference: {e}")))?;
    match resolve(state, &reference) {
        Resolution::Concept(id) => Ok(json!({ "reference_type": "concept_id", "value": id })),
        Resolution::Vanilla(v) => Ok(json!({ "reference_type": "minecraft_id", "value": v })),
        Resolution::NeedsClarification(m) => Err(RefIssue::Clarify(m)),
        Resolution::NotFound(m) => Err(RefIssue::Error(m)),
    }
}

fn validate_add_recipe(
    state: &CanonicalState,
    index: usize,
    op: &Value,
    payload: &Value,
) -> OpOutcome {
    let Some(recipe) = payload.get("recipe") else {
        return err(
            index,
            "MISSING_RECIPE",
            "payload.recipe",
            owner::SCHEMA,
            false,
            "add_recipe requires a recipe",
        );
    };
    let recipe_type = recipe
        .get("recipe_type")
        .and_then(Value::as_str)
        .unwrap_or("");

    // Stage 6: capability.
    if !amc_generator::capability::is_recipe_type_supported(recipe_type) {
        return OpOutcome::Unsupported(Unsupported {
            operation_index: index,
            reason_code: "UNSUPPORTED_RECIPE_TYPE".to_string(),
            message: format!("recipe type `{recipe_type}` is not supported"),
        });
    }

    // Stage 3: resolve & normalize every reference in the recipe.
    let mut norm = op.clone();
    let Some(recipe_mut) = norm
        .pointer_mut("/payload/recipe")
        .and_then(Value::as_object_mut)
    else {
        return err(
            index,
            "BAD_RECIPE",
            "payload.recipe",
            owner::SCHEMA,
            false,
            "recipe must be an object",
        );
    };

    // Resolve result (required).
    if recipe_mut.get("result").is_none() {
        return err(
            index,
            "MISSING_RESULT",
            "payload.recipe.result",
            owner::SCHEMA,
            false,
            "recipe requires a result",
        );
    }
    match canonical_ref(state, &recipe_mut["result"].clone()) {
        Ok(v) => {
            recipe_mut.insert("result".to_string(), v);
        }
        Err(RefIssue::Clarify(m)) => {
            return OpOutcome::Clarify(Clarification {
                operation_index: index,
                reason_code: "AMBIGUOUS_REFERENCE".to_string(),
                question: m,
            })
        }
        Err(RefIssue::Error(m)) => {
            return err(
                index,
                "REFERENCE_UNRESOLVED",
                "payload.recipe.result",
                owner::REFERENCE,
                false,
                m,
            )
        }
    }

    // Resolve key entries.
    if let Some(Value::Object(key)) = recipe_mut.get("key").cloned() {
        let mut new_key = serde_json::Map::new();
        for (symbol, reference) in &key {
            match canonical_ref(state, reference) {
                Ok(v) => {
                    new_key.insert(symbol.clone(), v);
                }
                Err(RefIssue::Clarify(m)) => {
                    return OpOutcome::Clarify(Clarification {
                        operation_index: index,
                        reason_code: "AMBIGUOUS_REFERENCE".to_string(),
                        question: m,
                    })
                }
                Err(RefIssue::Error(m)) => {
                    return err(
                        index,
                        "REFERENCE_UNRESOLVED",
                        &format!("payload.recipe.key.{symbol}"),
                        owner::REFERENCE,
                        false,
                        m,
                    )
                }
            }
        }
        recipe_mut.insert("key".to_string(), Value::Object(new_key));
    }

    // Resolve shapeless ingredients.
    if let Some(Value::Array(items)) = recipe_mut.get("ingredients").cloned() {
        let mut resolved = Vec::with_capacity(items.len());
        for (i, reference) in items.iter().enumerate() {
            match canonical_ref(state, reference) {
                Ok(v) => resolved.push(v),
                Err(RefIssue::Clarify(m)) => {
                    return OpOutcome::Clarify(Clarification {
                        operation_index: index,
                        reason_code: "AMBIGUOUS_REFERENCE".to_string(),
                        question: m,
                    })
                }
                Err(RefIssue::Error(m)) => {
                    return err(
                        index,
                        "REFERENCE_UNRESOLVED",
                        &format!("payload.recipe.ingredients[{i}]"),
                        owner::REFERENCE,
                        false,
                        m,
                    )
                }
            }
        }
        recipe_mut.insert("ingredients".to_string(), Value::Array(resolved));
    }

    // Resolve smelting/blasting single ingredient.
    if recipe_mut.get("ingredient").is_some() {
        match canonical_ref(state, &recipe_mut["ingredient"].clone()) {
            Ok(v) => {
                recipe_mut.insert("ingredient".to_string(), v);
            }
            Err(RefIssue::Clarify(m)) => {
                return OpOutcome::Clarify(Clarification {
                    operation_index: index,
                    reason_code: "AMBIGUOUS_REFERENCE".to_string(),
                    question: m,
                })
            }
            Err(RefIssue::Error(m)) => {
                return err(
                    index,
                    "REFERENCE_UNRESOLVED",
                    "payload.recipe.ingredient",
                    owner::REFERENCE,
                    false,
                    m,
                )
            }
        }
    }

    OpOutcome::Normalized(norm)
}

/// Stage 3: resolve a target reference to a concept id, returning the concept's type too.
fn resolve_target(
    state: &CanonicalState,
    index: usize,
    target: &Value,
) -> Result<(String, String), OpOutcome> {
    let reference: Reference = serde_json::from_value(target.clone()).map_err(|e| {
        err(
            index,
            "BAD_REFERENCE",
            "payload.target",
            owner::SCHEMA,
            false,
            e.to_string(),
        )
    })?;
    match resolve(state, &reference) {
        Resolution::Concept(id) => {
            let ty = state
                .concepts
                .get(&id)
                .and_then(|c| serde_json::to_value(c.concept_type).ok())
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default();
            Ok((id, ty))
        }
        Resolution::NeedsClarification(msg) => Err(OpOutcome::Clarify(Clarification {
            operation_index: index,
            reason_code: "AMBIGUOUS_TARGET".to_string(),
            question: msg,
        })),
        Resolution::NotFound(msg) => Err(err(
            index,
            "TARGET_NOT_FOUND",
            "payload.target",
            owner::REFERENCE,
            false,
            msg,
        )),
        Resolution::Vanilla(v) => Err(err(
            index,
            "TARGET_NOT_A_CONCEPT",
            "payload.target",
            owner::REFERENCE,
            false,
            format!("{v} is a vanilla id, not a modifiable project concept"),
        )),
    }
}

/// Replace `payload.target` with a canonical concept-id reference.
fn normalize_target(op: &Value, id: &str) -> Value {
    let mut norm = op.clone();
    if let Some(target) = norm.pointer_mut("/payload/target") {
        *target = json!({ "reference_type": "concept_id", "value": id });
    }
    norm
}

/// Stages 4 & 5: numeric property constraints (`SPEC-004 §10`–§14) and the rule that relative
/// terms must already be resolved to numbers (`SPEC-004 §36`). Returns the first violation.
fn check_properties(
    index: usize,
    concept_type: &str,
    props: &Map<String, Value>,
) -> Option<ValidationError> {
    // (field, must-be-integer, min, min-is-exclusive, max)
    type Rule = (&'static str, bool, Option<f64>, bool, Option<f64>);
    let rules: &[Rule] = match concept_type {
        "weapon" => &[
            ("attack_damage", false, Some(0.0), true, None),
            ("durability", true, Some(0.0), true, None),
            ("enchantability", true, Some(0.0), false, None),
            ("max_stack_size", true, Some(1.0), false, Some(1.0)),
        ],
        "tool" => &[
            ("mining_speed", false, Some(0.0), true, None),
            ("durability", true, Some(0.0), true, None),
            ("max_stack_size", true, Some(1.0), false, Some(1.0)),
        ],
        "item" => &[("max_stack_size", true, Some(1.0), false, Some(64.0))],
        "food" => &[
            ("nutrition", true, Some(0.0), false, None),
            ("saturation", false, Some(0.0), false, None),
        ],
        "block" => &[
            ("hardness", false, Some(0.0), false, None),
            ("blast_resistance", false, Some(0.0), false, None),
            ("light_level", true, Some(0.0), false, Some(15.0)),
        ],
        _ => &[],
    };

    for (field, must_be_int, min, min_exclusive, max) in rules {
        let Some(value) = props.get(*field) else {
            continue;
        };

        // Stage 5: a numeric field carrying a string means an unresolved relative term.
        let Some(n) = value.as_f64() else {
            return Some(ValidationError {
                code: "UNRESOLVED_VALUE".to_string(),
                operation_index: index,
                path: format!("properties.{field}"),
                owning_component: owner::SEMANTIC.to_string(),
                repairable: false,
                message: format!(
                    "`{field}` must be a resolved number; relative terms must be resolved by the \
                     intent adapter (SPEC-004 §36)"
                ),
            });
        };

        if *must_be_int && n.fract() != 0.0 {
            return Some(constraint_err(
                index,
                field,
                format!("`{field}` must be an integer"),
            ));
        }
        if let Some(min) = min {
            let violated = if *min_exclusive { n <= *min } else { n < *min };
            if violated {
                let rel = if *min_exclusive {
                    "greater than"
                } else {
                    "at least"
                };
                return Some(constraint_err(
                    index,
                    field,
                    format!("`{field}` must be {rel} {min}"),
                ));
            }
        }
        if let Some(max) = max {
            if n > *max {
                return Some(constraint_err(
                    index,
                    field,
                    format!("`{field}` must be at most {max}"),
                ));
            }
        }
    }
    None
}

fn constraint_err(index: usize, field: &str, message: String) -> ValidationError {
    ValidationError {
        code: "PROPERTY_CONSTRAINT".to_string(),
        operation_index: index,
        path: format!("properties.{field}"),
        owning_component: owner::SEMANTIC.to_string(),
        repairable: false,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use amc_state::ProjectStateManager;

    fn empty_state() -> CanonicalState {
        CanonicalState::new("Ruby World", "amc_ruby_world")
    }

    fn state_with_ruby_blade() -> CanonicalState {
        let mut sm = ProjectStateManager::new("Ruby World", "amc_ruby_world");
        let tx = sm
            .open_candidate(&[json!({
                "operation": "create_concept",
                "payload": {
                    "concept_type": "weapon", "subtype": "sword", "display_name": "Ruby Blade",
                    "properties": { "attack_damage": 9.0, "durability": 1200 }
                }
            })])
            .unwrap();
        sm.commit(&tx.transaction_id, true).unwrap();
        sm.canonical().clone()
    }

    #[test]
    fn valid_create_passes_and_normalizes() {
        let op = json!({
            "schema_version": "0.1.0",
            "operation": "create_concept",
            "payload": {
                "concept_type": "weapon", "subtype": "sword", "display_name": "Ruby Blade",
                "properties": { "attack_damage": 9.0, "durability": 1200, "max_stack_size": 1 }
            }
        });
        let report = validate_operations(&empty_state(), &[op]);
        assert!(report.ok, "errors: {:?}", report.errors);
        assert_eq!(report.normalized_operations.len(), 1);
    }

    #[test]
    fn duplicate_name_is_rejected() {
        // Creating a second concept whose identifier collides with an existing active one must be
        // rejected (else the generator emits duplicate Java and the mod won't compile).
        let op = json!({
            "schema_version": "0.1.0",
            "operation": "create_concept",
            "payload": {
                "concept_type": "weapon", "subtype": "dagger", "display_name": "Ruby Blade",
                "properties": { "attack_damage": 5.0 }
            }
        });
        let report = validate_operations(&state_with_ruby_blade(), std::slice::from_ref(&op));
        assert!(!report.ok);
        assert_eq!(report.errors[0].code, "DUPLICATE_CONCEPT");
        // ...but the same name in a fresh project is fine.
        assert!(validate_operations(&empty_state(), std::slice::from_ref(&op)).ok);
    }

    #[test]
    fn unsupported_subtype_is_reported() {
        let op = json!({
            "schema_version": "0.1.0",
            "operation": "create_concept",
            "payload": { "concept_type": "weapon", "subtype": "laser", "display_name": "Zap" }
        });
        let report = validate_operations(&empty_state(), &[op]);
        assert!(!report.ok);
        assert_eq!(report.unsupported.len(), 1);
        assert_eq!(report.unsupported[0].reason_code, "UNSUPPORTED_CONCEPT");
    }

    #[test]
    fn negative_durability_is_a_property_error() {
        let op = json!({
            "schema_version": "0.1.0",
            "operation": "create_concept",
            "payload": {
                "concept_type": "weapon", "display_name": "Ruby Blade",
                "properties": { "attack_damage": 9.0, "durability": -5 }
            }
        });
        let report = validate_operations(&empty_state(), &[op]);
        assert!(!report.ok);
        assert_eq!(report.errors[0].code, "PROPERTY_CONSTRAINT");
        assert_eq!(report.errors[0].owning_component, owner::SEMANTIC);
    }

    #[test]
    fn unresolved_relative_term_is_rejected() {
        // SPEC-004 §36: "stronger" must have been resolved to a number by the intent adapter.
        let op = json!({
            "schema_version": "0.1.0",
            "operation": "create_concept",
            "payload": {
                "concept_type": "weapon", "display_name": "Ruby Blade",
                "properties": { "attack_damage": 9.0, "durability": "stronger" }
            }
        });
        let report = validate_operations(&empty_state(), &[op]);
        assert!(!report.ok);
        assert_eq!(report.errors[0].code, "UNRESOLVED_VALUE");
    }

    #[test]
    fn update_resolves_and_normalizes_target_to_concept_id() {
        let state = state_with_ruby_blade();
        let op = json!({
            "schema_version": "0.1.0",
            "operation": "update_concept",
            "payload": {
                "target": { "reference_type": "display_name", "value": "Ruby Blade" },
                "changes": { "properties": { "durability": 1500 } }
            }
        });
        let report = validate_operations(&state, &[op]);
        assert!(report.ok, "errors: {:?}", report.errors);
        let target = &report.normalized_operations[0]["payload"]["target"];
        assert_eq!(target["reference_type"], "concept_id");
        assert!(target["value"].as_str().unwrap().starts_with("concept-"));
    }

    #[test]
    fn update_missing_target_is_not_found() {
        let op = json!({
            "schema_version": "0.1.0",
            "operation": "update_concept",
            "payload": {
                "target": { "reference_type": "display_name", "value": "Ghost" },
                "changes": { "properties": { "durability": 1500 } }
            }
        });
        let report = validate_operations(&empty_state(), &[op]);
        assert!(!report.ok);
        assert_eq!(report.errors[0].code, "TARGET_NOT_FOUND");
        assert_eq!(report.errors[0].owning_component, owner::REFERENCE);
    }
}
