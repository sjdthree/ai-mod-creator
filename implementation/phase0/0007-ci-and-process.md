# 0007 — CI pipeline & process scaffolding

- **Phase:** 0
- **Workstream:** qa
- **Effort:** Small
- **Owner:** unassigned
- **Status:** done

## Objective

Continuous integration implementing the every-commit tier, plus the process artifacts
(contributing rules, templates, ADRs, traceability) that keep the project auditable.

## Background

`SPEC-005 §28` (CI schedule: every commit / every model candidate / nightly / release
candidate). `SPEC-006` Living Engineering Backlog & Traceability. `SPEC-005 §18` (0%
false-success), §17 (failure ownership).

## Dependencies

`phase0/0001`–`0006`.

## Implementation Notes

- `.github/workflows/ci.yml`: `fmt --check`, `clippy -D warnings`, `cargo test --workspace`,
  and `amc bench benchmarks/ --json` on the empty suite. Model-candidate / nightly / RC tiers
  are documented placeholder jobs enabled in later phases.
- Process: `CONTRIBUTING.md`, `CHANGELOG.md`, `CODEOWNERS`, `.github/ISSUE_TEMPLATE/*`,
  `.github/pull_request_template.md`, `docs/adr/*`, `docs/traceability.md`.

## Acceptance Tests

- CI is green on the first PR of the branch.
- PR template enforces the DoD + failure-ownership checklist.

## Benchmark Coverage

Runs the empty suite as a smoke gate.

## Traceability

- Product requirement(s): process for all FR/NFR.
- Component(s): repo-wide.
