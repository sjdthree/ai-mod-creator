# 0004 — Recipe generation

- **Phase:** 2
- **Workstream:** core-engine
- **Effort:** Medium
- **Owner:** unassigned
- **Status:** done

## Objective

Generate NeoForge recipe JSON (shaped/shapeless/smelting/blasting) from recipe state.

## Background

`SPEC-004 §15`, §24. NeoForge 1.21.1 data layout: `data/<modid>/recipe/<name>.json`; shaped format
uses `type: minecraft:crafting_shaped`, `pattern`, `key` (ingredients as `{"item": id}`), and
`result: {"id": <id>, "count": n}`.

## Dependencies

`phase2/0002`. **Blocked on** extending canonical state with recipes — the Phase 1 state manager
handles concepts only. Add a `recipes` collection to `CanonicalState` and an `add_recipe`
operation before implementing this.

## Implementation Notes

- Resolve all recipe references to registry ids at generation time (they resolve to stable ids in
  state; map to `namespace_id`).
- Deterministic ordering by recipe id.

## Acceptance Tests

- The Ruby Blade shaped recipe (two rubies + one stick) generates the expected JSON.
- References resolve to registry ids.

## Benchmark Coverage

`COMP-GEN-*`; feeds `E2E-*` recipe cases.

## Traceability

- Product requirement(s): `FR-004`.
- MCL element(s): recipe (`SPEC-004 §15`).
- Component(s): `mce-generator`, `mce-state` (recipe model).
