//! End-to-end test against a real OpenAI-compatible endpoint serving the promoted intent model.
//!
//! Skipped unless `MCE_INTENT_ENDPOINT` is set, so the offline test tier never needs a model. Run
//! it against a local server:
//!
//! ```bash
//! # serve the MCE Model (see docs/GETTING_STARTED.md)
//! ./scripts/fetch_model.sh
//! llama-server -m models/ai-mod-create-270smol/ai-mod-create-270smol-q8_0.gguf --port 8080 --temp 0 \
//!     --grammar-file crates/mce-intent/contract/mce-ops.gbnf
//! MCE_INTENT_ENDPOINT=http://127.0.0.1:8080/v1/chat/completions \
//!     cargo test -p mce-intent --test live_endpoint -- --nocapture
//! ```

use mce_intent::{HttpIntentAdapter, IntentAdapter};
use serde_json::json;

fn adapter() -> Option<HttpIntentAdapter> {
    let endpoint = std::env::var("MCE_INTENT_ENDPOINT").ok()?;
    let model = std::env::var("MCE_INTENT_MODEL").unwrap_or_else(|_| "default_model".into());
    Some(HttpIntentAdapter::new(endpoint, model, "mce-model-v1"))
}

#[test]
fn create_request_yields_a_create_concept() {
    let Some(a) = adapter() else {
        eprintln!("skipping: set MCE_INTENT_ENDPOINT to run");
        return;
    };
    let res = a
        .interpret(
            "Create a ruby sword called Ruby Blade with 1200 durability and 9 attack damage.",
            &json!({}),
        )
        .expect("interpret should succeed");
    let ops = res.operations.as_array().expect("operations is an array");
    assert_eq!(ops.len(), 1, "one operation: {:?}", res.operations);
    assert_eq!(ops[0]["operation"], "create_concept");
    assert_eq!(ops[0]["payload"]["display_name"], "Ruby Blade");
    assert!(!res.needs_clarification && res.unsupported_reason.is_none());
}

#[test]
fn question_only_request_mutates_nothing() {
    let Some(a) = adapter() else { return };
    let res = a
        .interpret("How many weapons do I have?", &json!({}))
        .expect("interpret should succeed");
    assert_eq!(
        res.operations.as_array().map(|o| o.len()),
        Some(0),
        "a question changes nothing: {:?}",
        res.operations
    );
}

#[test]
fn delete_targets_the_named_concept_from_context() {
    let Some(a) = adapter() else { return };
    let res = a
        .interpret(
            "Remove the Ruby Digger from the project.",
            &json!({"concepts": ["Ruby Digger"]}),
        )
        .expect("interpret should succeed");
    let ops = res.operations.as_array().expect("array");
    assert_eq!(ops[0]["operation"], "delete_concept");
    assert_eq!(ops[0]["payload"]["target"]["value"], "Ruby Digger");
}
