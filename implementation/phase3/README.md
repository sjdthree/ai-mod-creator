# Phase 3 — Build & Verification (Weeks 6–7)

**Gate (`SPEC-006`):** generated mod builds; Minecraft launches; Ruby Blade can be crafted and
used. Depends on Phase 2.

> ⚠️ **Top project risk (ADR-0004).** Week 6 must begin with a spike: confirm NeoForge GameTest
> discovery works in production (may need manual registration) and stand up headless client
> smoke (HeadlessMC + Xvfb). Do this before building out the verification harness.

## Work items

| Item | Title | Effort | Status |
|---|---|---|---|
| [0000](./0000-verification-spike.md) | Spike: build + GameTest + headless verification | Medium | 🟢 Build + GameTest verified (ADR-0005/0006); persistence + client smoke open |
| [0001](./0001-build-manager.md) | Build Manager (Gradle orchestration + log classification) | Large | ✅ Core done; wire-up open |
| 0002 | Dedicated-server boot + registry verification (`SPEC-005 §15` L3) | Large | 🟡 Registry proven via GameTest; standalone boot open |
| 0003 | Generated GameTests from verification requirements (L4) | Large | ✅ Done — registry_exists GameTests (ADR-0006) |
| 0004 | Persistence checks (save/reload) (L5) | Medium | Not started |
| 0005 | Client smoke test (L6, Xvfb) | Medium | Not started |
| 0006 | Minecraft launcher after gates pass (`SPEC-003 §12`) | Medium | Not started |
| [0007](./0007-verification-harness-core.md) | Verification Harness core (planning + report) | Medium | ✅ Done (+ `gametest` LevelRunner) |

## Environment

The runtime levels need JDK 21 + network + (for client smoke) Xvfb — all provided by the
**dev container** (`docs/dev-container.md`), where the build and GameTest spike were verified. The
host is never touched. Remaining runtime work: persistence (L5), headless **client** smoke (L6),
and standalone dedicated-server boot (L3) if wanted separately from GameTest.

Benchmark focus: `COMP-BUILD-*`, `COMP-VERIFY-*`; `E2E-*` runtime cases (now runnable in the
container).
