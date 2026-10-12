# 0005 — Deterministic placeholder textures

- **Phase:** 2
- **Workstream:** core-engine
- **Effort:** Medium
- **Owner:** unassigned
- **Status:** done

## Objective

Generate a 16×16 placeholder PNG texture for each concept, deterministically from its appearance
`asset_seed` and palette.

## Background

`SPEC-004 §17` (appearance: template, palette, asset_seed), `SPEC-005 §22` (same seed → identical
bytes).

## Dependencies

`phase2/0002`.

## Implementation Notes

- Pure function `(asset_seed, palette) -> PNG bytes`. Use a small deterministic PRNG seeded by
  `asset_seed`; no wall-clock, no `Math.random`.
- Palette defaults per material theme when appearance is absent; derive a seed from the concept's
  `namespace_id` when `asset_seed` is absent so output stays deterministic.
- PNG encoding must be byte-stable (fixed color type, no timestamp chunks).

## Acceptance Tests

- Same seed + palette → byte-identical PNG across runs.
- Different seeds → different bytes.

## Benchmark Coverage

`COMP-GEN-*` determinism.

## Traceability

- Product requirement(s): `FR-004`, `FR-007`.
- MCL element(s): appearance (`SPEC-004 §17`).
- Component(s): `mce-generator`.
