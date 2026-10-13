# 0005 — CLI command surface (amc-cli)

- **Phase:** 0
- **Workstream:** core-engine
- **Effort:** Small
- **Owner:** unassigned
- **Status:** done

## Objective

Expose the full `amc` command surface from day one so the SPEC-006 acceptance sequence exists
as a stable interface, even before behavior is implemented.

## Background

`SPEC-003 §3` (CLI responsibilities & prohibitions), `SPEC-006 §5` and `SPEC-005 §31`
(acceptance command sequence: `init`, `request`, `test`, `play`, `revert`), `SPEC-005 §25`
(`bench`).

## Dependencies

`phase0/0001`, `phase0/0002`, `phase0/0004`.

## Implementation Notes

- `clap` derive. Subcommands: `init`, `request`, `test`, `play`, `revert`, `bench`.
- Every not-yet-implemented subcommand returns a structured result (human + `--json`) tagged
  `Unimplemented` with the owning component, and a **non-zero** exit code so nothing can be
  mistaken for success (`SPEC-005 §18` false-success = 0%).
- CLI must not interpret intent, mutate state, or generate code.

## Acceptance Tests

- `amc --help` lists all subcommands.
- `amc init ruby_world` returns a structured "unimplemented" result (non-zero exit).
- `amc bench benchmarks/` runs (delegates to `phase0/0004`).

## Benchmark Coverage

None yet; wired in Phase 5.

## Traceability

- Product requirement(s): `FR-001` (English interface entry point), primary workflow `SPEC-001 §4`.
- Component(s): `amc-cli`.
