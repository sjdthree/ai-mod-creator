//! Randomized property tests for the deterministic intent fast-path (create / update / rename /
//! delete). These prove the invariant the recurring "forever bug" violated — *an edit UPDATES the named
//! creation and never duplicates it* — over inputs generated at runtime (random names, types, values),
//! NOT the reported example values. Per project policy, a fix is only trusted once it holds on inputs
//! the author and model have never seen; baking "Sting"/"1600 durability" into a test only confirms the
//! one case and hides brittleness. The PRNG is seeded with a fixed constant so CI is reproducible while
//! still exercising a wide value space every run.

use super::*;

/// A tiny seeded xorshift64 — no external `rand` dependency, and reproducible in CI (the harness forbids
/// nondeterministic `Date::now`/`random` seeding anyway; a fixed seed with a wide draw space is exactly
/// the "generated, not hand-picked" property we want).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    /// Inclusive integer in `[lo, hi]`.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len() as u64) as usize]
    }
    /// A fresh Capitalized nonsense name (e.g. "Xkjdqa") — vanishingly unlikely to collide with a type
    /// word, a stat word, or another generated name, so it exercises pure by-name targeting.
    fn name(&mut self) -> String {
        let mut s = String::new();
        s.push((b'A' + self.below(26) as u8) as char);
        for _ in 0..self.range(3, 8) {
            s.push((b'a' + self.below(26) as u8) as char);
        }
        s
    }
}

/// Mirror `HttpIntentAdapter::interpret`'s deterministic fast-path ordering exactly (refine + non-refine),
/// returning the single op it would emit without ever calling the model. Any drift between this and
/// `http.rs` would make the test lie, so keep them in step.
fn interpret_det(request: &str, refine: Option<&str>, existing: &[String]) -> Option<Value> {
    if let Some(t) = refine {
        if let Some(op) = deterministic_update(request, Some(t), existing) {
            return Some(op);
        }
        if let Some(op) = deterministic_rename(request, Some(t), existing) {
            return Some(op);
        }
        if let Some(op) = deterministic_delete(request, Some(t), existing) {
            return Some(op);
        }
        if let Some(op) = deterministic_recipe(request, Some(t), existing) {
            return Some(op);
        }
        return None;
    }
    if let Some(op) = deterministic_recipe(request, None, existing) {
        return Some(op);
    }
    if let Some(op) = deterministic_create(request) {
        let mut ops = vec![op];
        resolve_numeric_properties(&mut ops);
        normalize_stack_size(&mut ops);
        if let Some(update) = create_collision_to_update(&ops[0], existing) {
            return Some(update);
        }
        return Some(ops.remove(0));
    }
    if let Some(op) = deterministic_rename(request, None, existing) {
        return Some(op);
    }
    if let Some(op) = deterministic_delete(request, None, existing) {
        return Some(op);
    }
    if let Some(op) = deterministic_update(request, None, existing) {
        return Some(op);
    }
    None
}

fn op_name(op: &Value) -> Option<&str> {
    op.get("operation").and_then(Value::as_str)
}

/// A vanilla material paired with the `minecraft:` id the deterministic parser must resolve it to. Drawn
/// at runtime so the recipe test proves resolution over a value space it never hard-codes a single case
/// of.
const VANILLA: &[(&str, &str)] = &[
    ("stick", "minecraft:stick"),
    ("iron", "minecraft:iron_ingot"),
    ("gold", "minecraft:gold_ingot"),
    ("diamond", "minecraft:diamond"),
    ("emerald", "minecraft:emerald"),
    ("redstone", "minecraft:redstone"),
    ("coal", "minecraft:coal"),
    ("obsidian", "minecraft:obsidian"),
    ("leather", "minecraft:leather"),
    ("string", "minecraft:string"),
];

const NUM_WORDS: &[&str] = &[
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
];

#[test]
fn randomized_recipe_invariants() {
    let mut rng = Rng(0x1234_5678_9ABC_DEF0);
    const N: usize = 300;

    for i in 0..N {
        // A fresh result creation, plus a couple of unrelated creations so targeting must pick the RIGHT
        // one. Some ingredients are existing creations (referenced by display name), the rest vanilla.
        let result = rng.name();
        let ore = format!("{result} Ore"); // an existing creation used as a smelting input
        let mut existing = vec![result.clone(), ore.clone()];
        for _ in 0..rng.below(2) {
            existing.push(rng.name());
        }

        // ---- CRAFTING (shapeless): 1–3 ingredients, each vanilla or the existing creation, each with a
        // runtime-random quantity 1..=3 (kept small so totals stay within the 9-slot grid).
        let n_ing = rng.range(1, 3) as usize;
        let mut phrases = Vec::new();
        let mut expected: Vec<(String, u32)> = Vec::new(); // (canonical value, count)
        for _ in 0..n_ing {
            let qty = rng.range(1, 3) as u32;
            let qword = if rng.below(2) == 0 {
                qty.to_string()
            } else {
                NUM_WORDS[(qty - 1) as usize].to_string()
            };
            // Choose an ingredient: existing creation (by display name) or a vanilla material.
            let (word, canon) = if rng.below(3) == 0 {
                (ore.clone(), ore.clone()) // reference an existing creation
            } else {
                let (w, id) = *rng.pick(VANILLA);
                // Occasionally pluralize a vanilla noun to exercise singularization.
                let w = if qty > 1 && rng.below(2) == 0 {
                    format!("{w}s")
                } else {
                    w.to_string()
                };
                (w, id.to_string())
            };
            phrases.push(format!("{qword} {word}"));
            // Accumulate expected canonical value counts.
            if let Some(e) = expected.iter_mut().find(|(v, _)| *v == canon) {
                e.1 += qty;
            } else {
                expected.push((canon, qty));
            }
        }
        // Only assert exact ingredient multiset when the total fits a crafting grid (the parser declines
        // otherwise, which is a separate, intentional path).
        let total: u32 = expected.iter().map(|(_, c)| c).sum();

        let joiner = if rng.below(2) == 0 { " and " } else { ", " };
        let ing_clause = phrases.join(joiner);
        let verb = *rng.pick(&["craft", "make"]);
        let prep = *rng.pick(&["from", "using"]);
        let craft_req = format!("{verb} a {result} {prep} {ing_clause}");

        let op = interpret_det(&craft_req, None, &existing);
        if total <= 9 {
            let op = op.unwrap_or_else(|| panic!("[{i}] crafting produced nothing: {craft_req:?}"));
            assert_eq!(op_name(&op), Some("add_recipe"), "[{i}] {craft_req:?}");
            assert_eq!(
                op.pointer("/payload/recipe/recipe_type"),
                Some(&json!("shapeless")),
                "[{i}] {craft_req:?}"
            );
            assert_eq!(
                op.pointer("/payload/recipe/result/value"),
                Some(&json!(result)),
                "[{i}] craft result: {craft_req:?}"
            );
            // The emitted ingredient multiset must match the requested one, canonicalized.
            let ings = op
                .pointer("/payload/recipe/ingredients")
                .and_then(Value::as_array)
                .unwrap_or_else(|| panic!("[{i}] no ingredients: {craft_req:?}"));
            let mut counts: std::collections::BTreeMap<String, u32> = Default::default();
            for r in ings {
                *counts
                    .entry(r["value"].as_str().unwrap().to_string())
                    .or_default() += 1;
            }
            for (v, c) in &expected {
                assert_eq!(
                    counts.get(v).copied(),
                    Some(*c),
                    "[{i}] ingredient {v} count in {craft_req:?} (got {counts:?})"
                );
            }
            assert_eq!(
                counts.values().sum::<u32>(),
                total,
                "[{i}] total ingredients: {craft_req:?}"
            );
        }

        // ---- SMELTING: "smelt <Result> Ore into a <Result>" → the ore creation as input, result out.
        let smelt_req = format!("smelt {ore} into a {result}");
        let op = interpret_det(&smelt_req, None, &existing)
            .unwrap_or_else(|| panic!("[{i}] smelting produced nothing: {smelt_req:?}"));
        assert_eq!(op_name(&op), Some("add_recipe"), "[{i}] {smelt_req:?}");
        assert_eq!(
            op.pointer("/payload/recipe/recipe_type"),
            Some(&json!("smelting")),
            "[{i}] {smelt_req:?}"
        );
        assert_eq!(
            op.pointer("/payload/recipe/ingredient/value"),
            Some(&json!(ore)),
            "[{i}] smelt input: {smelt_req:?}"
        );
        assert_eq!(
            op.pointer("/payload/recipe/result/value"),
            Some(&json!(result)),
            "[{i}] smelt result: {smelt_req:?}"
        );

        // ---- REMOVE: "remove the recipe for <Result>" → remove_recipe on the right target.
        let rm_req = format!("remove the recipe for {result}");
        let op = interpret_det(&rm_req, None, &existing)
            .unwrap_or_else(|| panic!("[{i}] remove produced nothing: {rm_req:?}"));
        assert_eq!(op_name(&op), Some("remove_recipe"), "[{i}] {rm_req:?}");
        assert_eq!(
            op.pointer("/payload/target/value"),
            Some(&json!(result)),
            "[{i}] remove target: {rm_req:?}"
        );
    }
}

#[test]
fn randomized_create_update_rename_delete_invariants() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    const N: usize = 400;
    let mats = [
        "ruby", "copper", "diamond", "emerald", "obsidian", "bronze", "gold", "iron", "crystal",
        "cobalt", "mithril",
    ];

    for i in 0..N {
        // Pick a category and its numeric stat, with runtime-random values in a category-appropriate range.
        let (subs, key, stat): (&[&str], &str, Option<&str>) = match rng.below(3) {
            0 => (
                &[
                    "sword", "dagger", "mace", "blade", "cutlass", "rapier", "katana", "spear",
                ],
                "attack_damage",
                Some("attack damage"),
            ),
            1 => (
                &["pickaxe", "axe", "shovel", "hoe", "drill", "hatchet"],
                "durability",
                Some("durability"),
            ),
            _ => (
                &[
                    "apple", "bread", "cookie", "steak", "pie", "muffin", "carrot",
                ],
                "nutrition",
                None,
            ),
        };
        let (lo, hi) = match key {
            "attack_damage" => (1, 50),
            "durability" => (100, 2000),
            _ => (1, 20),
        };
        let sub = *rng.pick(subs);
        let mat = *rng.pick(&mats);
        let name = rng.name();
        // Three distinct values, each generated this run.
        let v1 = rng.range(lo, hi);
        let bump = |rng: &mut Rng, avoid: &[i64]| loop {
            let v = rng.range(lo, hi);
            if !avoid.contains(&v) {
                return v;
            }
        };
        let v2 = bump(&mut rng, &[v1]);
        let v3 = bump(&mut rng, &[v1, v2]);
        let newname = loop {
            let n = rng.name();
            if n != name {
                break n;
            }
        };

        // A few unrelated pre-existing creations, so targeting must resolve the RIGHT one among several.
        let others: Vec<String> = (0..rng.below(3)).map(|_| rng.name()).collect();
        let existing_before: Vec<String> = others.clone();
        let mut existing_after = others.clone();
        existing_after.push(name.clone());

        // The stored value's JSON shape: attack_damage is a float, durability/nutrition are integers
        // (matching the deterministic parsers). Assert against that so we compare like-for-like.
        let expect = |v: i64| -> Value {
            if key == "attack_damage" {
                json!(v as f64)
            } else {
                json!(v)
            }
        };

        // Phrase builders that differ for food (no "attack damage"/"durability" keyword — it "heals").
        let (create_req, editform_req, update_req) = if let Some(stat) = stat {
            (
                format!("a {mat} {sub} called {name} with {v1} {stat}"),
                format!("a {sub} called {name} with {v2} {stat}"),
                format!("update {name} {stat} to {v3}"),
            )
        } else {
            (
                format!("a {mat} {sub} called {name} that heals {v1}"),
                format!("a {sub} called {name} that heals {v2}"),
                format!("update {name} to heal {v3}"),
            )
        };

        // 1. CREATE — a genuinely new name → a create carrying the right name and value.
        let op = interpret_det(&create_req, None, &existing_before)
            .unwrap_or_else(|| panic!("[{i}] create produced nothing: {create_req:?}"));
        assert_eq!(op_name(&op), Some("create_concept"), "[{i}] {create_req:?}");
        assert_eq!(
            op.pointer("/payload/display_name"),
            Some(&json!(name)),
            "[{i}] create name: {create_req:?}"
        );
        assert_eq!(
            op.pointer(&format!("/payload/properties/{key}")),
            Some(&expect(v1)),
            "[{i}] create value: {create_req:?}"
        );

        // 2. EDIT-FORM (a create-shaped definition for a name that now EXISTS) must become an UPDATE, never
        //    a duplicate — this is the exact shape the "forever bug" mangled.
        let op = interpret_det(&editform_req, None, &existing_after)
            .unwrap_or_else(|| panic!("[{i}] edit-form produced nothing: {editform_req:?}"));
        assert_eq!(
            op_name(&op),
            Some("update_concept"),
            "[{i}] edit-form must UPDATE, not duplicate: {editform_req:?}"
        );
        assert_eq!(
            op.pointer("/payload/target/value"),
            Some(&json!(name)),
            "[{i}] edit-form target: {editform_req:?}"
        );
        assert_eq!(
            op.pointer(&format!("/payload/changes/properties/{key}")),
            Some(&expect(v2)),
            "[{i}] edit-form value: {editform_req:?}"
        );

        // 3. NATURAL UPDATE naming the creation in prose → an update to the right target and value.
        let op = interpret_det(&update_req, None, &existing_after)
            .unwrap_or_else(|| panic!("[{i}] natural update produced nothing: {update_req:?}"));
        assert_eq!(op_name(&op), Some("update_concept"), "[{i}] {update_req:?}");
        assert_eq!(
            op.pointer("/payload/target/value"),
            Some(&json!(name)),
            "[{i}] update target: {update_req:?}"
        );
        assert_eq!(
            op.pointer(&format!("/payload/changes/properties/{key}")),
            Some(&expect(v3)),
            "[{i}] update value: {update_req:?}"
        );

        // 4. RENAME (both by-name and refine-scoped) → an update that changes display_name to the new one,
        //    targeting the existing creation and never producing a create.
        for (req, refine) in [
            (format!("rename {name} to {newname}"), None),
            (format!("call it {newname}"), Some(name.as_str())),
        ] {
            let op = interpret_det(&req, refine, &existing_after)
                .unwrap_or_else(|| panic!("[{i}] rename produced nothing: {req:?}"));
            assert_eq!(
                op_name(&op),
                Some("update_concept"),
                "[{i}] rename op: {req:?}"
            );
            assert_eq!(
                op.pointer("/payload/target/value"),
                Some(&json!(name)),
                "[{i}] rename target: {req:?}"
            );
            assert_eq!(
                op.pointer("/payload/changes/display_name"),
                Some(&json!(newname)),
                "[{i}] rename new name: {req:?}"
            );
        }

        // 5. DELETE (by-name and refine-scoped) → a delete_concept targeting exactly this creation.
        for (req, refine) in [
            (format!("delete {name}"), None),
            (String::from("delete it"), Some(name.as_str())),
        ] {
            let op = interpret_det(&req, refine, &existing_after)
                .unwrap_or_else(|| panic!("[{i}] delete produced nothing: {req:?}"));
            assert_eq!(
                op_name(&op),
                Some("delete_concept"),
                "[{i}] delete op: {req:?}"
            );
            assert_eq!(
                op.pointer("/payload/target/value"),
                Some(&json!(name)),
                "[{i}] delete target: {req:?}"
            );
        }
    }
}

/// Title-Case each word and join with spaces, mirroring the namer in `deterministic_create`.
fn title_words(ws: &[&str]) -> String {
    ws.iter()
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

/// A create request that opens with an imperative verb and/or filler ("make a", "please build me a",
/// "generate the") must name the concept after the DESCRIPTOR phrase, never the verb — else a later
/// by-name recipe/edit silently fails to resolve (the exact cascade the live end-to-end hit). Proves
/// the lead-in strip over randomly built prefixes and material/type pairs, hard-coding no single phrase.
#[test]
fn randomized_leadin_verbs_are_stripped_from_the_name() {
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    const N: usize = 400;
    let mats = [
        "ruby", "copper", "diamond", "emerald", "obsidian", "bronze", "gold", "iron", "crystal",
        "cobalt", "mithril",
    ];
    let types = [
        "sword", "dagger", "blade", "katana", "pickaxe", "axe", "shovel", "hoe",
    ];
    // Lead-in fragments: runs of verb/pronoun/politeness words + an article, each of which must be
    // stripped in full. Includes multi-word forms and the bare article as a baseline.
    let leadins = [
        "make a",
        "create a",
        "build a",
        "please make a",
        "give me a",
        "i want a",
        "can you make a",
        "generate the",
        "make me an",
        "a",
        "the",
    ];
    for i in 0..N {
        let mat = *rng.pick(&mats);
        let ty = *rng.pick(&types);
        let lead = *rng.pick(&leadins);
        let req = format!("{lead} {mat} {ty}");
        let expected = title_words(&[mat, ty]); // e.g. "Cobalt Sword" — verb + article gone.
        let op = interpret_det(&req, None, &[])
            .unwrap_or_else(|| panic!("[{i}] create produced nothing: {req:?}"));
        assert_eq!(op_name(&op), Some("create_concept"), "[{i}] {req:?}");
        assert_eq!(
            op.pointer("/payload/display_name"),
            Some(&json!(expected)),
            "[{i}] lead-in not stripped: {req:?}"
        );
    }
}
