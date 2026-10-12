# ADR-0004 — Verification approach & GameTest/headless risk

- **Status:** Proposed (spike required, Phase 3)
- **Date:** 2026-07-17

## Context

The product's defining guarantee is **0% false-success** (`SPEC-005 §18`): a request is
"complete" only when the behavior is verified in the pinned Minecraft runtime (`SPEC-005 §12`).
Automated runtime verification is therefore load-bearing, and it is also the riskiest area:

- NeoForge **GameTest discovery reportedly does not work reliably in production**; tests may
  need explicit manual registration and structure-template handling.
- **Headless client** smoke verification (`SPEC-005 §15` L6) needs HeadlessMC + a virtual
  framebuffer (Xvfb) in CI.
- Modern Minecraft networking makes server-only client verification unreliable; a
  server + lightweight client model is needed.

## Decision (proposed)

Adopt the layered verification model from `SPEC-005 §15` (static → build → server boot →
GameTest → persistence → client smoke). **Before** building the harness, run a Phase 3 week-6
**spike** proving: (a) a registered GameTest runs and reports pass/fail via exit code on a
dedicated server; (b) a headless client boots and loads the mod under Xvfb. If GameTest
discovery proves unworkable, fall back to a custom server-side assertion mod driven over RCON.

## Consequences

- **+** De-risks the core guarantee early instead of at MVP hardening.
- **−** Possible custom test-harness mod if GameTest is inadequate.
- **Supersede** this ADR with the spike's outcome and the chosen mechanism.

## References

- headlesshq/mc-runtime-test (headless MC in CI, HeadlessMC + Xvfb).
- NeoForge GameTest production-discovery limitations (community reports).
