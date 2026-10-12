# 0004 — Benchmark runner skeleton (mce-bench) & corpus layout

- **Phase:** 0
- **Workstream:** qa
- **Effort:** Medium
- **Owner:** unassigned
- **Status:** done

## Objective

A benchmark runner that discovers the corpus tree, executes an empty suite, and writes the
mandated report artifacts — the Phase 0 "benchmark runner executes empty suite" gate.

## Background

`SPEC-005 §24` (repository layout), §25 (CLI commands & options), §26 (report format:
`summary.json`, `cases.jsonl`, `failures.jsonl`, `metrics.json`, `environment.json`,
`artifacts/`), §19 (Verified Request Completion Rate scoring).

## Dependencies

`phase0/0001`, `phase0/0002`.

## Implementation Notes

- `benchmarks/` tree exactly per `SPEC-005 §24`.
- `mce bench <path>` supports `--json`, `--report-dir`, `--category`, `--case`, `--fail-fast`.
- With no cases, produce a valid, well-formed report with zeroed metrics and an
  `environment.json` capturing commit, versions, os/cpu, timestamp.
- Timestamps come from the OS clock at runtime (allowed here — the report is not a
  deterministic hash input).

## Acceptance Tests

- `mce bench benchmarks/ --json` on the empty suite exits 0 and writes
  `benchmarks/reports/<run>/summary.json` with `total_cases: 0`.
- Report files conform to `report.schema.json`.

## Benchmark Coverage

Self (the runner's own tests are `COMP-BENCH-*`).

## Traceability

- Product requirement(s): `FR-005` (automatic verification reporting), success metrics `SPEC-001 §9`.
- Component(s): `mce-bench`, `mce-cli`.
