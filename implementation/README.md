# Living Engineering Backlog

This directory is the **living engineering backlog** for the AI-mod creator (AMC),
as mandated by `SPEC-006` (Implementation Roadmap). The six specification documents in
`../AI_Mod_Creator_Engineering_Package_v1.0/` are the **stable contract** and change
slowly. Day-to-day work is tracked here as small, independently testable **work items**.

## How it works

- Each phase (`phase0/` … `phase6/`) holds numbered work items: `NNNN-short-title.md`.
- Every work item uses [`TEMPLATE.md`](./TEMPLATE.md) and stays **≤ one engineer-week**. Larger
  features are decomposed into independently testable tasks.
- Every work item is **traceable** (see [`../docs/traceability.md`](../docs/traceability.md)):
  it references product requirements, MCL elements, benchmark case IDs, and components.
- A work item is **Done** only per the checklist below.

## Workstreams (labels, not people)

This is a solo + Claude Code project. The four "teams" in `SPEC-006` are used as **labels** to
keep concerns separated:

| Label | Owns |
|---|---|
| `core-engine` | state manager, validator, transactions, generator |
| `ai-ml` | datasets, fine-tuning, intent evaluation, model promotion |
| `mc-integration` | build pipeline, adapter, GameTests, runtime verification |
| `qa` | benchmark corpus, regression suite, release qualification |

## Definition of Done (SPEC-006)

A work item is complete only when:

1. Implementation is merged.
2. Required unit tests pass.
3. Required integration tests pass.
4. Required benchmark cases pass.
5. Documentation updated if interfaces changed.
6. CI passes with no regression.

## Phase status board

| Phase | Theme | Gate (SPEC-006) | Status |
|---|---|---|---|
| 0 | Foundation | Clean checkout builds; unit tests run; empty benchmark suite runs | **In progress** |
| 1 | Canonical state | create/update/delete succeed; rollback restores; deterministic hashes | Not started |
| 2 | Deterministic generator | Ruby Blade generates reproducibly; stable output hashes | Not started |
| 3 | Build & verification | Mod builds; Minecraft launches; Ruby Blade craftable/usable | Not started |
| 4 | Intent model | English → valid MCL ops; unit suite runs; protected set gates | Not started |
| 5 | End-to-end loop | `init → request → test → play` succeeds with no manual edits | Not started |
| 6 | MVP hardening | 50 e2e cases; regression suite; **0% false-success**; release | Not started |

## Conventions

- Case ID prefixes (from `SPEC-005`): `COMP-*` (component), `FT-*` (fine-tuning intent),
  `E2E-*` (full-loop). Failure ownership taxonomy: see `SPEC-005 §17`.
- Reference a work item from commits/PRs as `impl:phaseN/NNNN`.
