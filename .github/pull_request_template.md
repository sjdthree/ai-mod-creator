## Summary

<!-- What changed and why. Link the issue it fixes, e.g. "Fixes #12". -->

## Traceability (maintainers; optional for outside contributors)

- Work item: <!-- impl:phaseN/NNNN -->
- Product requirement(s): <!-- FR-00x / NFR-00x -->
- MCL element(s): <!-- doc type / operation / schema section -->
- Component(s) / crate(s):
- Benchmark case(s): <!-- COMP-* / FT-* / INT-* / E2E-* -->

## Checklist

- [ ] Implementation complete
- [ ] `./scripts/ci.sh` passes (fmt, clippy, tests)
- [ ] Integration tests pass (if applicable this phase)
- [ ] Required benchmark cases pass
- [ ] Docs updated if interfaces changed (specs untouched; backlog/ADR/traceability updated)
- [ ] CI green, no regression

## False-success guard (SPEC-005 §18)

- [ ] No path reports success for unverified behavior
- [ ] A failed operation leaves the last verified state intact (or N/A)
- [ ] Unimplemented paths return a structured error with non-zero exit (or N/A)

## Failure ownership (if this fixes a failing case)

Primary owner from `SPEC-005 §17`: <!-- e.g. GENERATOR, MODEL_INTERPRETATION, BUILD_ENVIRONMENT -->
