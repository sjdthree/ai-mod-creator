# Phase 2 — Deterministic Generator (Weeks 4–5)

**Gate (`SPEC-006`):** the Ruby Blade project generates reproducibly; output hashes are stable.
Depends on Phase 1.

## Work items

| Item | Title | Effort | Status |
|---|---|---|---|
| [0001](./0001-capability-registry.md) | Generator capability registry | Small | ✅ Done (Phase 1) |
| [0002](./0002-java-generation.md) | Project skeleton & Java generation (item, weapon) | Large | ✅ Done |
| [0003](./0003-resource-generation.md) | Resource generation (models, lang) | Medium | ✅ Done |
| [0004](./0004-recipe-generation.md) | Recipe generation | Medium | ✅ Done |
| [0005](./0005-textures.md) | Deterministic placeholder textures | Medium | ✅ Done (noise placeholder) |
| [0006](./0006-manifest-hashing.md) | Generation manifest & output hashing | Medium | ✅ Done |
| [0007](./0007-texture-engine.md) | Template/palette texture engine (replace noise) | Large | Not started |

The generator must not call the model, interpret English, mutate state, or self-repair
(`SPEC-003 §9` prohibitions). Benchmark focus: `COMP-GEN-*`.

## Notes

- Recipe state landed after the first slice: `CanonicalState` now carries a `recipes` map, the
  state manager handles `add_recipe`/`remove_recipe` (resolving references to canonical form), and
  the generator emits `data/<modid>/recipe/*.json`. The full Ruby Blade (weapon + shaped recipe)
  path is now generable.
- Still follow-on: dedicated block/tool/food generators (currently emitted as basic items).
- **Textures (`0005`) are a seeded-noise placeholder** — deliberately, to prove determinism. The
  research (internal research review) is right that noise isn't a
  usable asset; `0007` replaces it with a template/palette pixel-art engine.
- Actual **compilation** of the generated project is the Phase 3 gate; Phase 2's gate is
  deterministic generation with stable output hashes.
