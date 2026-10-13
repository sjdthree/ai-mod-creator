# 0001 — Repository structure & Rust workspace

- **Phase:** 0
- **Workstream:** core-engine
- **Effort:** Small
- **Owner:** unassigned
- **Status:** done

## Objective

Establish the monorepo layout and a Cargo workspace with all component crates so a clean
checkout builds.

## Background

`SPEC-006` Phase 0 (Repository structure, Rust workspace). Component decomposition from
`SPEC-003 §2` (nine primary components).

## Dependencies

None.

## Implementation Notes

- Workspace root `Cargo.toml` with members under `crates/`.
- Crates: `amc-core`, `amc-schema`, `amc-state`, `amc-validator`, `amc-generator`,
  `amc-build`, `amc-verify`, `amc-launch`, `amc-intent`, `amc-bench`, `amc-cli`.
- `amc-cli` is the only binary; it may orchestrate but must not interpret intent, mutate
  state, or generate code (`SPEC-003 §3` prohibited responsibilities).

## Acceptance Tests

- `cargo build --workspace` succeeds.
- `cargo fmt --check` and `cargo clippy --workspace -- -D warnings` pass.

## Benchmark Coverage

None (structural).

## Traceability

- Product requirement(s): infrastructure for all FR/NFR.
- Component(s): all crates.
