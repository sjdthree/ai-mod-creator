# 0006 — Component trait stubs

- **Phase:** 0
- **Workstream:** core-engine
- **Effort:** Small
- **Owner:** unassigned
- **Status:** done

## Objective

Define the stable trait interface for each pipeline component and provide compiling stubs that
return `Unimplemented`, so later phases fill in behavior behind fixed boundaries.

## Background

`SPEC-003 §4–§12` (component responsibilities, inputs, outputs, prohibitions). Architectural
success criteria: components must be independently testable and failures assignable to one
owner (`SPEC-003 §20`, `SPEC-005 §17`).

## Dependencies

`phase0/0001`, `phase0/0002`.

## Implementation Notes

- One trait per component crate (`amc-state`, `amc-validator`, `amc-generator`, `amc-build`,
  `amc-verify`, `amc-launch`, `amc-intent`), each with a doc comment listing its prohibited
  responsibilities verbatim from `SPEC-003`.
- Stubs return `Err(AmcError::unimplemented(Component::X))` — no `todo!()`/`panic!()`.
- Each crate has a placeholder test asserting the stub is wired.

## Acceptance Tests

- `cargo test --workspace` passes.
- Each component crate exposes its trait and a default stub impl.

## Benchmark Coverage

None (structural).

## Traceability

- Product requirement(s): supports all FR via component boundaries.
- Component(s): all pipeline crates.
