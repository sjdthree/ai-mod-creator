# Benchmarks and Test Plan

**Specification ID:** SPEC-005  
**Status:** Draft 0.1  
**Audience:** Engineering, AI/ML, QA, and Minecraft integration teams  
**Purpose:** Define how AMC measures model quality, system correctness, and complete English-to-Minecraft success.

---

# 1. Objective

The benchmark system must answer three questions:

1. Did the fine-tuned model interpret the English request correctly?
2. Did the deterministic system implement the interpreted request correctly?
3. Did the requested behavior actually work inside Minecraft?

The benchmark program must distinguish these questions. A model failure must not be confused with a generator defect, and a successful build must not be confused with successful gameplay.

The primary product metric is:

> **Verified Request Completion Rate**

A request counts as complete only when the required gameplay behavior is verified in the pinned Minecraft runtime.

---

# 2. Test Layers

AMC uses four test layers.

```text
Layer 1: Deterministic component tests
Layer 2: Fine-tuning intent unit tests
Layer 3: Pipeline integration tests
Layer 4: Full-loop Minecraft tests
```

Each layer has a different purpose, runtime, and failure owner.

---

# 3. Layer 1 — Deterministic Component Tests

These tests cover code that should behave identically without language-model inference or Minecraft execution.

## Required targets

- JSON Schema validation
- operation normalization
- reference resolution
- canonical state transitions
- transaction commit and rollback
- namespace generation
- deterministic asset generation
- Java and resource generation
- verification-requirement generation
- build-log parsing
- Minecraft-log parsing
- benchmark scoring

## Required behavior

These tests must:

- run on every commit;
- use fixed fixtures;
- require no network access;
- require no language model;
- complete quickly;
- fail deterministically.

## Examples

```text
COMP-REF-001
Given one concept named "Ruby Blade",
a display-name reference resolves to its stable concept ID.
```

```text
COMP-STATE-004
Given a failed candidate transaction,
the verified parent state remains unchanged.
```

```text
COMP-GEN-009
Given identical canonical state and generator version,
the generated output hashes are identical.
```

---

# 4. Layer 2 — Fine-Tuning Intent Unit Tests

These tests evaluate only the English-to-MCL operation boundary.

They must not build or launch Minecraft.

Their purpose is to support rapid fine-tuning iteration.

## Required inputs

Every test case must contain:

```json
{
  "case_id": "FT-WEAPON-001",
  "category": "create",
  "initial_state_fixture": "empty_project",
  "request": "Create a ruby sword called Ruby Blade with 1200 durability.",
  "expected": {
    "operations": [],
    "must_clarify": false,
    "must_reject": false,
    "must_not_mutate": false
  }
}
```

## Required categories

### Creation

- item
- weapon
- tool
- food
- block
- recipe
- behavior
- appearance

### Modification

- property update
- rename
- delete
- recipe update
- behavior update
- appearance update

### Reference resolution

- exact display name
- alias
- last-created concept
- unique type reference
- ambiguous reference
- missing reference

### Multi-operation requests

- create concept plus recipe
- create concept plus appearance
- create concept plus behavior
- modify several properties
- create several related concepts

### Clarification

- ambiguous target
- ambiguous property
- contradictory request
- missing required value
- unsupported partial request

### Unsupported requests

- custom entity
- dimension
- GUI
- arbitrary Java
- custom networking
- world generation

### Non-mutating requests

- project question
- capability question
- explanation request
- inspection request

### Language robustness

- spelling mistakes
- missing punctuation
- short child-like wording
- verbose wording
- conversational references
- reordered clauses

---

# 5. Fine-Tuning Unit-Test Assertions

Each case may assert:

```text
operation sequence
operation count
operation type
target reference
required fields
field values
forbidden fields
clarification status
unsupported status
mutation status
schema validity
```

## Semantic comparison

Tests must compare parsed semantic output, not raw JSON string order.

Example:

```json
{
  "operation": "update_concept",
  "payload": {
    "target": {
      "reference_type": "display_name",
      "value": "Ruby Blade"
    },
    "changes": {
      "properties": {
        "durability": 1500
      }
    }
  }
}
```

Field ordering is irrelevant.

A materially different operation, target, or value is a failure.

---

# 6. Fine-Tuning Metrics

The test runner must calculate:

## Core metrics

- operation-selection accuracy;
- exact semantic match rate;
- field-level precision;
- field-level recall;
- numeric-value accuracy;
- target-resolution accuracy;
- schema-valid output rate;
- clarification precision;
- clarification recall;
- unsupported precision;
- unsupported recall;
- unintended-mutation rate;
- deterministic-repeat rate.

## Critical metrics

The following must always be shown separately:

```text
Unintended mutation
False supported classification
False success precursor
Wrong-target mutation
Schema-invalid output
```

Aggregate accuracy must never conceal these failures.

---

# 7. Protected Fine-Tuning Test Set

A protected test set must be held out from training and ordinary dataset editing.

It must include:

- ambiguous commands;
- destructive commands;
- unsupported commands;
- reference-heavy commands;
- multi-operation commands;
- numerical property changes;
- no-mutation cases.

The protected set must be versioned and access-controlled within the engineering workflow.

A candidate model must never be promoted based only on training or validation-set performance.

---

# 8. Fine-Tuning Regression Policy

Every model-owned production or benchmark failure must create or update a regression case.

The regression case must:

1. reproduce the original failure;
2. define the correct MCL output;
3. identify the initial state fixture;
4. include a failure classification;
5. be added to the permanent regression suite;
6. remain separate from the protected held-out set.

A corrected model must pass the new case without materially degrading existing cases.

---

# 9. Model Experiment Record

Every training experiment must produce a manifest:

```json
{
  "experiment_id": "exp-2026-07-17-001",
  "base_model": "functiongemma",
  "dataset_version": "dataset-0.3.0",
  "mcl_schema_version": "0.1.0",
  "training_config_hash": "sha256:...",
  "checkpoint": "checkpoint-4200",
  "quantization": "Q4_K_M",
  "source_commit": "git-sha",
  "unit_test_report": "reports/ft-unit-exp-001.json",
  "promotion_status": "rejected"
}
```

Required recorded parameters include:

- learning rate;
- epochs or steps;
- batch size;
- sequence length;
- adapter configuration;
- random seed;
- optimizer;
- training precision;
- validation cadence;
- quantization method;
- runtime inference settings.

---

# 10. Model Promotion Gate

A candidate model may be promoted only when:

1. schema-valid output meets the required threshold;
2. unintended mutation is zero on the protected set;
3. wrong-target mutation is zero on the protected set;
4. held-out intent quality meets or exceeds the current default model;
5. no critical regression is introduced;
6. selected integration tests pass;
7. selected full-loop Minecraft tests pass;
8. latency and memory fit the target desktop;
9. all model artifacts and reports are reproducible.

A candidate with better average accuracy but a critical regression must be rejected.

---

# 11. Layer 3 — Pipeline Integration Tests

Integration tests join adjacent components without requiring the full Minecraft client for every case.

## Required boundaries

```text
CLI → Intent Adapter
Intent Adapter → Validator
Validator → State Manager
State Manager → Generator
Generator → Build Manager
Build Manager → Verification Harness
```

## Required cases

### Intent plus validation

Verify that valid model output is normalized and invalid output is rejected.

### Validation plus state

Verify candidate-state creation, reference integrity, and atomic failure.

### State plus generation

Verify that canonical objects map to expected generated files.

### Generation plus build

Verify that supported fixtures compile and package.

### Build plus server verification

Verify server startup, registry loading, and generated GameTests.

---

# 12. Layer 4 — Full-Loop Minecraft Tests

These tests exercise the entire product:

```text
English request
    ↓
CLI
    ↓
Fine-tuned model
    ↓
MCL operations
    ↓
Validation
    ↓
Candidate state
    ↓
Generation
    ↓
Build
    ↓
Minecraft runtime
    ↓
Gameplay assertion
```

A full-loop test is the product-level source of truth.

---

# 13. Full-Loop Case Format

Each case must include:

```json
{
  "case_id": "E2E-WEAPON-001",
  "category": "create",
  "initial_state_fixture": "empty_project",
  "request": "Create a ruby sword called Ruby Blade with 1200 durability and 9 attack damage.",
  "expected_operations": [],
  "expected_state_assertions": [],
  "required_verification": [
    "schema",
    "semantic",
    "build",
    "server_boot",
    "registry",
    "gametest",
    "client_smoke"
  ],
  "expected_runtime_assertions": [],
  "maximum_retries": 1
}
```

---

# 14. Required Full-Loop Categories

The MVP suite must contain at least 50 cases.

## Creation — 10 cases

Examples:

- basic item;
- ingredient;
- sword;
- pickaxe;
- food;
- basic block;
- ore block;
- decorative block;
- shaped recipe;
- trigger-condition-action behavior.

## Modification — 10 cases

Examples:

- durability update;
- damage update;
- speed update;
- rename;
- appearance update;
- recipe replacement;
- loot update;
- behavior update;
- delete concept;
- restore concept through rollback.

## Reference resolution — 5 cases

Examples:

- exact name;
- alias;
- last-created concept;
- unique weapon;
- ambiguous target requiring clarification.

## Clarification — 5 cases

Examples:

- “Make it better”;
- ambiguous “it”;
- conflicting numeric values;
- unspecified target;
- unsupported plus supported intent.

## Unsupported — 5 cases

Examples:

- custom dragon entity;
- new dimension;
- custom GUI;
- arbitrary Java;
- multiplayer networking.

## Failure preservation and rollback — 5 cases

Examples:

- invalid recipe reference;
- generator failure fixture;
- build failure fixture;
- GameTest failure fixture;
- revert to earlier verified transaction.

## Runtime and regression — 10 cases

Examples:

- registry exists;
- recipe crafts correct output;
- weapon damage works;
- block places;
- block drops;
- behavior applies effect;
- save and reload preserves;
- previous concepts remain;
- server reaches ready;
- client loads the complete mod.

---

# 15. Runtime Verification Levels

A case may require one or more levels.

## Level 1 — Static

- schemas valid;
- references valid;
- generated JSON valid;
- required resources present;
- registry mappings complete.

## Level 2 — Build

- Gradle build succeeds;
- output JAR exists;
- packaging is valid.

## Level 3 — Dedicated server

- loader initializes;
- registries complete;
- server reaches ready state;
- no fatal exception.

## Level 4 — GameTest

- requested gameplay assertions pass.

## Level 5 — Persistence

- save and reload preserves required behavior or state.

## Level 6 — Client smoke

- client starts;
- mod loads;
- title screen or controlled test world is reached;
- no startup crash.

---

# 16. Generated Gameplay Assertions

Every mutating operation must create required verification assertions.

Examples:

## Create weapon

```text
registry_exists
property_equals durability
property_equals attack_damage
weapon_damages
```

## Add recipe

```text
recipe_exists
recipe_produces
```

## Create block

```text
registry_exists
block_places
block_drops
```

## Add behavior

```text
trigger_executes
condition_controls_action
action_has_expected_effect
```

## Delete concept

```text
registry_absent
dependent_references_valid
```

---

# 17. Failure Ownership

Every failure must be assigned to exactly one primary owner.

```text
MODEL_INTERPRETATION
CONTEXT_CONSTRUCTION
OUTPUT_PARSING
SCHEMA_VALIDATION
REFERENCE_RESOLUTION
SEMANTIC_VALIDATION
CAPABILITY_VALIDATION
STATE_TRANSITION
GENERATOR
ASSET_GENERATION
BUILD_ENVIRONMENT
MINECRAFT_ADAPTER
GAMETEST
CLIENT_LAUNCH
BENCHMARK_HARNESS
```

Secondary contributing causes may also be recorded.

A failure classified outside the model must not automatically become fine-tuning data.

---

# 18. False Success

False success is the most serious benchmark failure.

A false success occurs when AMC reports that a request worked but:

- the requested operation was omitted;
- the wrong concept was changed;
- the generated project failed verification;
- the feature does not work in Minecraft;
- only part of the request was implemented without consent;
- the system used the last verified state while presenting the candidate as successful.

The required MVP false-success rate is:

```text
0%
```

Any false-success case blocks release and model promotion.

---

# 19. Scoring

## Primary score

```text
Verified Request Completion Rate
=
fully verified requests
/
all eligible requests
```

Clarification and correctly identified unsupported cases are scored separately rather than automatically counted as failures.

## Supporting scores

```text
Intent Correctness
State Correctness
Generation Success
Build Success
Runtime Verification Success
Regression Success
Clarification Correctness
Unsupported Classification Correctness
```

## Strict completion

A mutating case passes only when all required stages pass.

Partial credit may appear in diagnostic reports but does not count as product completion.

---

# 20. Initial MVP Thresholds

Recommended release thresholds:

```text
Schema-valid model output:               >= 99%
Intent semantic accuracy:                >= 90%
Target-resolution accuracy:              >= 95%
Clarification precision:                 >= 95%
Unsupported precision:                   >= 95%
Verified end-to-end completion:          >= 85%
State preservation after failed request: 100%
Deterministic rebuild rate:              100%
Required regression pass rate:           100%
False-success rate:                      0%
Wrong-target protected-set mutations:    0
Unintended protected-set mutations:      0
```

These are MVP gates, not permanent performance ceilings.

---

# 21. Latency and Resource Metrics

The benchmark must record:

- prompt-construction time;
- model inference time;
- parsing and validation time;
- state-transition time;
- generation time;
- build time;
- server startup time;
- GameTest time;
- client startup time;
- total elapsed time;
- peak RAM;
- model memory;
- disk usage;
- retry count.

Results must separate:

```text
cold start
warm start
incremental rebuild
clean rebuild
```

Correctness takes precedence over latency.

---

# 22. Determinism Tests

The system must test determinism at three levels.

## Model determinism

Repeated constrained inference should produce semantically identical operations.

## State determinism

The same parent state and normalized operations should produce the same candidate-state hash.

## Generator determinism

The same state, generator version, target, and asset seed should produce the same generated-output hash.

Any intentionally variable metadata must be excluded from deterministic hashes.

---

# 23. Benchmark Data Separation

Datasets must be separated into:

```text
training
validation
held-out intent test
permanent regression
integration fixtures
full-loop acceptance
```

Cases must not be copied across partitions in ways that create leakage.

Near-duplicate English phrasing and equivalent state fixtures must be checked for leakage.

---

# 24. Benchmark Repository Layout

```text
benchmarks/
├── README.md
├── schemas/
│   ├── intent_case.schema.json
│   ├── e2e_case.schema.json
│   └── report.schema.json
├── intent/
│   ├── heldout/
│   ├── protected/
│   └── regression/
├── integration/
├── e2e/
│   ├── create/
│   ├── modify/
│   ├── references/
│   ├── clarification/
│   ├── unsupported/
│   ├── rollback/
│   └── runtime/
├── fixtures/
│   ├── projects/
│   └── minecraft/
└── reports/
```

---

# 25. CLI Benchmark Commands

Required commands:

```bash
amc benchmark intent benchmarks/intent/heldout
amc benchmark regression benchmarks/intent/regression
amc benchmark integration benchmarks/integration
amc benchmark e2e benchmarks/e2e
amc benchmark all
```

Required options:

```text
--model
--dataset-version
--repeat
--json
--report-dir
--case
--category
--fail-fast
--keep-artifacts
```

---

# 26. Report Format

Every benchmark run must create:

```text
summary.json
cases.jsonl
failures.jsonl
metrics.json
environment.json
artifacts/
```

## Environment record

```json
{
  "source_commit": "git-sha",
  "model_version": "functiongemma-amc-0.1.3",
  "dataset_version": "dataset-0.3.0",
  "mcl_schema_version": "0.1.0",
  "generator_version": "0.1.0",
  "minecraft_adapter_version": "neoforge-pinned-0.1.0",
  "os": "Windows",
  "cpu": "...",
  "gpu": "...",
  "ram_gb": 64,
  "java_version": "21",
  "run_started_at": "2026-07-17T12:00:00Z"
}
```

---

# 27. Per-Case Artifacts

For every failed full-loop case, preserve:

- request text;
- initial state;
- raw model output;
- parsed operations;
- normalized operations;
- validation report;
- candidate state;
- generated manifest;
- generated-output hash;
- build log;
- server log;
- GameTest report;
- client log when applicable;
- failure classification;
- screenshots only when required for manual diagnosis.

Passing cases may preserve a reduced artifact set unless the run is a release qualification.

---

# 28. Continuous Integration Schedule

## Every commit

- deterministic component tests;
- schemas;
- state transitions;
- generator snapshots;
- benchmark-runner tests.

## Every model candidate

- fine-tuning unit suite;
- protected set;
- regression suite;
- selected integration cases.

## Nightly

- larger intent suite;
- integration suite;
- server and GameTest suite;
- selected full-loop cases.

## Release candidate

- complete 50-case full-loop suite;
- clean environment build;
- model promotion checks;
- reproducibility check;
- client smoke suite;
- zero-false-success audit.

---

# 29. Fine-Tuning Iteration Workflow

The required workflow is:

```text
1. Run intent tests.
2. Identify failed cases.
3. Classify failure ownership.
4. Add regression case for model-owned failure.
5. Correct or expand training data.
6. Train candidate model.
7. Run held-out and protected tests.
8. Compare with current default model.
9. Run selected integration tests.
10. Run selected full-loop Minecraft tests.
11. Promote or reject.
```

No checkpoint may be promoted solely because the training loss improved.

---

# 30. Full-Loop Iteration Workflow

For an end-to-end failure:

```text
1. Preserve all artifacts.
2. Identify the first incorrect stage.
3. Assign failure ownership.
4. Create the narrowest reproducing test.
5. Fix the owning subsystem.
6. Run its unit tests.
7. Run affected integration tests.
8. Re-run the original full-loop case.
9. Run regression suite.
```

This prevents slow Minecraft tests from becoming the only debugging mechanism.

---

# 31. MVP Acceptance Scenario

The mandatory release scenario is:

```bash
amc init ruby_world

amc request \
  "Create a ruby sword called Ruby Blade.
   Give it 1200 durability and 9 attack damage.
   Add a crafting recipe using two rubies and one stick.
   Make the blade dark red."

amc test
amc play

amc request \
  "Make Ruby Blade slightly slower,
   increase its durability to 1500,
   and make the blade darker."

amc test
amc play

amc revert tx-000001
amc test
amc play
```

Required evidence:

- correct initial MCL operations;
- valid canonical state;
- successful deterministic generation;
- successful build;
- successful server boot;
- passing registry and recipe tests;
- successful client launch;
- correct modification;
- passing accumulated regression;
- successful verified rollback;
- no manual code editing.

---

# 32. Definition of Done

The benchmark and test system is complete for the MVP when:

1. fine-tuning experiments can be evaluated in minutes without Minecraft;
2. model regressions are automatically detected;
3. candidate models have explicit promotion gates;
4. deterministic components are tested independently;
5. the full English-to-Minecraft path is automated;
6. every full-loop failure produces traceable artifacts;
7. failure ownership is consistently classified;
8. at least 50 end-to-end cases exist;
9. failed requests preserve verified state;
10. false-success rate is zero.
