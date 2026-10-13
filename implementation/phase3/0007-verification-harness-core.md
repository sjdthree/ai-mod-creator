# 0007 — Verification Harness core (amc-verify)

- **Phase:** 3
- **Workstream:** mc-integration / qa
- **Effort:** Medium
- **Owner:** unassigned
- **Status:** in progress

## Objective

The harness's environment-independent core: model the verification levels, plan the checks needed
for a transaction's verification requirements, aggregate per-level results into a structured
report with pass/fail and failure ownership. Runtime-dependent levels (server boot, GameTest,
client smoke) plug into this and are exercised in the env-gated items `0002`–`0005`.

## Background

`SPEC-003 §11` (harness responsibilities/prohibitions), `SPEC-005 §15` (levels 1–6), §17 (failure
ownership), §18 (false success = 0%). `SPEC-004 §32` (verification requirements — produced by
`amc-state`).

## Dependencies

`phase1/0006` (verification requirements), `phase3/0001` (build result feeds level 2).

## Implementation Notes

- `VerificationLevel` (static/build/server_boot/registry/gametest/persistence/client_smoke) and a
  `plan(requirements) -> Vec<PlannedCheck>` that maps each requirement's assertion to the level(s)
  needed.
- `VerificationReport { passed, level_results, failed_requirement_ids, failure_owner }`. A report
  passes only when **all required** checks pass — compilation alone is never success
  (`SPEC-003 §11`).
- A `LevelRunner` trait so runtime levels (Phase 3 `0002`–`0005`) implement their own execution;
  the core stays testable with fake runners.
- Prohibited: must not alter requested behavior, commit state, or treat build success as gameplay
  success.

## Acceptance Tests

- `plan` maps a weapon's requirements to registry + gametest checks.
- A report with any failed required check → `passed == false` with the owning component set.
- A build-only pass (no gametest run) does not report overall success.

## Benchmark Coverage

`COMP-VERIFY-*` (planning + aggregation).

## Traceability

- Product requirement(s): `FR-005`, `NFR-003`; false-success `SPEC-005 §18`.
- Component(s): `amc-verify`, consumes `amc-state` requirements + `amc-build` result.
