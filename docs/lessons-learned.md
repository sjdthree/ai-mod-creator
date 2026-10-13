# Lessons Learned

A running log of non-obvious engineering lessons from building AMC. Newest first. These
complement the ADRs (`docs/adr/`), which record *decisions*; this file records *lessons*.

## GameTest verification works; the friction is datapack folder names, not discovery (Phase 3)

ADR-0004 feared NeoForge GameTest discovery was broken in production. It isn't — `@GameTestHolder`
+ `neoforge.enabledGameTestNamespaces` discovered and ran the generated tests fine (NeoForge
21.1.235). The real blocker was that **Minecraft 1.21 singularized datapack folders**: GameTest
structure templates load from `data/<ns>/structure/` (singular), not `structures/` — same change
as `recipes`→`recipe`. A hand-built empty 3×3×3 structure NBT is enough; the heavier
`net.neoforged.testframework` (`@EmptyTemplate`/`@TestHolder`/`DynamicTest`) is not needed for
plain registry assertions. `runGameTestServer` exit code 0 = all required passed — the clean CI
signal. Proven end-to-end in the container (ADR-0006).

## Pin generator versions from the official MDK, and prove it with a real build (Phase 3)

The generated project's `build.gradle` versions are load-bearing: guessing them (we had ModDevGradle
`2.0.28` / NeoForge `21.1.100`) would fail dependency resolution. The reliable source is the
official `NeoForgeMDKs/MDK-1.21.1-ModDevGradle` template — fetch its `build.gradle` +
`gradle.properties` and copy the exact strings (ModDevGradle `2.0.141`, NeoForge `21.1.235`,
`loaderVersion=[1,)`). Then a real `gradle build` in the container confirmed our generated
`SwordItem`/`SimpleTier`/`DeferredRegister` code compiles and packages a valid JAR (ADR-0005). Keep
the generator's pins in lockstep with the MDK as it updates.

## Determinism: exclude generated stable IDs from the state hash (Phase 1)

`candidate_state_hash` must satisfy "same parent state + same normalized operations → same hash"
(`SPEC-005 §22`, `SPEC-004 MCL-004`). Our stable ids (`concept_id`, `appearance_id`) are ULIDs —
time- and randomness-derived — so including them broke determinism across runs. Fix:
`CanonicalState::state_hash` hashes a **view** keyed by the deterministic `namespace_id`, with the
ULIDs, selector bookkeeping (`last_created` etc.), and timestamps stripped. Lesson: the hash
represents *generated-implementation* identity, not internal bookkeeping — hash the former only.

## Validator vs. state manager: split the 8-stage sequence by ownership (Phase 1)

`SPEC-004 §33` lists eight validation stages. We split them: `amc-validator` owns stages 1–6
(structural → operation schema → reference resolution → property constraints → semantic
constraints → capability) and emits **normalized** operations; `amc-state` enforces stages 7–8
(candidate-state schema + referential integrity) while *constructing* the candidate. This avoids
two components both trying to own referential integrity, and keeps the validator side-effect-free.

## Never guess: relative terms are rejected, not interpreted (Phase 1)

A relative word left in a numeric field (`durability: "stronger"`) is a hard error
(`UNRESOLVED_VALUE`, `SPEC-004 §36`) — the *intent adapter* is responsible for resolving relatives
to numbers. The deterministic core never invents a value. Similarly, ambiguous references become
clarifications, not a silent pick. This is how the pipeline upholds the 0%-false-success promise.

## NeoForge 1.21.1 API notes (Phase 2)

Verified from the NeoForged docs, for the generator templates:

- Items: `DeferredRegister.Items ITEMS = DeferredRegister.createItems(modid)`; basic item via
  `ITEMS.registerSimpleItem("name", new Item.Properties())`.
- Main class: `@Mod(MOD_ID)` with constructor `(IEventBus modEventBus)` calling
  `ITEMS.register(modEventBus)`.
- Sword: `new SimpleTier(BlockTags.INCORRECT_FOR_STONE_TOOL, durability, miningSpeed,
  attackBonus, enchantability, () -> Ingredient)` + `new SwordItem(tier, new
  Item.Properties().attributes(SwordItem.createAttributes(tier, attackDamage - 1, attackSpeed)))`.
  `createAttributes` adds the player's base 1, so pass `attackDamage - 1`.
- Recipes: `data/<modid>/recipe/<name>.json` (folder is singular in 1.21); shaped uses
  `type: minecraft:crafting_shaped`, `pattern`, `key` (`{"item": id}`), `result: {"id":..,"count":..}`.
- Models: `assets/<modid>/models/item/<name>.json` — `item/handheld` for swords/tools,
  `item/generated` for plain items. Lang: `assets/<modid>/lang/en_us.json`.

## Report honestly — the 0%-false-success rule applies to our own output too (Phases 0–2)

Unimplemented CLI paths return a structured error with a **non-zero** exit; discovered benchmark
cases with no runner are reported **errored**, never passed. Do not claim "the mod builds" without
having built it. This discipline is why Phase 3 (actual compilation) is gated behind a real build,
not assumed.

