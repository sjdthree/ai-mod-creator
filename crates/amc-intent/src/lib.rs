//! Intent Adapter runtime bridge (`SPEC-003 §4`).
//!
//! The only component allowed to interpret natural-language intent. It converts English into
//! structured MCL operations by calling the **promoted** local model over a narrow interface
//! (ADR-0002); it does not own training/evaluation, which live with the model, outside this repo.
//!
//! Prohibited (`SPEC-003 §4`): must not modify canonical state, generate Java, repair generated
//! code, run Gradle, decide whether Minecraft behavior works, or silently remove unsupported
//! intent.
//!
//! The runtime lives behind [`IntentAdapter`] so the model/serving stack can change without
//! touching the generator (ADR-0002). [`HttpIntentAdapter`] targets any OpenAI-compatible endpoint
//! (llama.cpp server, `mlx_lm.server`, or Ollama) — swapping the served
//! model is a config change, not a recompile.

mod http;
pub mod manifest;

#[cfg(test)]
mod randomized_tests;

pub use http::HttpIntentAdapter;
pub use manifest::{locked_model, LockedModel, ModelManifest, MANIFEST_FILE};

use amc_core::{AmcError, Component, Result};
use serde_json::{json, Map, Value};

/// The operation CONTRACT sent as the system prompt. The engine **owns** it
/// (`contract/contract.txt`): it is the interface any AMC Model must be trained against. Serving with
/// a different system prompt silently wrecks output (a train/serve-skew failure caught in the Phase 4
/// setup smoke test), so the model's training pipeline vendors these exact bytes and every model ships
/// a manifest pinning their sha256 ([`ModelManifest`]); a test below pins them here.
pub const CONTRACT: &str = include_str!("../contract/contract.txt");

/// The GBNF grammar the model is served with (`contract/amc-ops.gbnf`): forces a well-formed,
/// bounded JSON array of operations. Engine-owned for the same reason as [`CONTRACT`].
pub const GRAMMAR: &str = include_str!("../contract/amc-ops.gbnf");

/// The Intent Adapter's structured result (`SPEC-003 §4` outputs, `SPEC-004 §37` partition).
#[derive(Debug, Clone)]
pub struct IntentResult {
    /// Raw operations proposed by the model (validated downstream, never trusted directly).
    pub operations: Value,
    pub needs_clarification: bool,
    pub unsupported_reason: Option<String>,
}

/// Converts English + compact project context into MCL operations. Implemented in Phase 4.
pub trait IntentAdapter {
    /// The promoted model version, recorded in the transaction (`SPEC-004 §31`).
    fn model_version(&self) -> &str;

    /// Interpret an English request against compact project context.
    fn interpret(&self, request: &str, context: &Value) -> Result<IntentResult>;
}

/// Build the user message exactly as training/eval did: a compact "Current project already
/// contains: ..." line (only when concepts exist) followed by `Request: <request>`. This MUST
/// match the training format byte-for-byte, or reference resolution and the wrong-target safety
/// metric degrade.
///
/// `context` carries existing concepts under `context["concepts"]` — an array whose entries are
/// either display-name strings or objects with a `display_name` field. Anything else yields no
/// prefix (an empty project).
pub(crate) fn build_user_message(request: &str, context: &Value) -> String {
    let names = concept_display_names(context);
    if names.is_empty() {
        format!("Request: {request}")
    } else {
        format!(
            "Current project already contains: {}.\nRequest: {request}",
            names.join(", ")
        )
    }
}

fn concept_display_names(context: &Value) -> Vec<String> {
    let items = match context.get("concepts") {
        Some(Value::Array(items)) => items,
        _ => return Vec::new(),
    };
    items
        .iter()
        .filter_map(|c| match c {
            Value::String(s) => Some(s.clone()),
            Value::Object(_) => c
                .get("display_name")
                .and_then(Value::as_str)
                .map(str::to_owned),
            _ => None,
        })
        .collect()
}

/// Extract the operations array from raw model output : scan every top-level `[...]` span and return the last one that parses to a JSON
/// array that is empty or all-objects-with-an-`operation`-key. Tolerant of trailing tokens the
/// model may emit past the first array.
pub(crate) fn extract_ops(text: &str) -> Option<Vec<Value>> {
    for frag in bracket_arrays(text).into_iter().rev() {
        let Ok(Value::Array(items)) = serde_json::from_str::<Value>(frag) else {
            continue;
        };
        let ok = items.is_empty()
            || items
                .iter()
                .all(|x| x.is_object() && x.get("operation").is_some());
        if ok {
            return Some(items);
        }
    }
    None
}

/// Every balanced top-level `[...]` span in `text`, in order of appearance.
fn bracket_arrays(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start: Option<usize> = None;
    for (i, ch) in text.char_indices() {
        if ch == '[' {
            if depth == 0 {
                start = Some(i);
            }
            depth += 1;
        } else if ch == ']' && depth > 0 {
            depth -= 1;
            if depth == 0 {
                if let Some(s) = start.take() {
                    out.push(&text[s..=i]);
                }
            }
        }
    }
    out
}

/// Every property the semantic validator treats as a number (`SPEC-004 §10`–§14). If the model emits
/// one of these as a string, we must resolve it here — validation rejects a non-numeric value outright.
const NUMERIC_PROPERTIES: &[&str] = &[
    "attack_damage",
    "attack_speed",
    "durability",
    "enchantability",
    "max_stack_size",
    "mining_speed",
    "nutrition",
    "saturation",
    "hardness",
    "blast_resistance",
    "light_level",
];

/// Resolve numeric properties the model emitted as strings: a numeric string ("9", "1.5") becomes a
/// number; an unresolvable relative term ("high", "fast") is dropped so the generator's default fills
/// in. Mutates each op's `payload.properties` in place. This is the §36 "resolved by the intent
/// adapter" step — without it, e.g. `enchantability: "high"` on a diamond dagger sinks the whole
/// creation instead of just falling back to the material default.
pub(crate) fn resolve_numeric_properties(ops: &mut [Value]) {
    for op in ops.iter_mut() {
        let Some(props) = op
            .pointer_mut("/payload/properties")
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        for key in NUMERIC_PROPERTIES {
            let Some(v) = props.get(*key) else { continue };
            // Already a valid number → leave it. Anything else (relative word, null, bool, object) is
            // an unresolved value: parse it if it's a numeric string, otherwise drop it for the default.
            if v.is_number() {
                continue;
            }
            let parsed = v.as_str().and_then(|s| s.trim().parse::<f64>().ok());
            match parsed.and_then(serde_json::Number::from_f64) {
                Some(num) => {
                    props.insert((*key).to_string(), Value::Number(num));
                }
                None => {
                    props.remove(*key); // unresolvable → use the generator's default
                }
            }
        }
    }
}

/// Deterministically flag common NOT-yet-supported kinds (armor, bows, wands, mobs, vehicles, …) as a
/// clean `unsupported_request`, so the UI shows "I can't make that, but here's what I can" with
/// suggestions — instead of the model returning a confusing clarify/recipe. Only consulted AFTER
/// [`deterministic_create`] declines, so "a sword that shoots arrows" still makes a sword.
pub(crate) fn deterministic_unsupported(request: &str) -> Option<Value> {
    const UNSUPPORTED: &[(&str, &str)] = &[
        ("wand", "wands"),
        ("staff", "magic staves"),
        ("bow", "bows"),
        ("crossbow", "crossbows"),
        ("arrow", "arrows"),
        ("gun", "guns"),
        ("rifle", "guns"),
        ("pistol", "guns"),
        ("helmet", "armor"),
        ("chestplate", "armor"),
        ("leggings", "armor"),
        ("boots", "armor"),
        ("armor", "armor"),
        ("armour", "armor"),
        ("shield", "shields"),
        ("ring", "rings"),
        ("amulet", "amulets"),
        ("necklace", "jewelry"),
        ("potion", "potions"),
        ("elixir", "potions"),
        ("trident", "tridents"),
        ("totem", "totems"),
        ("backpack", "backpacks"),
        ("mob", "mobs"),
        ("creature", "creatures"),
        ("monster", "monsters"),
        ("animal", "animals"),
        ("pet", "pets"),
        ("boss", "bosses"),
        ("biome", "biomes"),
        ("dimension", "dimensions"),
        ("structure", "structures"),
        ("enchantment", "enchantments"),
        ("spell", "spells"),
        ("vehicle", "vehicles"),
        ("car", "vehicles"),
        ("dragon", "creatures"),
        ("golem", "creatures"),
        ("zombie", "creatures"),
        ("skeleton", "creatures"),
        ("robot", "creatures"),
        ("dog", "animals"),
        ("cat", "animals"),
    ];
    let words: Vec<String> = request
        .to_lowercase()
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .collect();
    for (kw, label) in UNSUPPORTED {
        if words.iter().any(|w| w == kw) {
            return Some(json!({
                "operation": "unsupported_request",
                "payload": { "reason_code": "UNSUPPORTED_TYPE", "explanation": format!("can't make {label} yet") }
            }));
        }
    }
    None
}

/// Normalize `max_stack_size` to what the game actually allows, instead of letting a model slip sink
/// the creation: weapons/tools are never stackable (must be 1) — drop any value so the default applies;
/// items/food/blocks clamp to the legal 1..=64. This is the same "sanitize model output, don't
/// hard-reject" principle as [`resolve_numeric_properties`] (a redstone/obsidian tool with
/// `max_stack_size: 64` was being rejected outright).
pub(crate) fn normalize_stack_size(ops: &mut [Value]) {
    for op in ops.iter_mut() {
        let concept_type = op
            .pointer("/payload/concept_type")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let Some(props) = op
            .pointer_mut("/payload/properties")
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        match concept_type.as_deref() {
            Some("weapon") | Some("tool") => {
                props.remove("max_stack_size"); // always 1 → let the creation default fill it in
            }
            Some("item") | Some("food") | Some("block") => {
                if let Some(n) = props.get("max_stack_size").and_then(Value::as_f64) {
                    let clamped = n.round().clamp(1.0, 64.0) as i64;
                    props.insert("max_stack_size".to_string(), Value::from(clamped));
                }
            }
            _ => {}
        }
    }
}

/// A deterministic fast-path for the most common request shape: "a `<words>` `<type>`" (optionally
/// "called `<Name>`" and "with `<N>` `<stat>`"). The 270M model has real gaps — it wrongly returns
/// `unsupported`/`add_recipe` for e.g. any pickaxe or a copper/iron tool — so when the model punts on
/// an obviously-makeable item, we build the `create_concept` op ourselves. Returns `None` for anything
/// that doesn't clearly name a supported type (so genuinely-unsupported requests, like "a magic wand",
/// stay unsupported). The model still handles everything fuzzy; this just guarantees the obvious cases.
pub(crate) fn deterministic_create(request: &str) -> Option<Value> {
    // Never treat a modification (update/delete/recipe/refine) as a fresh create — those go to the
    // model, which owns targeting an existing concept. Only genuine "make a new thing" requests qualify.
    const MODIFY: &[&str] = &[
        "recipe", "update", "change", "delete", "remove", "rename", "stronger", "weaker", "buff",
        "nerf", "increase", "decrease", "instead", "make it", "turn it",
    ];
    let low = request.to_lowercase();
    if MODIFY.iter().any(|k| low.contains(k)) {
        return None;
    }
    // A recipe request ("craft a Ruby Blade from two rubies", "smelt copper ore") is never a fresh
    // create — even though it names a buildable type word. [`deterministic_recipe`] owns it; declining
    // here keeps a recipe whose result already exists from being mistaken for a duplicate create.
    if looks_like_recipe(&low) {
        return None;
    }
    // type keyword → (concept_type, subtype). Includes common SYNONYMS/near-types mapped to the closest
    // buildable kind, so "a stone hammer" or "a hatchet" just works instead of confusing the model. Food
    // nouns map to concept_type "food" (no subtype). (Ranged/armor/magic — bow, helmet, wand — are left
    // out on purpose: they're genuinely unsupported and should surface as such with suggestions.)
    const TYPES: &[(&str, &str, &str)] = &[
        // melee weapons → sword
        ("sword", "weapon", "sword"),
        ("blade", "weapon", "sword"),
        ("greatsword", "weapon", "sword"),
        ("broadsword", "weapon", "sword"),
        ("longsword", "weapon", "sword"),
        ("cutlass", "weapon", "sword"),
        ("katana", "weapon", "sword"),
        ("scimitar", "weapon", "sword"),
        ("rapier", "weapon", "sword"),
        ("saber", "weapon", "sword"),
        ("sabre", "weapon", "sword"),
        ("machete", "weapon", "sword"),
        ("cleaver", "weapon", "sword"),
        ("mace", "weapon", "sword"),
        ("club", "weapon", "sword"),
        ("hammer", "weapon", "sword"),
        ("warhammer", "weapon", "sword"),
        ("maul", "weapon", "sword"),
        ("spear", "weapon", "sword"),
        ("halberd", "weapon", "sword"),
        ("glaive", "weapon", "sword"),
        // short blades → dagger
        ("dagger", "weapon", "dagger"),
        ("knife", "weapon", "dagger"),
        ("dirk", "weapon", "dagger"),
        ("shiv", "weapon", "dagger"),
        ("stiletto", "weapon", "dagger"),
        // tools
        ("pickaxe", "tool", "pickaxe"),
        ("pick", "tool", "pickaxe"),
        ("drill", "tool", "pickaxe"),
        ("axe", "tool", "axe"),
        ("hatchet", "tool", "axe"),
        ("shovel", "tool", "shovel"),
        ("spade", "tool", "shovel"),
        ("trowel", "tool", "shovel"),
        ("hoe", "tool", "hoe"),
        ("mattock", "tool", "hoe"),
        ("cultivator", "tool", "hoe"),
        ("block", "block", ""),
        ("apple", "food", ""),
        ("carrot", "food", ""),
        ("bread", "food", ""),
        ("cookie", "food", ""),
        ("cake", "food", ""),
        ("pie", "food", ""),
        ("berry", "food", ""),
        ("melon", "food", ""),
        ("pepper", "food", ""),
        ("steak", "food", ""),
        ("stew", "food", ""),
        ("soup", "food", ""),
        ("potato", "food", ""),
        ("egg", "food", ""),
        ("candy", "food", ""),
        ("donut", "food", ""),
        ("muffin", "food", ""),
        ("fruit", "food", ""),
        ("snack", "food", ""),
        ("food", "food", ""),
    ];
    let bare = |w: &str| {
        w.trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase()
    };
    let words: Vec<String> = request.split_whitespace().map(|w| w.to_string()).collect();
    let lower: Vec<String> = words.iter().map(|w| bare(w)).collect();

    // The type is the LAST type-keyword in the request (nouns come after adjectives/materials).
    let (type_idx, concept_type, subtype) = {
        let mut hit = None;
        for (i, w) in lower.iter().enumerate() {
            if let Some(t) = TYPES.iter().find(|(kw, _, _)| kw == w) {
                hit = Some((i, t.1, t.2));
            }
        }
        hit?
    };

    // Display name: "called/named X…" wins; else the descriptors up to and including the type,
    // Title-Cased ("a copper pickaxe" → "Copper Pickaxe"). Numbers/stat-words are never part of a name.
    const STATWORDS: &[&str] = &[
        "with",
        "that",
        "and",
        "durability",
        "damage",
        "attack",
        "speed",
        "mining",
        "heals",
        "heal",
        "nutrition",
        "of",
        "having",
    ];
    let title = |ws: &[String]| -> String {
        ws.iter()
            .filter(|w| !w.is_empty())
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let name = {
        let called = lower.iter().position(|w| w == "called" || w == "named");
        if let Some(ci) = called {
            let mut parts = Vec::new();
            for w in words.iter().skip(ci + 1) {
                let b = bare(w);
                if b.is_empty()
                    || STATWORDS.contains(&b.as_str())
                    || b.chars().any(|c| c.is_ascii_digit())
                {
                    break;
                }
                parts.push(b);
            }
            if parts.is_empty() {
                None
            } else {
                Some(title(&parts))
            }
        } else {
            // descriptors + type: skip a leading run of imperative/filler lead-in words
            // ("make", "create a", "give me a", "please build the" …) so the NAME is the descriptor
            // phrase, not the verb — "make a cobalt sword" → "Cobalt Sword", not "Make A Cobalt Sword".
            // A wrong name here silently breaks by-name follow-ups (recipes/edits resolve by name).
            // Always leave at least the type word (never skip past `type_idx`).
            const LEADIN: &[&str] = &[
                "make",
                "makes",
                "making",
                "create",
                "creates",
                "creating",
                "build",
                "builds",
                "building",
                "add",
                "adds",
                "generate",
                "generates",
                "give",
                "gives",
                "gimme",
                "spawn",
                "design",
                "produce",
                "forge",
                "want",
                "wants",
                "need",
                "needs",
                "please",
                "can",
                "could",
                "would",
                "you",
                "i",
                "me",
                "my",
                "a",
                "an",
                "the",
                "some",
                "new",
            ];
            let mut start = 0usize;
            while start < type_idx && LEADIN.contains(&lower[start].as_str()) {
                start += 1;
            }
            let parts: Vec<String> = lower[start..=type_idx]
                .iter()
                .filter(|w| !w.is_empty() && !STATWORDS.contains(&w.as_str()))
                .cloned()
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(title(&parts))
            }
        }
    }?;

    // Stats: "<N> attack damage" / "<N> damage" → attack_damage; "<N> durability" → durability.
    let mut props = Map::new();
    for (i, w) in lower.iter().enumerate() {
        if let Ok(n) = w.parse::<f64>() {
            let ctx = lower
                .get(i + 1)
                .map(String::as_str)
                .into_iter()
                .chain(lower.get(i + 2).map(String::as_str))
                .chain(lower.get(i.wrapping_sub(1)).map(String::as_str))
                .collect::<Vec<_>>();
            if ctx.contains(&"damage") || ctx.contains(&"attack") {
                props.insert("attack_damage".into(), json!(n));
            } else if ctx.contains(&"durability") {
                props.insert("durability".into(), json!(n as i64));
            } else if ctx.contains(&"heals")
                || ctx.contains(&"heal")
                || ctx.contains(&"nutrition")
                || ctx.contains(&"hunger")
            {
                props.insert("nutrition".into(), json!(n as i64));
            }
        }
    }

    let mut payload = Map::new();
    payload.insert("concept_type".into(), json!(concept_type));
    if !subtype.is_empty() {
        payload.insert("subtype".into(), json!(subtype));
    }
    payload.insert("display_name".into(), json!(name));
    payload.insert("properties".into(), Value::Object(props));
    Some(json!({ "operation": "create_concept", "payload": payload }))
}

/// A word reduced to its bare lowercase alphanumeric core (strips surrounding punctuation).
fn bare_word(w: &str) -> String {
    w.trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase()
}

/// A deterministic UPDATE for the common "tweak an existing creation" edits. The target is resolved
/// either from an explicit `--refine <name>` (desktop refine mode) OR, when none is given, from an
/// existing creation NAMED in the request itself ("update Sting to 9 damage", "give Sting 12 attack")
/// — so an edit works whether or not the UI is in a special mode. Reuses the same number-in-context
/// parsing as [`deterministic_create`]: "do 12 damage", "9 attack", "500 durability", "heals 6",
/// "stack to 16" → `update_concept{ target, changes: { properties: {…} } }`. Reliable and model-free.
/// Returns `None` when no target resolves or no numeric stat is present (rename/recipe/prose → model).
pub(crate) fn deterministic_update(
    request: &str,
    explicit_target: Option<&str>,
    existing: &[String],
) -> Option<Value> {
    let words: Vec<String> = request
        .split_whitespace()
        .map(bare_word)
        .filter(|w| !w.is_empty())
        .collect();

    // Resolve WHICH creation to update: an explicit refine target wins; otherwise the request must name
    // an existing creation as a whole word/phrase (longest name first, so "Golden Apple" beats "Apple").
    let target: String = match explicit_target {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            let mut names: Vec<&String> = existing.iter().collect();
            names.sort_by_key(|n| std::cmp::Reverse(n.len()));
            names
                .into_iter()
                .find(|n| {
                    let nw: Vec<String> = n
                        .split_whitespace()
                        .map(bare_word)
                        .filter(|w| !w.is_empty())
                        .collect();
                    !nw.is_empty() && words.windows(nw.len()).any(|w| w == nw.as_slice())
                })?
                .clone()
        }
    };

    // Extract numeric stat edits — look ±2 words around each number for the stat keyword (update
    // phrasings put the keyword before the value with a gap: "stack to 16", "durability of 500").
    let mut props = Map::new();
    for (i, w) in words.iter().enumerate() {
        let Ok(n) = w.parse::<f64>() else { continue };
        let ctx = words
            .get(i + 1)
            .map(String::as_str)
            .into_iter()
            .chain(words.get(i + 2).map(String::as_str))
            .chain(words.get(i.wrapping_sub(1)).map(String::as_str))
            .chain(words.get(i.wrapping_sub(2)).map(String::as_str))
            .collect::<Vec<_>>();
        if ctx.contains(&"damage") || ctx.contains(&"attack") {
            props.insert("attack_damage".into(), json!(n));
        } else if ctx.contains(&"durability") {
            props.insert("durability".into(), json!(n as i64));
        } else if ctx.contains(&"heals")
            || ctx.contains(&"heal")
            || ctx.contains(&"nutrition")
            || ctx.contains(&"hunger")
        {
            props.insert("nutrition".into(), json!(n as i64));
        } else if ctx.contains(&"stack") || ctx.contains(&"stacks") || ctx.contains(&"stackable") {
            props.insert("max_stack_size".into(), json!(n as i64));
        }
    }
    if props.is_empty() {
        return None;
    }
    // The target is a structured Reference (SPEC-004 §6) resolved by display name, not a bare string.
    Some(json!({
        "operation": "update_concept",
        "payload": {
            "target": { "reference_type": "display_name", "value": target },
            "changes": { "properties": Value::Object(props) }
        }
    }))
}

/// If a `create_concept` op names a creation that ALREADY exists, it can only mean an UPDATE — you can
/// never have two creations with one name, so a "create" of an existing name is a doomed duplicate. This
/// converts it to a targeted `update_concept` carrying the same properties. That's what makes editing a
/// creation's definition robust: the refine box pre-fills "a pickaxe called Digger with 1500 durability",
/// the user changes 1500→1600, and it updates Digger — whether or not the UI is in refine mode or passed
/// `--refine`. Returns `None` when the create's name is new (a genuine new creation) or has no properties.
pub(crate) fn create_collision_to_update(create_op: &Value, existing: &[String]) -> Option<Value> {
    if create_op.get("operation").and_then(Value::as_str) != Some("create_concept") {
        return None;
    }
    let name = create_op
        .pointer("/payload/display_name")
        .and_then(Value::as_str)?;
    // Case-insensitive whole-name match against existing creations.
    let matched = existing.iter().find(|n| n.eq_ignore_ascii_case(name))?;
    let props = create_op
        .pointer("/payload/properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if props.is_empty() {
        return None; // name-only re-entry (no stat) → let it surface as a duplicate + "Update X" offer
    }
    Some(json!({
        "operation": "update_concept",
        "payload": {
            "target": { "reference_type": "display_name", "value": matched },
            "changes": { "properties": Value::Object(props) }
        }
    }))
}

/// Title-Case a list of already-bare (lowercased, punctuation-stripped) words: `["golden","apple"]`
/// → `"Golden Apple"`. Shared by the deterministic name builders.
fn title_case(words: &[String]) -> String {
    words
        .iter()
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Resolve WHICH existing creation a request refers to: an explicit refine target wins; otherwise the
/// request must contain an existing display name as a whole word/phrase (longest name first, so
/// "Golden Apple" beats "Apple"). `words` are the bare-lowercased request tokens. Returns `None` when
/// nothing resolves — callers must then decline rather than act on a guess.
fn resolve_named_target(
    explicit: Option<&str>,
    words: &[String],
    existing: &[String],
) -> Option<String> {
    if let Some(t) = explicit.filter(|t| !t.is_empty()) {
        return Some(t.to_string());
    }
    let mut names: Vec<&String> = existing.iter().collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    names
        .into_iter()
        .find(|n| {
            let nw: Vec<String> = n
                .split_whitespace()
                .map(bare_word)
                .filter(|w| !w.is_empty())
                .collect();
            !nw.is_empty() && words.windows(nw.len()).any(|w| w == nw.as_slice())
        })
        .cloned()
}

/// A deterministic RENAME — the most common edit after numeric stats, and one the 270M model handles
/// unreliably. Recognizes "rename X to Y", "rename it to Y", "call it Y", "name it Y", "change/set the
/// name to Y". The target is the explicit `--refine` concept (refine mode) OR an existing creation named
/// before the new name; the new name is the words after the marker, Title-Cased like a fresh creation.
/// Emitted as an `update_concept` that changes `display_name` — a top-level field the state manager
/// already applies (`apply_update`), so no new operation plumbing is needed. Returns `None` when it isn't
/// a rename, no target resolves, or no new name is found — those fuzzy cases still fall through to the
/// model. Never renames to the current name (a no-op) or touches a recipe ("rename the recipe" → model).
pub(crate) fn deterministic_rename(
    request: &str,
    explicit_target: Option<&str>,
    existing: &[String],
) -> Option<Value> {
    let low = request.to_lowercase();
    if low.contains("recipe") {
        return None;
    }
    // Original-case words for building the new name; a parallel bare-lower view for matching.
    let owords: Vec<String> = request.split_whitespace().map(str::to_owned).collect();
    let lwords: Vec<String> = owords.iter().map(|w| bare_word(w)).collect();

    // Is this a rename at all? Either an explicit rename/naming verb, or "change/set/give … name".
    let mentions_name = lwords.iter().any(|w| w == "rename")
        || lwords.windows(2).any(|w| w == ["new", "name"])
        || (lwords
            .iter()
            .any(|w| w == "change" || w == "set" || w == "give")
            && lwords.iter().any(|w| w == "name"));
    let call_verb = lwords.iter().enumerate().any(|(i, w)| {
        (w == "call" || w == "name" || w == "rename")
            && (explicit_target.is_some()
                || matches!(
                    lwords.get(i + 1).map(String::as_str),
                    Some("it" | "this" | "that")
                ))
    });
    if !mentions_name && !call_verb {
        return None;
    }

    // Where does the NEW name begin? After the last standalone "to"/"as" (or "is"), else right after an
    // "it"/"this"/"that" that follows a call/name/rename verb ("call it Zephyr", "rename it Zephyr").
    let name_start = lwords
        .iter()
        .rposition(|w| w == "to" || w == "as")
        .or_else(|| lwords.iter().rposition(|w| w == "is"))
        .or_else(|| {
            lwords.iter().enumerate().position(|(i, w)| {
                matches!(w.as_str(), "it" | "this" | "that")
                    && i > 0
                    && matches!(lwords[i - 1].as_str(), "call" | "name" | "rename")
            })
        })
        .map(|m| m + 1)?;

    // The new name: consecutive words from `name_start`, skipping leading articles, stopping at a
    // stop/stat word, a number, or a punctuation-only token. Title-Cased like a created concept's name.
    const STOP: &[&str] = &[
        "with", "that", "and", "the", "a", "an", "please", "now", "instead", "to", "called",
        "named", "having", "of", "from",
    ];
    let mut parts: Vec<String> = Vec::new();
    for w in owords.iter().skip(name_start) {
        let b = bare_word(w);
        let breakish =
            b.is_empty() || STOP.contains(&b.as_str()) || b.chars().any(|c| c.is_ascii_digit());
        if breakish {
            if parts.is_empty() {
                continue; // skip leading articles / filler before the name
            }
            break;
        }
        parts.push(b);
    }
    if parts.is_empty() {
        return None;
    }
    let new_name = title_case(&parts);

    // Resolve the target — for by-name resolution only look BEFORE the new name so we don't match it.
    let before = &lwords[..name_start.min(lwords.len())];
    let target = resolve_named_target(explicit_target, before, existing)?;
    if target.eq_ignore_ascii_case(&new_name) {
        return None; // renaming to the current name is a no-op
    }
    Some(json!({
        "operation": "update_concept",
        "payload": {
            "target": { "reference_type": "display_name", "value": target },
            "changes": { "display_name": new_name }
        }
    }))
}

/// A deterministic DELETE — "delete X", "remove the Golden Apple", "get rid of it". The target is the
/// explicit `--refine` concept OR an existing creation named in the request; recognizes
/// delete/remove/destroy/discard/scrap/erase/trash and the phrases "get rid of" / "throw away" /
/// "take out". Returns `None` when it isn't a delete or no known creation resolves — so we never delete
/// on a guess (fuzzy targeting stays the model's job), and "remove the recipe for X" (a recipe edit, not
/// a concept delete) is deferred to the model.
pub(crate) fn deterministic_delete(
    request: &str,
    explicit_target: Option<&str>,
    existing: &[String],
) -> Option<Value> {
    let low = request.to_lowercase();
    if low.contains("recipe") {
        return None;
    }
    let words: Vec<String> = request
        .split_whitespace()
        .map(bare_word)
        .filter(|w| !w.is_empty())
        .collect();
    const DELETE_VERBS: &[&str] = &[
        "delete", "remove", "destroy", "discard", "scrap", "erase", "trash",
    ];
    let has_verb = words.iter().any(|w| DELETE_VERBS.contains(&w.as_str()))
        || low.contains("get rid of")
        || low.contains("throw away")
        || low.contains("take out");
    if !has_verb {
        return None;
    }
    let target = resolve_named_target(explicit_target, &words, existing)?;
    Some(json!({
        "operation": "delete_concept",
        "payload": { "target": { "reference_type": "display_name", "value": target } }
    }))
}

/// Common vanilla crafting materials → their canonical `minecraft:` id, so a recipe ingredient named in
/// plain English resolves without the model. Two-word phrases are listed first and matched before the
/// bare word so "iron ingot" and "iron ore" resolve to different items than plain "iron". Intentionally a
/// curated common set — an unknown material makes the whole recipe decline to the model rather than
/// invent a wrong id (the CONTRACT's "never invent internal ids" rule).
const VANILLA_MATERIALS: &[(&str, &str)] = &[
    // two-word phrases (checked before the bare word)
    ("iron ingot", "minecraft:iron_ingot"),
    ("gold ingot", "minecraft:gold_ingot"),
    ("copper ingot", "minecraft:copper_ingot"),
    ("netherite ingot", "minecraft:netherite_ingot"),
    ("iron ore", "minecraft:iron_ore"),
    ("gold ore", "minecraft:gold_ore"),
    ("copper ore", "minecraft:copper_ore"),
    ("iron nugget", "minecraft:iron_nugget"),
    ("gold nugget", "minecraft:gold_nugget"),
    ("raw iron", "minecraft:raw_iron"),
    ("raw gold", "minecraft:raw_gold"),
    ("raw copper", "minecraft:raw_copper"),
    ("blaze rod", "minecraft:blaze_rod"),
    ("ender pearl", "minecraft:ender_pearl"),
    ("nether star", "minecraft:nether_star"),
    ("lapis lazuli", "minecraft:lapis_lazuli"),
    ("amethyst shard", "minecraft:amethyst_shard"),
    ("slime ball", "minecraft:slime_ball"),
    ("oak planks", "minecraft:oak_planks"),
    ("oak log", "minecraft:oak_log"),
    // single words
    ("stick", "minecraft:stick"),
    ("iron", "minecraft:iron_ingot"),
    ("gold", "minecraft:gold_ingot"),
    ("copper", "minecraft:copper_ingot"),
    ("netherite", "minecraft:netherite_ingot"),
    ("diamond", "minecraft:diamond"),
    ("emerald", "minecraft:emerald"),
    ("redstone", "minecraft:redstone"),
    ("lapis", "minecraft:lapis_lazuli"),
    ("coal", "minecraft:coal"),
    ("charcoal", "minecraft:charcoal"),
    ("quartz", "minecraft:quartz"),
    ("amethyst", "minecraft:amethyst_shard"),
    ("string", "minecraft:string"),
    ("leather", "minecraft:leather"),
    ("feather", "minecraft:feather"),
    ("flint", "minecraft:flint"),
    ("bone", "minecraft:bone"),
    ("gunpowder", "minecraft:gunpowder"),
    ("slimeball", "minecraft:slime_ball"),
    ("slime", "minecraft:slime_ball"),
    ("clay", "minecraft:clay_ball"),
    ("brick", "minecraft:brick"),
    ("paper", "minecraft:paper"),
    ("sugar", "minecraft:sugar"),
    ("wheat", "minecraft:wheat"),
    ("egg", "minecraft:egg"),
    ("apple", "minecraft:apple"),
    ("planks", "minecraft:oak_planks"),
    ("plank", "minecraft:oak_planks"),
    ("wood", "minecraft:oak_planks"),
    ("log", "minecraft:oak_log"),
    ("cobblestone", "minecraft:cobblestone"),
    ("cobble", "minecraft:cobblestone"),
    ("stone", "minecraft:stone"),
    ("obsidian", "minecraft:obsidian"),
    ("glass", "minecraft:glass"),
    ("blaze", "minecraft:blaze_rod"),
];

/// Does this request read like a recipe (crafting/smelting) rather than a create/update/rename/delete?
/// True when it mentions a recipe outright, a furnace/smelting verb, or an ingredient-introducing
/// preposition ("from"/"using"/"out of"/"made of"/"made from"). Deliberately excludes "with" — that
/// carries stat phrasings ("with 9 damage") far more often than ingredients, and a false positive here
/// would wrongly steer a plain create toward the recipe path.
fn looks_like_recipe(low: &str) -> bool {
    let words: Vec<&str> = low
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let has_recipe = words.iter().any(|w| *w == "recipe" || *w == "recipes");
    let has_smelt = words.iter().any(|w| {
        matches!(
            *w,
            "smelt" | "smelted" | "smelting" | "blast" | "blasted" | "blasting" | "furnace"
        )
    });
    let has_prep = words.iter().enumerate().any(|(i, w)| {
        *w == "from"
            || *w == "using"
            || (*w == "of" && i > 0 && matches!(words[i - 1], "out" | "made"))
    });
    has_recipe || has_smelt || has_prep
}

/// Reduce a plural noun phrase to its singular so "two rubies"/"sticks" resolve like "ruby"/"stick".
/// Handles the common `-ies → -y` and trailing `-s` cases; leaves `-ss` words ("glass") alone.
fn singularize(s: &str) -> String {
    if let Some(stem) = s.strip_suffix("ies") {
        return format!("{stem}y");
    }
    if let Some(stem) = s.strip_suffix('s') {
        if !s.ends_with("ss") && stem.len() > 1 {
            return stem.to_string();
        }
    }
    s.to_string()
}

/// A leading count word/number → its integer ("two" → 2, "3" → 3, "a"/"an" → 1). Only 1–9 (a crafting
/// grid holds at most nine items); `None` for anything that isn't a quantity.
fn parse_quantity(tok: &str) -> Option<u32> {
    if let Ok(n) = tok.parse::<u32>() {
        return (1..=9).contains(&n).then_some(n);
    }
    const WORDS: &[(&str, u32)] = &[
        ("a", 1),
        ("an", 1),
        ("one", 1),
        ("single", 1),
        ("two", 2),
        ("pair", 2),
        ("couple", 2),
        ("three", 3),
        ("four", 4),
        ("five", 5),
        ("six", 6),
        ("seven", 7),
        ("eight", 8),
        ("nine", 9),
    ];
    WORDS.iter().find(|(w, _)| *w == tok).map(|(_, n)| *n)
}

/// Resolve one ingredient noun phrase (bare-lowercased words) to a canonical recipe reference: an
/// existing creation by display name (matched whole, plural-aware — "rubies" → "Ruby"), else a known
/// vanilla material id. Returns `None` for anything we can't resolve, so an unknown ingredient defers
/// the whole recipe to the model instead of inventing an id.
fn resolve_ingredient_ref(phrase: &[String], existing: &[String]) -> Option<Value> {
    if phrase.is_empty() {
        return None;
    }
    let joined = phrase.join(" ");
    let singular = singularize(&joined);
    // 1. An existing creation (whole display name, case-insensitive), matched raw or singularized.
    if let Some(name) = existing
        .iter()
        .find(|n| n.eq_ignore_ascii_case(&joined) || n.eq_ignore_ascii_case(&singular))
    {
        return Some(json!({ "reference_type": "display_name", "value": name }));
    }
    // 2. A known vanilla material — try the full phrase, its singular, then the last word (so "block of
    //    iron"/"iron ingots" still land on iron).
    let last = phrase.last().expect("non-empty");
    for cand in [
        joined.as_str(),
        singular.as_str(),
        last.as_str(),
        singularize(last).as_str(),
    ] {
        if let Some((_, id)) = VANILLA_MATERIALS.iter().find(|(k, _)| *k == cand) {
            return Some(json!({ "reference_type": "minecraft_id", "value": id }));
        }
    }
    None
}

/// Strip a leading quantity + articles from a single-ingredient phrase (smelting takes exactly one
/// input) and resolve it. `None` if it doesn't resolve.
fn resolve_single_ingredient(side: &[String], existing: &[String]) -> Option<Value> {
    const ARTICLES: &[&str] = &["a", "an", "the", "some", "of", "each"];
    let mut it = side.iter().peekable();
    if it.peek().is_some_and(|w| parse_quantity(w).is_some()) {
        it.next();
    }
    let phrase: Vec<String> = it
        .filter(|w| !ARTICLES.contains(&w.as_str()))
        .cloned()
        .collect();
    resolve_ingredient_ref(&phrase, existing)
}

/// Split a crafting ingredient side ("2 sticks, 3 iron and a diamond") into resolved references,
/// quantity-expanded (a "3 iron" chunk yields three iron refs — a shapeless recipe carries one entry per
/// item). Operates on the RAW whitespace tokens (not the punctuation-stripped ones) so a comma separates
/// items just like "and"/"plus"/"with"/"&"/"+". Strips articles. `None` if any chunk names an
/// unresolvable material, nothing resolves, or the total exceeds a crafting grid's nine slots — all of
/// which defer to the model.
fn parse_crafting_ingredients(side: &[&str], existing: &[String]) -> Option<Vec<Value>> {
    const ARTICLES: &[&str] = &["a", "an", "the", "some", "of", "each"];
    let mut chunks: Vec<Vec<String>> = vec![Vec::new()];
    for tok in side {
        let bare = bare_word(tok);
        let is_connector =
            matches!(bare.as_str(), "and" | "plus" | "with") || *tok == "&" || *tok == "+";
        if is_connector {
            chunks.push(Vec::new());
            continue;
        }
        if !bare.is_empty() {
            chunks.last_mut().expect("seeded with one chunk").push(bare);
        }
        // A comma/semicolon glued to this token ("iron,") ends the current ingredient.
        if tok.ends_with(',') || tok.ends_with(';') {
            chunks.push(Vec::new());
        }
    }
    let mut out = Vec::new();
    for chunk in chunks {
        let mut it = chunk.iter().peekable();
        let mut count = 1u32;
        if let Some(n) = it.peek().and_then(|w| parse_quantity(w)) {
            count = n;
            it.next();
        }
        let phrase: Vec<String> = it
            .filter(|w| !ARTICLES.contains(&w.as_str()))
            .cloned()
            .collect();
        if phrase.is_empty() {
            continue; // a trailing/empty connector ("... and")
        }
        let r = resolve_ingredient_ref(&phrase, existing)?;
        for _ in 0..count {
            out.push(r.clone());
        }
    }
    (!out.is_empty() && out.len() <= 9).then_some(out)
}

/// A deterministic ADD/REMOVE RECIPE — the last common edit still routed to the 270M model. Handles the
/// reliably-parseable shapes and leaves genuinely spatial layouts to the model:
///
/// * **remove** — "remove the recipe for X" / "delete X's recipe" → `remove_recipe { target }`.
/// * **smelting/blasting** — "smelt Ruby Ore into a Ruby", "smelt it from copper ore" (refine),
///   "blast raw iron into an ingot" → `add_recipe` with a single `ingredient`.
/// * **crafting** — "craft a Ruby Blade from two rubies and a stick", "give it a recipe using 3 iron"
///   → a `shapeless` `add_recipe`. Crafting is emitted shapeless on purpose: a shapeless recipe crafts
///   regardless of arrangement, so the result is always obtainable — inferring a specific 2-D pattern
///   from prose is the spatial reasoning the model owns, not something to guess deterministically.
///
/// The result is resolved the same way as every other deterministic edit: an explicit `--refine` target,
/// otherwise an existing creation NAMED in the request — never on a guess. Every ingredient must resolve
/// to an existing creation or a known vanilla id. Returns `None` (→ model) for anything that isn't a
/// recipe, whose result/ingredients don't resolve, or that needs a layout we won't invent.
pub(crate) fn deterministic_recipe(
    request: &str,
    explicit_target: Option<&str>,
    existing: &[String],
) -> Option<Value> {
    let low = request.to_lowercase();
    if !looks_like_recipe(&low) {
        return None;
    }
    let words: Vec<String> = request
        .split_whitespace()
        .map(bare_word)
        .filter(|w| !w.is_empty())
        .collect();

    // --- remove_recipe: a recipe mentioned alongside a removal verb → remove it from the named result.
    if words.iter().any(|w| w == "recipe" || w == "recipes") {
        const REMOVE: &[&str] = &[
            "remove", "delete", "destroy", "discard", "scrap", "erase", "clear",
        ];
        let removing = words.iter().any(|w| REMOVE.contains(&w.as_str()))
            || low.contains("get rid of")
            || low.contains("take away");
        if removing {
            let target = resolve_named_target(explicit_target, &words, existing)?;
            return Some(json!({
                "operation": "remove_recipe",
                "payload": { "target": { "reference_type": "display_name", "value": target } }
            }));
        }
    }

    // A display-name reference to the recipe's result: the refine target, else an existing creation named
    // in `side`. Never a guess — returns None to fall through to the model.
    let result_ref = |side: &[String]| -> Option<Value> {
        let name = resolve_named_target(explicit_target, side, existing)?;
        Some(json!({ "reference_type": "display_name", "value": name }))
    };

    // Where does an ingredient clause begin? The first "from"/"using"/"out of"/"made of"/"made from".
    let prep_pos = words.iter().enumerate().position(|(i, w)| {
        w == "from"
            || w == "using"
            || (w == "of" && i > 0 && matches!(words[i - 1].as_str(), "out" | "made"))
    });

    // --- smelting / blasting: exactly one input, one result.
    let smelt_verb = words.iter().position(|w| {
        matches!(
            w.as_str(),
            "smelt" | "smelted" | "smelting" | "blast" | "blasted" | "blasting"
        )
    });
    if let Some(vpos) = smelt_verb {
        let recipe_type = if words[vpos].starts_with("blast") {
            "blasting"
        } else {
            "smelting"
        };
        let (ing_words, result): (Vec<String>, Value) =
            if let Some(ipos) = words.iter().position(|w| w == "into") {
                // "smelt <INPUT> into <RESULT>"
                (
                    words[vpos + 1..ipos].to_vec(),
                    result_ref(&words[ipos + 1..])?,
                )
            } else if let Some(pp) = prep_pos {
                // "smelt <RESULT> from <INPUT>" or, refine, "smelt from <INPUT>"
                (words[pp + 1..].to_vec(), result_ref(&words[..pp])?)
            } else {
                // "smelt <INPUT>" — only makes sense in refine mode (result = the refined creation)
                let t = explicit_target?;
                (
                    words[vpos + 1..].to_vec(),
                    json!({ "reference_type": "display_name", "value": t }),
                )
            };
        let ingredient = resolve_single_ingredient(&ing_words, existing)?;
        return Some(json!({
            "operation": "add_recipe",
            "payload": { "recipe": {
                "recipe_type": recipe_type,
                "ingredient": ingredient,
                "result": result,
                "count": 1
            }}
        }));
    }

    // --- crafting (shapeless): needs an ingredient clause to know what goes in. Split the RAW tokens on
    // the same preposition so commas in the ingredient list survive (they're stripped from `words`).
    let pp = prep_pos?;
    let raw: Vec<&str> = request.split_whitespace().collect();
    let raw_prep = raw.iter().enumerate().position(|(i, t)| {
        let b = bare_word(t);
        b == "from"
            || b == "using"
            || (b == "of" && i > 0 && matches!(bare_word(raw[i - 1]).as_str(), "out" | "made"))
    })?;
    let ingredients = parse_crafting_ingredients(&raw[raw_prep + 1..], existing)?;
    let result = result_ref(&words[..pp])?;
    Some(json!({
        "operation": "add_recipe",
        "payload": { "recipe": {
            "recipe_type": "shapeless",
            "ingredients": ingredients,
            "result": result,
            "count": 1
        }}
    }))
}

/// Turn a validated operations array into an [`IntentResult`], surfacing the two terminal
/// single-operation signals (`clarify_request`, `unsupported_request`) the CONTRACT defines.
pub(crate) fn to_result(operations: Vec<Value>) -> IntentResult {
    let op_name = |v: &Value| {
        v.get("operation")
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let needs_clarification = operations
        .iter()
        .any(|o| op_name(o).as_deref() == Some("clarify_request"));
    let unsupported_reason = operations
        .iter()
        .find(|o| op_name(o).as_deref() == Some("unsupported_request"))
        .and_then(|o| o.get("payload"))
        .and_then(|p| p.get("explanation").or_else(|| p.get("reason_code")))
        .and_then(Value::as_str)
        .map(str::to_owned);
    IntentResult {
        operations: Value::Array(operations),
        needs_clarification,
        unsupported_reason,
    }
}

/// Phase 0 stub, kept for callers that must not yet interpret intent.
#[derive(Debug, Default)]
pub struct UnimplementedIntentAdapter;

impl IntentAdapter for UnimplementedIntentAdapter {
    fn model_version(&self) -> &str {
        "none"
    }
    fn interpret(&self, _request: &str, _context: &Value) -> Result<IntentResult> {
        Err(AmcError::unimplemented(
            Component::IntentAdapter,
            "interpret (Phase 4)",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_reports_unimplemented_owned_by_intent_adapter() {
        let a = UnimplementedIntentAdapter;
        assert_eq!(a.model_version(), "none");
        let err = a.interpret("Create a ruby sword", &json!({})).unwrap_err();
        assert_eq!(err.owner(), Some(Component::IntentAdapter));
    }

    #[test]
    fn contract_is_the_trained_bytes() {
        // Pinned to the sha256 of the exact prompt the promoted model was fine-tuned with. If this
        // fails, contract/contract.txt drifted from training — retrain or restore, do not just bump.
        use sha2::{Digest, Sha256};
        let hex = format!("{:x}", Sha256::digest(CONTRACT.as_bytes()));
        assert_eq!(
            hex, "133e1c328a379d16df1e97cb2ce60b19949ccdec21b04beb4a2a5f1d9d9678ba",
            "CONTRACT bytes changed — model must be retrained against the new prompt"
        );
    }

    #[test]
    fn grammar_is_the_served_bytes() {
        // Pinned like the CONTRACT: every shipped model manifest records this hash, so changing the
        // grammar is a model-compatibility break, not a silent edit.
        assert_eq!(
            manifest::grammar_sha256(),
            "2ea1ce992b090b18c09c645d019cd29a5dc8a305653c738b6297f25ca8e7ff08",
            "grammar bytes changed — bump the model manifest's grammar pin deliberately"
        );
    }

    #[test]
    fn user_message_matches_training_format() {
        let empty = build_user_message("Make a ruby sword", &json!({}));
        assert_eq!(empty, "Request: Make a ruby sword");

        let ctx = json!({"concepts": [{"display_name": "Ruby Digger"}, "Ruby Blade"]});
        let with_ctx = build_user_message("Remove the Ruby Digger", &ctx);
        assert_eq!(
            with_ctx,
            "Current project already contains: Ruby Digger, Ruby Blade.\nRequest: Remove the Ruby Digger"
        );
    }

    #[test]
    fn extract_ops_takes_leading_array_and_ignores_trailing_garbage() {
        let raw = "[{\"operation\":\"create_concept\",\"payload\":{\"display_name\":\"Ruby Blade\"}}]<end_of_turn>\n<pad>model garbage {\"operation\":\"x\"}";
        let ops = extract_ops(raw).expect("should parse leading array");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0]["operation"], "create_concept");
    }

    #[test]
    fn resolve_numeric_properties_parses_strings_and_drops_relative_terms() {
        let mut ops = vec![json!({
            "operation": "create_concept",
            "payload": { "properties": {
                "attack_damage": "9",          // numeric string → number
                "enchantability": "high",      // relative term → dropped (use default)
                "durability": 1500,            // already a number → untouched
                "display_name": "Fang"         // non-numeric field → untouched
            }}
        })];
        resolve_numeric_properties(&mut ops);
        let p = ops[0].pointer("/payload/properties").unwrap();
        assert_eq!(p.get("attack_damage"), Some(&json!(9.0)));
        assert!(p.get("enchantability").is_none());
        assert_eq!(p.get("durability"), Some(&json!(1500)));
        assert_eq!(p.get("display_name"), Some(&json!("Fang")));
    }

    #[test]
    fn deterministic_update_targets_the_refined_concept() {
        // A numeric stat edit scoped to a known target → update_concept, never a create/duplicate.
        let none: &[String] = &[];
        // (a) EXPLICIT target (refine mode): a stat edit → update_concept, whatever the phrasing.
        for phrase in [
            "make it do 12 damage",
            "do 12 damage",
            "12 attack damage",
            "set the damage to 12",
            "a sword called Sting with 12 attack damage",
        ] {
            let op = deterministic_update(phrase, Some("Sting"), none)
                .unwrap_or_else(|| panic!("explicit-target phrasing failed: {phrase}"));
            assert_eq!(op.pointer("/operation"), Some(&json!("update_concept")));
            assert_eq!(
                op.pointer("/payload/target/reference_type"),
                Some(&json!("display_name"))
            );
            assert_eq!(op.pointer("/payload/target/value"), Some(&json!("Sting")));
            assert_eq!(
                op.pointer("/payload/changes/properties/attack_damage"),
                Some(&json!(12.0))
            );
        }
        // (b) NO explicit target: resolve the concept BY NAME from the request itself (any phrasing).
        let existing = vec!["Sting".to_string(), "Golden Apple".to_string()];
        for phrase in [
            "update Sting with a damage of 9",
            "give Sting 9 damage",
            "Sting to 9 attack",
        ] {
            let op = deterministic_update(phrase, None, &existing)
                .unwrap_or_else(|| panic!("by-name phrasing failed: {phrase}"));
            assert_eq!(op.pointer("/payload/target/value"), Some(&json!("Sting")));
            assert_eq!(
                op.pointer("/payload/changes/properties/attack_damage"),
                Some(&json!(9.0))
            );
        }
        // multi-word name wins as a whole phrase
        assert_eq!(
            deterministic_update("Golden Apple heals 10", None, &existing)
                .unwrap()
                .pointer("/payload/target/value"),
            Some(&json!("Golden Apple"))
        );
        // heals / durability / stack recognized too (explicit target)
        assert_eq!(
            deterministic_update("heals 6 now", Some("Apple"), none)
                .unwrap()
                .pointer("/payload/changes/properties/nutrition"),
            Some(&json!(6))
        );
        assert_eq!(
            deterministic_update("stack to 16", Some("Gem"), none)
                .unwrap()
                .pointer("/payload/changes/properties/max_stack_size"),
            Some(&json!(16))
        );
        // No stat → None (rename/prose go to the model). No named-and-existing concept → None.
        assert!(deterministic_update("rename it to Excalibur", Some("Sting"), none).is_none());
        assert!(deterministic_update("make it glow", Some("Sting"), none).is_none());
        assert!(deterministic_update("give it 9 damage", None, &existing).is_none()); // no name in text
                                                                                      // A substring of a name must NOT match a different word ("Stinger" is not "Sting").
        assert!(
            deterministic_update("a sword called Stinger with 9 damage", None, &existing).is_none()
                || deterministic_update("a sword called Stinger with 9 damage", None, &existing)
                    .unwrap()
                    .pointer("/payload/target/value")
                    != Some(&json!("Sting"))
        );
    }

    #[test]
    fn create_naming_an_existing_concept_becomes_an_update() {
        // The refine box pre-fills a create-shaped definition; editing a number must UPDATE the existing
        // creation, not attempt a duplicate — regardless of UI mode / the --refine flag.
        let existing = vec!["Digger".to_string(), "Sting".to_string()];
        let create = deterministic_create("a pickaxe called Digger with 1600 durability").unwrap();
        let up = create_collision_to_update(&create, &existing).expect("existing name → update");
        assert_eq!(up.pointer("/operation"), Some(&json!("update_concept")));
        assert_eq!(up.pointer("/payload/target/value"), Some(&json!("Digger")));
        assert_eq!(
            up.pointer("/payload/changes/properties/durability"),
            Some(&json!(1600))
        );
        // case-insensitive match
        let c2 = deterministic_create("a sword called sting with 9 attack damage").unwrap();
        assert_eq!(
            create_collision_to_update(&c2, &existing)
                .unwrap()
                .pointer("/payload/target/value"),
            Some(&json!("Sting"))
        );
        // a genuinely NEW name is left as a create
        let fresh =
            deterministic_create("a copper axe called Chopper with 400 durability").unwrap();
        assert!(create_collision_to_update(&fresh, &existing).is_none());
    }

    #[test]
    fn deterministic_create_handles_the_obvious_cases() {
        // a plain material+type → Title-Cased name, right type/subtype
        let op = deterministic_create("a copper pickaxe").unwrap();
        assert_eq!(op.pointer("/payload/concept_type"), Some(&json!("tool")));
        assert_eq!(op.pointer("/payload/subtype"), Some(&json!("pickaxe")));
        assert_eq!(
            op.pointer("/payload/display_name"),
            Some(&json!("Copper Pickaxe"))
        );
        // "called X" wins for the name; stats are parsed
        let op = deterministic_create("a redstone sword called Zap with 9 attack damage").unwrap();
        assert_eq!(op.pointer("/payload/subtype"), Some(&json!("sword")));
        assert_eq!(op.pointer("/payload/display_name"), Some(&json!("Zap")));
        assert_eq!(
            op.pointer("/payload/properties/attack_damage"),
            Some(&json!(9.0))
        );
        // blocks have no subtype
        let op = deterministic_create("a glowing amethyst block").unwrap();
        assert_eq!(op.pointer("/payload/concept_type"), Some(&json!("block")));
        assert_eq!(
            op.pointer("/payload/display_name"),
            Some(&json!("Glowing Amethyst Block"))
        );
        // food: concept_type food, no subtype, "heals N" → nutrition
        let op = deterministic_create("a golden apple that heals 8").unwrap();
        assert_eq!(op.pointer("/payload/concept_type"), Some(&json!("food")));
        assert!(op.pointer("/payload/subtype").is_none());
        assert_eq!(
            op.pointer("/payload/display_name"),
            Some(&json!("Golden Apple"))
        );
        assert_eq!(op.pointer("/payload/properties/nutrition"), Some(&json!(8)));
        // an unsupported type must NOT be rescued (stays None → model's unsupported stands)
        assert!(deterministic_create("a magic wand").is_none());
        assert!(deterministic_create("a bronze helmet").is_none());
        // a modification is never a fresh create
        assert!(deterministic_create("make the sword stronger").is_none());
        // synonyms/near-types map to the closest buildable kind
        assert_eq!(
            deterministic_create("a stone hammer")
                .unwrap()
                .pointer("/payload/subtype"),
            Some(&json!("sword"))
        );
        assert_eq!(
            deterministic_create("a rusty hatchet")
                .unwrap()
                .pointer("/payload/subtype"),
            Some(&json!("axe"))
        );
    }

    #[test]
    fn deterministic_unsupported_flags_and_defers_correctly() {
        // clearly-unsupported kinds → a clean unsupported op
        let op = deterministic_unsupported("a golden bow").unwrap();
        assert_eq!(
            op.pointer("/operation"),
            Some(&json!("unsupported_request"))
        );
        assert!(deterministic_unsupported("a bronze helmet").is_some());
        assert!(deterministic_unsupported("a fire dragon").is_some());
        // a plain makeable request is NOT flagged (create wins; unsupported only runs after create declines)
        assert!(deterministic_unsupported("a copper pickaxe").is_none());
        // supported type present → create wins over the unsupported word
        assert!(deterministic_create("a sword that shoots arrows").is_some());
    }

    #[test]
    fn normalize_stack_size_forces_weapon_tool_and_clamps_others() {
        let mut ops = vec![
            json!({"operation":"create_concept","payload":{"concept_type":"tool","properties":{"max_stack_size":64}}}),
            json!({"operation":"create_concept","payload":{"concept_type":"item","properties":{"max_stack_size":999}}}),
            json!({"operation":"create_concept","payload":{"concept_type":"food","properties":{"max_stack_size":16}}}),
        ];
        normalize_stack_size(&mut ops);
        assert!(ops[0]
            .pointer("/payload/properties/max_stack_size")
            .is_none()); // tool → dropped (→ default 1)
        assert_eq!(
            ops[1].pointer("/payload/properties/max_stack_size"),
            Some(&json!(64))
        ); // clamped 999→64
        assert_eq!(
            ops[2].pointer("/payload/properties/max_stack_size"),
            Some(&json!(16))
        ); // in range → kept
    }

    #[test]
    fn extract_ops_accepts_empty_array_and_rejects_non_ops() {
        assert_eq!(extract_ops("[]").unwrap().len(), 0);
        assert!(extract_ops("no array here").is_none());
        assert!(extract_ops("[1, 2, 3]").is_none()); // not operation objects
    }

    #[test]
    fn deterministic_rename_targets_and_extracts_the_new_name() {
        let none: &[String] = &[];
        // (a) EXPLICIT target (refine mode): every common phrasing → an update to display_name.
        for phrase in [
            "rename it to Zephyr",
            "rename to Zephyr",
            "call it Zephyr",
            "name it Zephyr",
            "change its name to Zephyr",
            "set the name to Zephyr",
            "rename it Zephyr",
        ] {
            let op = deterministic_rename(phrase, Some("Old Blade"), none)
                .unwrap_or_else(|| panic!("explicit-target rename failed: {phrase}"));
            assert_eq!(op.pointer("/operation"), Some(&json!("update_concept")));
            assert_eq!(
                op.pointer("/payload/target/value"),
                Some(&json!("Old Blade"))
            );
            assert_eq!(
                op.pointer("/payload/changes/display_name"),
                Some(&json!("Zephyr")),
                "phrase: {phrase}"
            );
        }
        // multi-word new name is preserved and Title-Cased
        assert_eq!(
            deterministic_rename("rename it to golden fang", Some("X"), none)
                .unwrap()
                .pointer("/payload/changes/display_name"),
            Some(&json!("Golden Fang"))
        );
        // (b) NO explicit target: resolve the OLD name from the request, rename to the new one.
        let existing = vec!["Sting".to_string(), "Golden Apple".to_string()];
        let op = deterministic_rename("rename Sting to Needle", None, &existing).unwrap();
        assert_eq!(op.pointer("/payload/target/value"), Some(&json!("Sting")));
        assert_eq!(
            op.pointer("/payload/changes/display_name"),
            Some(&json!("Needle"))
        );
        // multi-word existing name wins as a whole phrase
        assert_eq!(
            deterministic_rename("rename the Golden Apple to Sunfruit", None, &existing)
                .unwrap()
                .pointer("/payload/target/value"),
            Some(&json!("Golden Apple"))
        );
        // a type word in the NEW name must not be mistaken for a create
        assert_eq!(
            deterministic_rename("rename Sting to Golden Sword", None, &existing)
                .unwrap()
                .pointer("/payload/changes/display_name"),
            Some(&json!("Golden Sword"))
        );
        // No new name → None (falls to model). Renaming to the current name → None (no-op).
        assert!(deterministic_rename("rename it", Some("Sting"), none).is_none());
        assert!(deterministic_rename("rename it to Sting", Some("Sting"), none).is_none());
        // Not a rename at all → None.
        assert!(deterministic_rename("make it stronger", Some("Sting"), none).is_none());
        assert!(deterministic_rename("give it 9 damage", Some("Sting"), none).is_none());
        // No target resolvable without refine → None (never rename on a guess).
        assert!(deterministic_rename("rename it to Needle", None, &existing).is_none());
        // A recipe edit is never a rename.
        assert!(
            deterministic_rename("rename the recipe to shapeless", Some("Sting"), none).is_none()
        );
    }

    #[test]
    fn deterministic_delete_requires_a_verb_and_a_resolved_target() {
        let none: &[String] = &[];
        // Explicit target (refine mode): any delete verb/phrase → delete_concept.
        for phrase in [
            "delete it",
            "remove it",
            "destroy this",
            "get rid of it",
            "erase it",
            "scrap it",
        ] {
            let op = deterministic_delete(phrase, Some("Doomed"), none)
                .unwrap_or_else(|| panic!("explicit-target delete failed: {phrase}"));
            assert_eq!(op.pointer("/operation"), Some(&json!("delete_concept")));
            assert_eq!(op.pointer("/payload/target/value"), Some(&json!("Doomed")));
            assert_eq!(
                op.pointer("/payload/target/reference_type"),
                Some(&json!("display_name"))
            );
        }
        // By-name resolution with no refine.
        let existing = vec!["Sting".to_string(), "Golden Apple".to_string()];
        assert_eq!(
            deterministic_delete("delete Sting", None, &existing)
                .unwrap()
                .pointer("/payload/target/value"),
            Some(&json!("Sting"))
        );
        assert_eq!(
            deterministic_delete("remove the Golden Apple please", None, &existing)
                .unwrap()
                .pointer("/payload/target/value"),
            Some(&json!("Golden Apple"))
        );
        // No delete verb → None. A recipe removal is not a concept delete → None.
        assert!(deterministic_delete("make it stronger", None, &existing).is_none());
        assert!(deterministic_delete("remove the recipe for Sting", None, &existing).is_none());
        // Delete verb but no known creation named → None (never delete on a guess).
        assert!(deterministic_delete("delete the sword", None, &existing).is_none());
    }

    #[test]
    fn deterministic_recipe_builds_smelting_crafting_and_removal() {
        let existing = vec![
            "Ruby".to_string(),
            "Ruby Ore".to_string(),
            "Ruby Blade".to_string(),
        ];
        let none: &[String] = &[];

        // --- SMELTING: input + result, both directions and refine.
        let op = deterministic_recipe("Smelt Ruby Ore into a Ruby", None, &existing).unwrap();
        assert_eq!(op.pointer("/operation"), Some(&json!("add_recipe")));
        assert_eq!(
            op.pointer("/payload/recipe/recipe_type"),
            Some(&json!("smelting"))
        );
        assert_eq!(
            op.pointer("/payload/recipe/ingredient/value"),
            Some(&json!("Ruby Ore"))
        );
        assert_eq!(
            op.pointer("/payload/recipe/result/value"),
            Some(&json!("Ruby"))
        );
        // refine: "smelt it from copper ore" → result = target, ingredient = vanilla copper ore
        let op = deterministic_recipe("smelt it from copper ore", Some("Ruby"), &existing).unwrap();
        assert_eq!(
            op.pointer("/payload/recipe/ingredient/reference_type"),
            Some(&json!("minecraft_id"))
        );
        assert_eq!(
            op.pointer("/payload/recipe/ingredient/value"),
            Some(&json!("minecraft:copper_ore"))
        );
        assert_eq!(
            op.pointer("/payload/recipe/result/value"),
            Some(&json!("Ruby"))
        );
        // blasting is recognized as its own recipe type
        assert_eq!(
            deterministic_recipe("blast raw iron into a Ruby", None, &existing)
                .unwrap()
                .pointer("/payload/recipe/recipe_type"),
            Some(&json!("blasting"))
        );

        // --- CRAFTING → shapeless, ingredients resolved and quantity-expanded.
        for phrase in [
            "craft a Ruby Blade from two rubies and a stick",
            "make a Ruby Blade using 2 rubies and a stick",
            "a Ruby Blade is made from two rubies and a stick",
        ] {
            let op = deterministic_recipe(phrase, None, &existing)
                .unwrap_or_else(|| panic!("crafting phrasing failed: {phrase}"));
            assert_eq!(
                op.pointer("/payload/recipe/recipe_type"),
                Some(&json!("shapeless")),
                "{phrase}"
            );
            assert_eq!(
                op.pointer("/payload/recipe/result/value"),
                Some(&json!("Ruby Blade")),
                "{phrase}"
            );
            let ings = op
                .pointer("/payload/recipe/ingredients")
                .and_then(Value::as_array)
                .unwrap();
            // two Ruby (display_name) + one stick (minecraft_id)
            assert_eq!(ings.len(), 3, "{phrase}");
            assert_eq!(
                ings.iter().filter(|r| r["value"] == json!("Ruby")).count(),
                2,
                "{phrase}"
            );
            assert!(
                ings.iter().any(|r| r["value"] == json!("minecraft:stick")),
                "{phrase}"
            );
        }
        // refine: "give it a recipe from 3 iron and a diamond" → result = target
        let op = deterministic_recipe(
            "give it a recipe from 3 iron and a diamond",
            Some("Ruby Blade"),
            &existing,
        )
        .unwrap();
        assert_eq!(
            op.pointer("/payload/recipe/result/value"),
            Some(&json!("Ruby Blade"))
        );
        assert_eq!(
            op.pointer("/payload/recipe/ingredients")
                .and_then(Value::as_array)
                .unwrap()
                .len(),
            4 // 3 iron + 1 diamond
        );

        // --- REMOVE recipe: a removal verb + "recipe" → remove_recipe on the named result.
        let op =
            deterministic_recipe("remove the recipe for the Ruby Blade", None, &existing).unwrap();
        assert_eq!(op.pointer("/operation"), Some(&json!("remove_recipe")));
        assert_eq!(
            op.pointer("/payload/target/value"),
            Some(&json!("Ruby Blade"))
        );
        assert_eq!(
            deterministic_recipe("delete its recipe", Some("Ruby"), &existing)
                .unwrap()
                .pointer("/payload/target/value"),
            Some(&json!("Ruby"))
        );

        // --- DECLINES (→ model) — never guess.
        // Not a recipe at all.
        assert!(deterministic_recipe("make it stronger", Some("Ruby"), &existing).is_none());
        // A plain create must NOT be mistaken for a recipe (no ingredient clause / smelt / recipe word).
        assert!(deterministic_recipe("craft a copper pickaxe", None, none).is_none());
        // Unknown ingredient → decline the whole recipe rather than invent an id.
        assert!(
            deterministic_recipe("craft a Ruby Blade from unobtanium", None, &existing).is_none()
        );
        // Result not an existing creation and no refine target → decline.
        assert!(deterministic_recipe("craft a Widget from two rubies", None, &existing).is_none());
        // A crafting request naming its result must NOT be rescued as a create (regression guard).
        assert!(deterministic_create("craft a Ruby Blade from two rubies").is_none());
    }

    #[test]
    fn to_result_flags_clarify_and_unsupported() {
        let clarify = to_result(vec![json!({"operation":"clarify_request","payload":{}})]);
        assert!(clarify.needs_clarification);
        assert!(clarify.unsupported_reason.is_none());

        let unsupported = to_result(vec![
            json!({"operation":"unsupported_request","payload":{"explanation":"custom dimensions"}}),
        ]);
        assert!(!unsupported.needs_clarification);
        assert_eq!(
            unsupported.unsupported_reason.as_deref(),
            Some("custom dimensions")
        );

        let create = to_result(vec![json!({"operation":"create_concept","payload":{}})]);
        assert!(!create.needs_clarification && create.unsupported_reason.is_none());
    }
}
