# AMC Benchmark Corpus

Layout and rules per `SPEC-005 §24`. The runner (`amc bench`) discovers **runnable** cases under
`intent/`, `integration/`, and `e2e/`. Everything else is support data.

```text
benchmarks/
├── schemas/     # case + report JSON Schemas
├── integration/ # Layer-3 pipeline integration cases
├── e2e/         # Layer-4 full-loop cases: create/modify/references/clarification/unsupported/rollback/runtime
├── fixtures/    # projects/, minecraft/, examples/ (examples are NOT runnable)
└── reports/     # generated run reports (gitignored except .gitkeep)
```

## Data separation (`SPEC-005 §23`)

Keep these partitions disjoint — no leakage of near-duplicate phrasing or equivalent fixtures:
`training`, `validation`, `heldout intent test`, `permanent regression`, `integration fixtures`,
`full-loop acceptance`. The **protected** set (`intent/protected/`) is held out from training and
ordinary editing (`SPEC-005 §7`).

## Phase 0 status

The runnable case directories are intentionally **empty** (`.gitkeep`) so the Phase 0 gate — the
runner executes the empty suite and emits a zeroed report — is meaningful. Worked examples of the
case formats live in `fixtures/examples/` for reference; they are not executed. The ≥ 50-case
suite is authored in Phase 6 (`implementation/phase6/`).

## Running

```bash
amc bench benchmarks/ --json
amc bench benchmarks/e2e --category create
```
