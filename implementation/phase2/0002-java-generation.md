# 0002 — Project skeleton & Java generation (item, weapon)

- **Phase:** 2
- **Workstream:** core-engine (mc-integration reviews)
- **Effort:** Large
- **Owner:** unassigned
- **Status:** done

## Objective

Generate a NeoForge 1.21.1 project skeleton (Gradle files, `neoforge.mods.toml`, main mod class)
and Java registration for `item` and `weapon` (sword) concepts, deterministically from canonical
state.

## Background

`SPEC-003 §9` (generator), ADR-0003 (MC 1.21.1 / NeoForge 21.1.x / Java 21). NeoForge 1.21.1
APIs: `DeferredRegister.createItems`, `@Mod` main class, `SimpleTier` + `SwordItem` +
`SwordItem.createAttributes` (per NeoForged docs).

## Dependencies

`phase1/0001` (data model), `phase2/0001` (capability).

## Implementation Notes

- Real `Generator` impl in `mce-generator` taking `&CanonicalState`.
- Package derived from project namespace; mod class = PascalCase(namespace).
- `ModItems.java`: one `DeferredRegister.Items`; basic items via `registerSimpleItem`; swords via
  `SimpleTier(durability, mining_speed, 0, enchantability, repairIngredient)` + `new SwordItem(tier,
  Properties().attributes(SwordItem.createAttributes(tier, attack_damage-1, attack_speed)))`.
- Deterministic: iterate concepts in `namespace_id` order; no timestamps in generated files.
- Generator must not call the model or interpret English (`SPEC-003 §9`).

## Acceptance Tests

- Generating the Ruby Blade (weapon) + Ruby (item) project writes the expected file tree.
- Java references the correct registry names and tier params.
- Generating twice yields byte-identical files.

## Benchmark Coverage

`COMP-GEN-*` (snapshot/determinism).

## Traceability

- Product requirement(s): `FR-004`, `FR-007`.
- Component(s): `mce-generator`, consumes `mce-state`.
