# 0001 — Generator capability registry

- **Phase:** 2
- **Workstream:** core-engine
- **Effort:** Small
- **Owner:** unassigned
- **Status:** done

## Objective

Machine-readable registry of the concept/subtype and recipe combinations the generator can
implement, consumed by the validator (`SPEC-004 §39`).

## Background

`SPEC-004 §39`, §9. Delivered early in Phase 1 to unblock the validator's capability stage.

## Implementation Notes

Implemented as `amc-generator::capability` (`is_concept_supported`, `subtypes`,
`recipe_types`, `is_recipe_type_supported`). The validator (`amc-validator`) enforces it.

## Acceptance Tests

- `is_concept_supported("weapon", Some("sword"))` true; `Some("laser")` false. (Done.)

## Traceability

- Product requirement(s): `FR-004`.
- MCL element(s): capability registry (`SPEC-004 §39`).
- Component(s): `amc-generator`.
