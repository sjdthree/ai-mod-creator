# Traceability Matrix

Per `SPEC-006` (Traceability), every work item links a product requirement, MCL element,
benchmark case, and component; conversely benchmark cases and code changes reference their
originating work item. This file is the index. Keep it current as part of each work item's
Definition of Done.

## Specification map

| Doc file | Spec ID | Title |
|---|---|---|
| `01_PRODUCT_REQUIREMENTS.md` | SPEC-001 | Product Requirements |
| `02_SYSTEM_ARCHITECTURE.md` | SPEC-003 | System Architecture |
| `03_MCL_SPECIFICATION.md` | SPEC-004 | Mod Creation Language |
| `04_BENCHMARKS_AND_TEST_PLAN.md` | SPEC-005 | Benchmarks & Test Plan |
| `05_IMPLEMENTATION_ROADMAP.md` | SPEC-006 | Implementation Roadmap |

> **Note — SPEC-002 is unassigned.** The package numbers files `01`–`05` but the internal Spec
> IDs are `001, 003, 004, 005, 006`. There is no `SPEC-002`. Treat this as an intentional gap
> (or a reserved slot); not a blocker. If a sixth document is authored, `SPEC-002` is available.

## Component → crate map (`SPEC-003 §2`)

| Architecture component | Crate | Prohibitions (summary) |
|---|---|---|
| CLI | `amc-cli` | no intent interp, no state mutation, no code gen (`§3`) |
| Intent Adapter | `amc-intent` | no state mutation, no Java, no Gradle (`§4`) |
| Fine-Tuning & Eval | separate model repository | no prod-state mutation, no generator patching (`§5`) |
| Operation Validator | `amc-validator` | no intent inference, no code gen, no commit (`§7`) |
| Project State Manager | `amc-state` | no English interp, no impl gen, no false-verify (`§8`) |
| Deterministic Generator | `amc-generator` | no model calls, no self-repair (`§9`) |
| Build Manager | `amc-build` | no file edits, no gameplay-success claim (`§10`) |
| Verification Harness | `amc-verify` | no behavior alteration, compile ≠ success (`§11`) |
| Minecraft Launcher | `amc-launch` | no validation, no code gen, no state change (`§12`) |
| Benchmark runner | `amc-bench` | scoring/reporting only (`SPEC-005 §25`) |
| Shared primitives | `amc-core` | ids/errors/logging/hashing |

## Benchmark case-ID conventions (`SPEC-005 §15`, examples §3–§14)

| Prefix | Layer | Meaning |
|---|---|---|
| `COMP-*` | 1 | Deterministic component test (e.g. `COMP-REF-001`, `COMP-STATE-004`, `COMP-GEN-009`) |
| `FT-*` | 2 | Fine-tuning intent unit test (e.g. `FT-WEAPON-001`) |
| `INT-*` | 3 | Pipeline integration test |
| `E2E-*` | 4 | Full-loop Minecraft test (e.g. `E2E-WEAPON-001`, `E2E-ROLLBACK-002`) |

Failure-ownership taxonomy for every failing case: `SPEC-005 §17`.

## Work item ↔ requirement ↔ component (seed — Phase 0)

| Work item | Product req | MCL element | Component(s) | Benchmark |
|---|---|---|---|---|
| `phase0/0001` | infra | — | all crates | — |
| `phase0/0002` | FR-007, NFR-003 | ids `§5`, hashing | `amc-core` | `COMP-GEN-009` (precursor) |
| `phase0/0003` | FR-004, FR-002 | all doc types | `amc-schema` | `COMP-*` schema |
| `phase0/0004` | FR-005 | — | `amc-bench`, `amc-cli` | `COMP-BENCH-*` |
| `phase0/0005` | FR-001 | operation `§19` | `amc-cli` | — |
| `phase0/0006` | all FR | — | all pipeline crates | — |
| `phase0/0007` | process | — | repo-wide | empty-suite smoke |
| `phase1/0001` | FR-002 | concept `§8`, all docs | `amc-state` | `COMP-STATE-*` |
| `phase1/0002` | FR-003 | references `§6`, selectors | `amc-state` | `COMP-REF-*` |
| `phase1/0003` | FR-002, NFR-003 | operations `§19`, validation `§33` | `amc-validator`, `amc-generator` | `COMP-*` validation |
| `phase1/0004` | FR-002, FR-007, NFR-003 | transaction `§31` | `amc-state` | `COMP-STATE-004` |
| `phase1/0005` | FR-006 | `revert_change §30` | `amc-state` | `COMP-STATE-*` rollback |
| `phase1/0006` | FR-005 | verification requirement `§32` | `amc-state`, `amc-verify` | feeds `E2E-*` |
| `phase2/0001` | FR-004 | capability registry `§39` | `amc-generator` | capability tests |
| `phase2/0002` | FR-004, FR-007 | concept `§8`–§14 | `amc-generator` | `COMP-GEN-*` |
| `phase2/0003` | FR-004 | — (resources) | `amc-generator` | `COMP-GEN-*` |
| `phase2/0005` | FR-004, FR-007 | appearance `§17` | `amc-generator` | `COMP-GEN-*` |
| `phase2/0006` | FR-007 | generator determinism `§22` | `amc-generator` | `COMP-GEN-009` |
| `phase3/0001` | FR-005, NFR-003 | build manager `§10` | `amc-build` | `COMP-BUILD-*` |
| `phase3/0007` | FR-005, NFR-003 | harness `§11`, levels `§15` | `amc-verify` | `COMP-VERIFY-*` |
| `phase3/0000` | FR-005 | verification `§12` (env-gated) | `amc-build/verify/launch` | `E2E-*` |

_Extend this table per work item as phases progress._
