//! [`HttpIntentAdapter`] — the promoted intent model behind any OpenAI-compatible chat endpoint.
//!
//! The **AMC Model** (a Gemma-3-270M fine-tune, released as a GGUF) served by llama.cpp's
//! `llama-server`, Ollama, or `mlx_lm.server` all speak the same `/v1/chat/completions` protocol —
//! so this one adapter covers every runtime, and switching runtimes is just a different
//! `endpoint`/`model`.
//!
//! Inference mirrors training exactly (or the 270M model emits garbage): the CONTRACT is the system
//! message, the user message is built by [`build_user_message`], decoding is greedy (`temperature`
//! 0), and generation stops at gemma's `<end_of_turn>` turn terminator. The returned operations are
//! *proposed*, never trusted — the deterministic validator/oracle downstream is the real judge.

use amc_core::{AmcError, Component, Result};
use serde_json::{json, Value};

use crate::{
    build_user_message, concept_display_names, create_collision_to_update, deterministic_create,
    deterministic_delete, deterministic_recipe, deterministic_rename, deterministic_unsupported,
    deterministic_update, extract_ops, normalize_stack_size, resolve_numeric_properties, to_result,
    IntentAdapter, IntentResult, CONTRACT,
};

/// Default gemma turn terminator; the served model keeps decoding past it without an explicit stop.
const STOP: &str = "<end_of_turn>";

/// An [`IntentAdapter`] backed by an OpenAI-compatible chat-completions endpoint.
#[derive(Debug, Clone)]
pub struct HttpIntentAdapter {
    endpoint: String,
    model: String,
    version: String,
    max_tokens: u32,
    timeout_secs: u64,
}

impl HttpIntentAdapter {
    /// * `endpoint` — full chat-completions URL, e.g. `http://127.0.0.1:8080/v1/chat/completions`.
    /// * `model` — the served model id (or `"default_model"`; some servers 404 on an unknown id).
    /// * `version` — the promoted model version recorded in the transaction (`SPEC-004 §31`),
    ///   e.g. `"ai-mod-create-270smol"`.
    pub fn new(
        endpoint: impl Into<String>,
        model: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            model: model.into(),
            version: version.into(),
            max_tokens: 256,
            timeout_secs: 60,
        }
    }

    /// Cap on generated tokens (default 256). Our operation arrays are short.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Per-request timeout in seconds (default 60).
    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    fn request_body(&self, request: &str, context: &Value) -> Value {
        json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": CONTRACT},
                {"role": "user", "content": build_user_message(request, context)},
            ],
            "temperature": 0.0,
            "max_tokens": self.max_tokens,
            "stop": [STOP],
        })
    }
}

fn err(msg: impl Into<String>) -> AmcError {
    // The Intent Adapter owns every failure of model interpretation (SPEC-005 §17), including a
    // dead/slow endpoint or unparseable output — the taxonomy tag is MODEL_INTERPRETATION.
    AmcError::validation(Component::IntentAdapter, msg)
}

impl IntentAdapter for HttpIntentAdapter {
    fn model_version(&self) -> &str {
        &self.version
    }

    fn interpret(&self, request: &str, context: &Value) -> Result<IntentResult> {
        // When the desktop is refining a KNOWN creation it passes the target through in the context
        // (`--refine <name>`). A refine is an UPDATE to that target, never a fresh create — so we scope
        // to it and must NOT fall into the create path (which would collide on the name → DUPLICATE).
        let existing = concept_display_names(context);
        let refine = context
            .get("refine_target")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        if let Some(target) = refine {
            // Refine mode: an explicit target. Obvious edits are deterministic — no model: a numeric stat
            // tweak ("do 12 damage", "heals 6"), a rename ("call it Zephyr"), or a delete ("delete it").
            // Anything else fuzzy ("make it glow", a recipe) falls through to the model below, with the
            // target NAMED so it updates the right concept, not a guess.
            if let Some(op) = deterministic_update(request, Some(target), &existing) {
                return Ok(to_result(vec![op]));
            }
            if let Some(op) = deterministic_rename(request, Some(target), &existing) {
                return Ok(to_result(vec![op]));
            }
            if let Some(op) = deterministic_delete(request, Some(target), &existing) {
                return Ok(to_result(vec![op]));
            }
            // A recipe on the refined creation ("craft it from 2 sticks and 3 iron", "smelt it from
            // copper ore", "remove its recipe") — deterministic, with the result scoped to the target.
            if let Some(op) = deterministic_recipe(request, Some(target), &existing) {
                return Ok(to_result(vec![op]));
            }
        } else {
            // A recipe that names its result creation ("craft a Ruby Blade from two rubies", "smelt Ruby
            // Ore into a Ruby", "remove the recipe for the Ruby Blade"). Runs first because a crafting
            // phrasing names a buildable type word the create path would otherwise grab as a duplicate.
            if let Some(op) = deterministic_recipe(request, None, &existing) {
                return Ok(to_result(vec![op]));
            }
            // A clean "make a <type>" phrasing. The 270M model is unreliable once a project has
            // concepts; the parser is authoritative.
            if let Some(op) = deterministic_create(request) {
                let mut ops = vec![op];
                resolve_numeric_properties(&mut ops);
                normalize_stack_size(&mut ops);
                // If that name already exists, a create is impossible — it can only be an UPDATE. This
                // makes editing a pre-filled definition ("a pickaxe called Digger with 1600 durability")
                // update the existing creation, with or without refine mode / the --refine flag.
                if let Some(update) = create_collision_to_update(&ops[0], &existing) {
                    return Ok(to_result(vec![update]));
                }
                return Ok(to_result(ops));
            }
            // A rename or delete that names an existing creation ("rename Sting to Zephyr", "delete the
            // Golden Apple") — deterministic and model-free, working with no special UI mode. These run
            // before the update path because their new/old names can contain a type word ("rename it to
            // Golden Sword") that must not be mistaken for a fresh create.
            if let Some(op) = deterministic_rename(request, None, &existing) {
                return Ok(to_result(vec![op]));
            }
            if let Some(op) = deterministic_delete(request, None, &existing) {
                return Ok(to_result(vec![op]));
            }
            // Not a create phrasing → maybe an UPDATE that names an existing creation in the request
            // ("update Sting to 9 damage", "give Sting 12 attack") — works with no special UI mode.
            if let Some(op) = deterministic_update(request, None, &existing) {
                return Ok(to_result(vec![op]));
            }
            // A clearly not-yet-supported kind (armor, bow, wand, mob, …) → a clean unsupported result
            // so the UI offers close alternatives, rather than the model returning a confusing clarify.
            if let Some(op) = deterministic_unsupported(request) {
                return Ok(to_result(vec![op]));
            }
        }

        // Model call. For a refine, name the target so the model scopes the update to the right concept.
        let scoped;
        let effective_request = match refine {
            Some(t) => {
                scoped = format!("For the existing \"{t}\", {request}");
                scoped.as_str()
            }
            None => request,
        };
        let body = self.request_body(effective_request, context);
        let resp = ureq::post(&self.endpoint)
            .config()
            .timeout_global(Some(std::time::Duration::from_secs(self.timeout_secs)))
            .http_status_as_error(false)
            .build()
            .header("Content-Type", "application/json")
            .send(body.to_string());

        let mut resp = resp.map_err(|e| err(format!("model endpoint unreachable: {e}")))?;
        let status = resp.status();
        let text = resp.body_mut().read_to_string();
        if !status.is_success() {
            let detail = text.unwrap_or_default();
            return Err(err(format!(
                "model endpoint returned HTTP {}: {}",
                status.as_u16(),
                detail.trim()
            )));
        }
        let text = text.map_err(|e| err(format!("reading model response body: {e}")))?;

        let content = parse_content(&text)?;
        // We only reach the model for requests the deterministic fast-path declined (fuzzy / update /
        // delete / recipe), so a non-parseable response here is a genuine failure.
        let mut ops = extract_ops(&content).ok_or_else(|| {
            err(format!(
                "model output had no valid operations array: {:.200}",
                content
            ))
        })?;
        // SPEC-004 §36: relative terms must be resolved by the intent adapter before validation.
        // The small model sometimes emits an optional numeric stat as a word ("high") or a numeric
        // string ("9"). Resolve strings→numbers and drop unresolvable ones so the generator's sensible
        // defaults apply — a stray optional stat must never sink an otherwise-fine creation.
        resolve_numeric_properties(&mut ops);
        normalize_stack_size(&mut ops);
        Ok(to_result(ops))
    }
}

/// Pull `choices[0].message.content` out of an OpenAI-compatible chat response.
fn parse_content(text: &str) -> Result<String> {
    let v: Value =
        serde_json::from_str(text).map_err(|e| err(format!("model response was not JSON: {e}")))?;
    v.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| err("model response missing choices[0].message.content"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body_mirrors_training_prompt() {
        let a = HttpIntentAdapter::new("http://localhost:8080/v1/chat/completions", "m", "v0.2");
        let body = a.request_body(
            "Remove the Ruby Digger",
            &json!({"concepts": ["Ruby Digger"]}),
        );
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], CONTRACT);
        assert_eq!(
            body["messages"][1]["content"],
            "Current project already contains: Ruby Digger.\nRequest: Remove the Ruby Digger"
        );
        assert_eq!(body["temperature"], 0.0);
        assert_eq!(body["stop"][0], STOP);
    }

    #[test]
    fn parse_content_extracts_message() {
        let resp = json!({"choices": [{"message": {"content": "[]"}}]}).to_string();
        assert_eq!(parse_content(&resp).unwrap(), "[]");
        assert!(parse_content("not json").is_err());
        assert!(parse_content(&json!({"choices": []}).to_string()).is_err());
    }

    /// Serve one canned HTTP response on a local port and return the chat-completions URL.
    fn one_shot_server(status: &'static str, body: String) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            // Drain the whole request (headers + Content-Length body) before replying. A single
            // `read` can return just the headers; closing with the body still unread makes Windows
            // reset the connection (os error 10054) and the client never sees the reply.
            let mut req = Vec::new();
            let mut buf = [0u8; 8192];
            loop {
                let n = conn.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                req.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&req);
                if let Some(end) = text.find("\r\n\r\n") {
                    let len = text[..end]
                        .lines()
                        .find_map(|l| {
                            let (k, v) = l.split_once(':')?;
                            k.eq_ignore_ascii_case("content-length")
                                .then(|| v.trim().parse::<usize>().ok())?
                        })
                        .unwrap_or(0);
                    if req.len() >= end + 4 + len {
                        break;
                    }
                }
            }
            let reply = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = conn.write_all(reply.as_bytes());
        });
        format!("http://{addr}/v1/chat/completions")
    }

    /// A fuzzy refine the deterministic parser declines, so it must go to the model endpoint.
    fn model_bound_call(endpoint: &str) -> Result<IntentResult> {
        let a = HttpIntentAdapter::new(endpoint, "m", "v").with_timeout_secs(5);
        a.interpret(
            "make it glow and look fancy",
            &json!({"concepts": ["Sting"], "refine_target": "Sting"}),
        )
    }

    #[test]
    fn interpret_reads_a_successful_response() {
        let ops = r#"[{"operation":"delete_concept","payload":{"target":"Sting"}}]"#;
        let body = json!({"choices": [{"message": {"content": ops}}]}).to_string();
        let result = model_bound_call(&one_shot_server("200 OK", body)).expect("model call");
        assert_eq!(
            result.operations[0]["operation"], "delete_concept",
            "{result:?}"
        );
    }

    #[test]
    fn interpret_reports_http_errors_with_status_and_body() {
        let endpoint = one_shot_server("404 Not Found", r#"{"error":"no such route"}"#.into());
        let e = model_bound_call(&endpoint).unwrap_err().to_string();
        assert!(e.contains("HTTP 404"), "{e}");
        assert!(e.contains("no such route"), "{e}");
    }

    #[test]
    fn interpret_reports_an_unreachable_endpoint() {
        let e = model_bound_call("http://127.0.0.1:1/v1/chat/completions")
            .unwrap_err()
            .to_string();
        assert!(e.contains("unreachable"), "{e}");
    }

    #[test]
    fn model_version_is_reported() {
        let a = HttpIntentAdapter::new("http://x/v1/chat/completions", "m", "amc-model-v1");
        assert_eq!(a.model_version(), "amc-model-v1");
    }
}
