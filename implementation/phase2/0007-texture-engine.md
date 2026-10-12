# 0007 — Template/palette texture engine (replace noise placeholder)

- **Phase:** 2 (follow-on)
- **Workstream:** core-engine
- **Effort:** Large
- **Owner:** unassigned
- **Status:** not started

## Objective

Replace the current random-noise placeholder texture generator with a deterministic **shape-,
palette-, and template-aware** pixel-art system that produces usable Minecraft item/block textures.

## Background

Research (internal research review): "Random colored noise will produce
unusable textures." The Phase 2 `texture.rs` intentionally shipped a seeded-noise placeholder to
prove determinism/plumbing; this item makes textures actually usable. `SPEC-004 §17` (appearance:
template, palette, style tags, asset_seed).

## Dependencies

`phase2/0005` (current placeholder), appearance state (`SPEC-004 §17`).

## Implementation Notes

- Deterministic pipeline: (1) silhouette templates by object category (sword, pickaxe, ingot,
  block, …); (2) curated Minecraft-compatible palettes; (3) semantic material patterns (metallic
  blade, gem, wood handle); (4) highlight/shadow/edge passes; (5) deterministic seed; (6)
  transparent-background rules; (7) asset validation.
- Optional user regeneration commands ("make it darker", "add glowing blue edges") map to
  appearance updates, not model file edits.
- A separate local image model may improve assets later, but must not block the first release.

## Acceptance Tests

- A "ruby sword" yields a recognizable sword silhouette with a ruby-red blade + dark handle +
  outline — not noise.
- Same seed + appearance → byte-identical PNG (determinism preserved).

## Traceability

- Product requirement(s): `FR-004`.
- MCL element(s): appearance `SPEC-004 §17`.
- Component(s): `mce-generator` (`texture`/appearance).
