# ADR-0007 — Intent model: fine-tuned FunctionGemma 270M with tiered escalation

- **Status:** Accepted
- **Date:** 2026-07-17
- **Refines:** ADR-0002 (ML subsystem boundary); grounds the `SPEC-003 §5` "functiongemma" base
  model with concrete facts.

## Context

`SPEC-003 §5` names `functiongemma` as the base model but leaves size, role, and architecture
open. Research corrected several assumptions: FunctionGemma is **Gemma 3 270M** fine-tuned for
**function/tool calling** — small, on-device, meant to be *specialized* by fine-tuning, and
explicitly positioned by Google as either a narrow agent or a "traffic controller" that routes
hard work to a larger model. It is **not** a coder or dialogue model, and should not be asked to
know Minecraft Java APIs.

## Decision

Adopt a **tiered local inference** architecture:

| Layer | Model / system | Responsibility |
|---|---|---|
| Intent (resident) | **fine-tuned FunctionGemma 270M** | English → operation selection, argument extraction, reference resolution, clarify/split/escalate/no-op decisions |
| Clarification text | templates or a small instruct model | concise player-facing questions |
| Standard implementation | **no LLM** | the deterministic MCL compiler (already built) |
| Complex planning | conditional 1B–3B instruct | decompose into multiple supported operations |
| Custom extension / hard repair | conditional local 3B–7B coder | minimal extension code against MCE's **stable internal API** |
| Verification | deterministic | build, GameTest, logs, pass/fail |

FunctionGemma stays resident (well under 1 GB quantized); larger specialists load **only when
needed** and are unloaded/paused before launching Minecraft (which itself wants several GB).

### Consequences for the operation contract

- Expose **~20–30 composable operations** (create/update/delete/rename concept, add/update
  behavior, add recipe/spawn/loot/progression, set appearance/sound, test, revert, clarify). The
  **object type lives in the operation payload**, not as a separate tool — a 270M model handles a
  small operation set far better than hundreds of object-specific tools. This matches the existing
  MCL operation set (`SPEC-004 §19`).
- **Constrained decoding guarantees syntax, not meaning.** Keep the four validation levels:
  grammar → schema → **semantic** (IDs/refs/ranges/combinations) → **Minecraft capability**. The
  `mce-validator` staged pipeline already implements grammar/schema/reference/property/semantic/
  capability; this ADR affirms that split.
- **The model never edits generated files.** Failures route by ownership (`SPEC-005 §17`): bad
  model output → FunctionGemma retry/clarify; bad MCL semantics → deterministic fix/clarify;
  generator defect → engineering (never model-patched); custom-extension compile error → coder
  loop; missing asset → asset generator; adapter incompatibility → adapter layer.

## Fine-tuning target (what FunctionGemma is trained to do)

Operation selection, exact argument extraction, reference resolution, when to clarify, when to
split into multiple operations, when to escalate, and **when to change nothing**. Not to generate
Java or explain Minecraft APIs.

## Consequences

- Resident memory stays tiny; correctness improves by keeping each model narrow.
- As the deterministic coverage grows, model size matters progressively less.
- Custom-behavior seam (`generated/` overwritten, `extensions/` never, `project.mce.json` = source
  of truth) becomes the key boundary for the specialist tier — tracked as its own work item.
