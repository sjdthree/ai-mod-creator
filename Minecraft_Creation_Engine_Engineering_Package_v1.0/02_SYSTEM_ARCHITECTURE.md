
# System Architecture

**Specification ID:** SPEC-003  
**Status:** Draft 0.1  
**Purpose:** Define the minimum architecture required to deliver and iterate on the Minecraft Creation Engine MVP.

---

# 1. Architectural Goal

The system must convert an English request into a verified Minecraft mod through a local, testable, repeatable pipeline.

The architecture must support rapid iteration on the fine-tuned intent model without coupling model changes to the generator, build system, or Minecraft runtime.

The complete path is:

```text
English request
    ↓
CLI
    ↓
Intent model
    ↓
Structured MCE operations
    ↓
Validation
    ↓
Candidate project state
    ↓
Deterministic generation
    ↓
Build
    ↓
Minecraft runtime verification
    ↓
Commit verified state
```

The same architecture must support both:

1. narrow unit testing of the fine-tuned model; and
2. full-loop testing from English input to working behavior inside Minecraft.

---

# 2. Core Components

The MVP contains nine primary components:

```text
CLI
Intent Adapter
Fine-Tuning and Evaluation System
Operation Validator
Project State Manager
Deterministic Generator
Build Manager
Verification Harness
Minecraft Launcher
```

Each component must have a narrow responsibility and a testable interface.

---

# 3. CLI

## Responsibility

The CLI is the user-facing entry point and orchestration layer.

It accepts English requests, invokes the pipeline, displays results, and exposes commands for build, test, benchmark, rollback, and launch.

## Inputs

- English user request
- project path
- command-line options
- benchmark files
- transaction identifiers

## Outputs

- human-readable status
- machine-readable JSON
- structured error codes
- log and report paths
- process exit code

## Dependencies

The CLI may call all major application services through interfaces.

## Prohibited responsibilities

The CLI must not:

- interpret Minecraft intent itself;
- mutate canonical state directly;
- generate Java or resources;
- determine gameplay correctness;
- contain model-specific logic.

---

# 4. Intent Adapter

## Responsibility

The Intent Adapter converts English into structured MCE operations.

It is the only component allowed to interpret natural-language intent.

## Inputs

- English request
- compact project context
- supported operations
- supported concept types
- aliases and references
- relevant schema constraints

## Outputs

```json
{
  "request_id": "req-001",
  "operations": [],
  "needs_clarification": false,
  "unsupported_reason": null
}
```

## Dependencies

- model runtime
- prompt and schema definitions
- context builder
- output parser
- constrained decoding or schema validation

## Prohibited responsibilities

The Intent Adapter must not:

- modify canonical project state;
- generate Java;
- repair generated code;
- run Gradle;
- decide whether Minecraft behavior works;
- silently remove unsupported intent.

---

# 5. Fine-Tuning and Evaluation System

## Responsibility

This subsystem owns the complete lifecycle of the local intent model:

- dataset generation;
- dataset validation;
- fine-tuning;
- checkpoint versioning;
- quantization;
- unit evaluation;
- regression evaluation;
- model promotion;
- rollback;
- experiment reporting.

This is a first-class part of the MVP because the model will require repeated iteration.

## Inputs

- MCE operation schemas
- supported concept definitions
- project-state fixtures
- English request examples
- expected operations
- clarification labels
- unsupported-request labels
- benchmark datasets
- previous model results

## Outputs

- versioned training datasets
- versioned model checkpoints
- quantized runtime artifacts
- evaluation reports
- confusion analyses
- regression reports
- promoted model manifest

## Required internal modules

```text
Dataset Generator
Dataset Validator
Training Runner
Checkpoint Registry
Quantization Pipeline
Intent Unit Test Runner
Regression Test Runner
Experiment Reporter
Model Promotion Gate
```

## Dataset requirements

Each training example must include:

- unique case ID;
- initial project state;
- English request;
- expected operations;
- expected target references;
- clarification expectation;
- unsupported expectation;
- expected mutation status;
- expected resulting-state assertions where applicable.

## Model versioning

Every model artifact must record:

- base model;
- dataset version;
- schema version;
- training configuration;
- quantization configuration;
- source commit;
- evaluation results;
- promotion status.

## Promotion rule

A candidate model may become the default only if it passes:

1. intent unit-test thresholds;
2. held-out evaluation thresholds;
3. regression thresholds;
4. no-false-mutation checks;
5. required end-to-end smoke cases.

## Prohibited responsibilities

The fine-tuning system must not:

- modify production project state;
- patch generator defects;
- use end-to-end failures to hide model errors;
- promote a model based only on aggregate accuracy.

---

# 6. Fine-Tuning Unit Tests

Fine-tuning unit tests evaluate the English-to-operation boundary without running Minecraft.

These tests must be fast enough to run after every training experiment.

## Test categories

### Operation selection

Verify that the correct operation type is selected.

Example:

```text
"Create a ruby sword"
→ create_concept
```

### Argument extraction

Verify exact values and normalized units.

Example:

```text
"Give it 1200 durability"
→ durability = 1200
```

### Reference resolution

Verify references such as:

```text
"the sword"
"Ruby Blade"
"the last item"
"the one we made earlier"
```

### Multi-operation decomposition

Verify that one request can produce several operations.

Example:

```text
"Create a sword and add a recipe"
→ create_concept + add_recipe
```

### Clarification

Verify that ambiguous requests do not mutate state.

Example:

```text
"Make it better"
→ clarification required
```

### Unsupported classification

Verify that unsupported requests are identified precisely.

### Non-mutating questions

Verify that informational questions do not produce state changes.

### Typographical robustness

Verify common spelling, grammar, punctuation, and child-like phrasing.

### Schema compliance

Verify that every emitted operation conforms to the actual operation schema.

### Determinism

Verify that temperature-zero or constrained inference produces stable output for the same input and context.

## Required unit-test metrics

- exact operation accuracy;
- field-level argument accuracy;
- target-resolution accuracy;
- clarification precision and recall;
- unsupported precision and recall;
- schema-valid output rate;
- unintended-mutation rate;
- exact-match rate;
- deterministic-repeat rate.

## Critical thresholds

The specific values may evolve, but the following rules are mandatory:

- schema-valid output rate must approach 100%;
- unintended mutation must be 0% on the protected test set;
- clarification and unsupported failures must be reported separately;
- aggregate accuracy must never conceal catastrophic cases.

---

# 7. Operation Validator

## Responsibility

The validator determines whether proposed operations are valid, coherent, and implementable.

## Inputs

- current verified state
- proposed operations
- schemas
- Minecraft capability definitions
- generator capability definitions

## Outputs

- validation report
- normalized operations
- clarification requirement
- unsupported capability report

## Validation layers

```text
JSON syntax
Schema
References
Properties
Semantics
Implementation capability
```

## Prohibited responsibilities

The validator must not:

- infer user intent;
- generate source code;
- silently correct material intent;
- commit state.

---

# 8. Project State Manager

## Responsibility

The state manager owns canonical project state, candidate transactions, history, rollback, IDs, references, and commit behavior.

## Inputs

- verified current state
- validated operations
- verification result
- transaction metadata

## Outputs

- candidate state
- committed state
- transaction ID
- state hash
- rollback state

## Transaction rule

A candidate state becomes canonical only after required verification succeeds.

## Prohibited responsibilities

The state manager must not:

- interpret English;
- generate implementation;
- run Minecraft;
- mark unverified state as verified.

---

# 9. Deterministic Generator

## Responsibility

The generator converts canonical state into Java, resources, tests, and placeholder assets.

## Inputs

- canonical or candidate project state
- pinned Minecraft target
- generator version
- asset seed

## Outputs

- generated source
- generated resources
- generated tests
- generation manifest
- output hashes

## Prohibited responsibilities

The generator must not:

- call the language model;
- interpret English;
- mutate canonical state;
- repair itself through free-form model output.

---

# 10. Build Manager

## Responsibility

The Build Manager executes the pinned Gradle build and classifies build failures.

## Inputs

- generated Minecraft project
- pinned JDK
- Gradle wrapper
- build configuration

## Outputs

- build result
- logs
- output JAR
- artifact checksum
- classified failure

## Prohibited responsibilities

The Build Manager must not:

- edit generated files;
- reinterpret user intent;
- report gameplay success.

---

# 11. Verification Harness

## Responsibility

The Verification Harness determines whether the requested behavior works.

## Verification levels

```text
Static validation
Build
Dedicated-server boot
Registry verification
Generated GameTests
Persistence checks
Client smoke test
```

## Inputs

- candidate state
- generated project
- generated requirements
- Minecraft logs
- test results

## Outputs

- structured verification report
- pass or fail
- failed requirement IDs
- failure ownership classification

## Prohibited responsibilities

The harness must not:

- alter requested behavior;
- commit state;
- treat compilation alone as success.

---

# 12. Minecraft Launcher

## Responsibility

Launch the pinned development client after required verification gates pass.

## Inputs

- verified project
- generated mod
- runtime configuration

## Outputs

- process status
- client logs
- startup result

## Prohibited responsibilities

The launcher must not:

- validate language-model output;
- generate source;
- modify canonical state.

---

# 13. Full-Loop Tests

Full-loop tests exercise the entire product path:

```text
English request
    ↓
CLI
    ↓
Intent model
    ↓
Operations
    ↓
Validation
    ↓
Candidate state
    ↓
Generation
    ↓
Build
    ↓
Minecraft
    ↓
Gameplay assertion
```

These tests are slower and must run on selected commits, release candidates, and model-promotion candidates.

## Required full-loop categories

### Create

English creates a new supported concept that works in Minecraft.

### Modify

English changes an existing concept and the changed behavior is verified.

### Multi-step

English creates a concept plus recipe, loot, appearance, or behavior.

### Clarification

Ambiguous English does not alter state and returns a clear question.

### Unsupported

Unsupported English does not alter state and returns a precise capability response.

### Failure preservation

A failed build or failed GameTest leaves the prior verified state intact.

### Rollback

A prior verified state can be restored and rebuilt.

### Regression

All previously accepted behaviors continue to pass after a new request.

## Minimum MVP full-loop suite

The first release must include at least:

- 10 create cases;
- 10 modify cases;
- 5 reference-resolution cases;
- 5 clarification cases;
- 5 unsupported cases;
- 5 failure-preservation and rollback cases;
- 10 runtime and regression cases.

Total minimum: 50 end-to-end cases.

---

# 14. Test Pyramid

The architecture uses four test layers.

## Layer 1: deterministic component tests

Fast tests for:

- schemas;
- validators;
- state transitions;
- generator outputs;
- build-command construction;
- report parsing.

Run on every commit.

## Layer 2: fine-tuning unit tests

Fast model tests for English-to-operation behavior.

Run for every candidate checkpoint and relevant code change.

## Layer 3: integration tests

Tests across:

- intent adapter plus validator;
- validator plus state;
- state plus generator;
- generator plus build;
- build plus verification.

Run in CI where environment permits.

## Layer 4: full-loop Minecraft tests

English to verified gameplay.

Run on release branches, model-promotion candidates, nightly builds, and before MVP acceptance.

---

# 15. Test Artifacts and Traceability

Every test case must have a stable ID.

Example:

```text
FT-CREATE-001
FT-REF-014
E2E-WEAPON-003
E2E-ROLLBACK-002
```

Each case should trace to:

- product requirement;
- supported operation;
- schema version;
- model version;
- generator version;
- Minecraft adapter version;
- expected state assertion;
- expected runtime assertion.

Every end-to-end result must preserve:

- input request;
- initial state hash;
- emitted operations;
- normalized operations;
- resulting candidate-state hash;
- generated-output hash;
- build log;
- Minecraft log;
- test report;
- final disposition.

---

# 16. Model Iteration Loop

The fine-tuning loop is:

```text
Collect failures
    ↓
Classify failure ownership
    ↓
Add or correct test case
    ↓
Update training or validation data
    ↓
Train candidate
    ↓
Run intent unit tests
    ↓
Run regression tests
    ↓
Run selected full-loop Minecraft tests
    ↓
Promote or reject candidate
```

A failed end-to-end case must first be classified as one of:

- model interpretation error;
- context construction error;
- schema or validator error;
- state transition error;
- generator defect;
- build environment failure;
- Minecraft adapter defect;
- verification defect.

Only model-owned failures should be added to the fine-tuning queue.

---

# 17. Model Promotion Gates

A candidate model is promoted only when:

1. all protected no-mutation tests pass;
2. schema-valid output meets the required threshold;
3. held-out intent metrics meet or exceed the current production model;
4. no critical regression is introduced;
5. selected full-loop Minecraft cases pass;
6. memory and latency stay within the desktop target;
7. all artifacts are versioned and reproducible.

A candidate with higher aggregate accuracy but a critical regression must be rejected.

---

# 18. MVP Acceptance Path

The MVP is accepted only when the following automated scenario passes:

```bash
mce init ruby_world

mce request   "Create a ruby sword called Ruby Blade.
   Give it 1200 durability and make it stronger than a diamond sword.
   Add a crafting recipe using two rubies and one stick."

mce test
mce play

mce request   "Make Ruby Blade slightly slower,
   increase durability to 1500,
   and make the blade dark red."

mce test
mce play
```

Required assertions:

- the first request produces valid operations;
- the initial sword exists in canonical state;
- generated code and resources build;
- the sword is registered;
- the recipe works;
- the client launches;
- the second request resolves the existing sword;
- durability becomes 1500;
- speed changes in the intended direction;
- appearance changes;
- all previous tests still pass;
- no manual code editing occurs.

---

# 19. Implementation Order

1. Define schemas and operation fixtures.
2. Build canonical state and validator.
3. Generate one weapon deterministically.
4. Build and launch the mod.
5. Add GameTest verification.
6. Add modification and rollback.
7. Build fine-tuning unit-test harness.
8. Add deterministic English adapter.
9. Integrate the local model.
10. Create the first training dataset.
11. Train and evaluate candidate checkpoints.
12. Add model-promotion gates.
13. Run the complete English-to-Minecraft suite.
14. Iterate until MVP acceptance thresholds are met.

---

# 20. Architectural Success Criteria

The architecture is successful when:

- model checkpoints can be replaced without changing generator code;
- generator code can be improved without retraining the model unless the operation contract changes;
- every model experiment can be tested independently;
- every end-to-end failure can be assigned to an owning component;
- an English request can be traced through every pipeline artifact;
- a candidate model cannot be promoted without unit and full-loop evidence;
- no unverified state is committed as successful.
