# 0003 — Resource generation (models, lang)

- **Phase:** 2
- **Workstream:** core-engine
- **Effort:** Medium
- **Owner:** unassigned
- **Status:** done

## Objective

Generate the client resources each concept needs: item model JSON and localized display names.

## Background

`SPEC-003 §9`. NeoForge 1.21.1 resource layout: `assets/<modid>/models/item/<name>.json`,
`assets/<modid>/lang/en_us.json`.

## Dependencies

`phase2/0002`.

## Implementation Notes

- Item model: `{"parent":"item/generated","textures":{"layer0":"<modid>:item/<name>"}}`; sword uses
  `item/handheld`.
- Lang: `{"item.<modid>.<name>": "<display_name>"}`, keys sorted for determinism.

## Acceptance Tests

- Each generated concept has a model referencing its texture and a lang entry with its display
  name.
- Sword models use `item/handheld`; plain items use `item/generated`.

## Benchmark Coverage

`COMP-GEN-*`.

## Traceability

- Product requirement(s): `FR-004`.
- Component(s): `amc-generator`.
