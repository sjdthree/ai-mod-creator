# Implementation Roadmap

**Specification ID:** SPEC-006
**Status:** Draft 0.1

## Purpose

This document defines the engineering execution plan for delivering the AI-mod creator MVP. It translates the previous specifications into an implementation sequence with clear milestones, dependencies, deliverables, and acceptance gates.

---

# Guiding Principles

1. Build the smallest end-to-end system first.
2. Keep every component independently testable.
3. Add capabilities only after the previous vertical slice is verified.
4. Fine-tuning and deterministic generation evolve together through benchmarks.
5. Never optimize before the complete English-to-Minecraft loop works.

---

# Phase 0 — Foundation (Week 1)

Deliverables:

- Repository structure
- CI pipeline
- Rust workspace
- Schema infrastructure
- JSON validation
- Logging
- Benchmark skeleton
- Sample fixtures

Acceptance:

- Clean checkout builds
- Unit tests execute
- Benchmark runner executes empty suite

---

# Phase 1 — Canonical State (Weeks 2–3)

Implement:

- MCL schemas
- State manager
- Transactions
- Rollback
- Reference resolution
- Validation engine

Acceptance:

- Create/update/delete operations succeed
- Rollback restores prior verified state
- State hashes are deterministic

Dependencies:

- Phase 0

---

# Phase 2 — Deterministic Generator (Weeks 4–5)

Implement:

- Weapon generation
- Item generation
- Recipe generation
- Placeholder textures
- Resource generation
- Java generation

Acceptance:

- Ruby Blade project generates reproducibly
- Output hashes are stable

Dependencies:

- Phase 1

---

# Phase 3 — Build & Verification (Weeks 6–7)

Implement:

- Gradle orchestration
- Server launch
- Registry verification
- Generated GameTests
- Client smoke test

Acceptance:

- Generated mod builds
- Minecraft launches
- Ruby Blade can be crafted and used

Dependencies:

- Phase 2

---

# Phase 4 — Intent Model (Weeks 8–9)

Implement:

- FunctionGemma integration
- Prompting
- Structured decoding
- Dataset loader
- Fine-tuning pipeline
- Evaluation runner

Acceptance:

- English request produces valid MCL operations
- Unit-test suite executes
- Protected set passes promotion gates

Dependencies:

- Phase 1

---

# Phase 5 — End-to-End Loop (Weeks 10–11)

Integrate:

CLI → Intent → Validator → State → Generator → Build → Verification → Minecraft

Acceptance:

The following command sequence succeeds without manual edits:

```bash
amc init ruby_world

amc request "Create a ruby sword called Ruby Blade with 1200 durability."

amc test

amc play
```

Dependencies:

- Phases 3 and 4

---

# Phase 6 — MVP Hardening (Week 12)

Complete:

- 50 end-to-end benchmark cases
- Regression suite
- Documentation
- Packaging
- Release candidate

Acceptance:

- Benchmark thresholds met
- False-success rate = 0%
- Release checklist complete

---

# Team Responsibilities

## Core Engine

Owns:

- State manager
- Validator
- Transactions
- Generator

## AI/ML

Owns:

- Datasets
- Fine-tuning
- Intent evaluation
- Model promotion

## Minecraft Integration

Owns:

- Build pipeline
- Adapter
- GameTests
- Runtime verification

## QA / Benchmark

Owns:

- Benchmark corpus
- Regression suite
- Release qualification

---

# Release Gates

Every release candidate must satisfy:

- All unit tests pass
- Integration tests pass
- Full-loop suite passes
- Protected fine-tuning set passes
- No benchmark regression
- No false success
- Reproducible build artifacts

---

# Risks

| Risk | Mitigation |
|------|------------|
| Model hallucination | Strict schemas, validator, protected tests |
| Generator complexity | Deterministic templates and snapshot tests |
| Minecraft API changes | Pin one loader/version for MVP |
| Slow iteration | Separate model unit tests from runtime tests |
| Scope creep | Defer unsupported concepts until post-MVP |

---

# Deferred After MVP

- Entities
- Dimensions
- World generation
- GUIs
- Multiplayer
- Multiple loaders
- Multiple Minecraft versions
- Asset-quality improvements
- Cloud inference

---

# Success Criteria

The MVP is complete when a new user can:

1. Install AMC.
2. Create a project.
3. Describe a supported mod feature in English.
4. Watch AMC build and verify it.
5. Launch Minecraft.
6. Modify the feature using another English request.
7. Repeat without writing code.

This is the first production milestone. Future releases expand supported gameplay while preserving the same English-to-MCL-to-Minecraft architecture.


---

# Living Engineering Backlog

The six specification documents are intended to remain relatively stable. Day-to-day engineering work should be managed through a separate **living engineering backlog**, derived from these specifications rather than modifying them directly.

## Repository Layout

```text
implementation/
├── phase0/
├── phase1/
├── phase2/
├── phase3/
├── phase4/
├── phase5/
└── phase6/
```

Each phase contains small implementation work items that can be assigned independently.

Example:

```text
implementation/
└── phase1/
    ├── 0001-json-schema.md
    ├── 0002-reference-resolution.md
    ├── 0003-state-manager.md
    ├── 0004-transactions.md
    └── 0005-rollback.md
```

## Standard Work Item Template

Every engineering work item should contain:

- **Objective** — What capability is being implemented.
- **Background** — Relevant specification references (SPEC and section numbers).
- **Dependencies** — Required completed work items.
- **Implementation Notes** — Constraints and interfaces.
- **Acceptance Tests** — Exact conditions required for completion.
- **Benchmark Coverage** — Unit, integration, or end-to-end cases that must pass.
- **Estimated Effort** — Small (≤1 day), Medium (2–3 days), or Large (≤1 week).
- **Owner** — Assigned engineer.

## Work Item Size

Work items should generally require no more than one engineer-week. Large features should be decomposed into independently testable tasks.

## Traceability

Every work item should reference:

- Product requirement(s)
- MCL schema elements
- Benchmark case IDs
- Relevant component(s)

Conversely, completed benchmark cases and code changes should reference the originating work item.

## Definition of Complete

A work item is complete only when:

1. The implementation is merged.
2. Required unit tests pass.
3. Required integration tests pass.
4. Required benchmark cases pass.
5. Documentation is updated if interfaces changed.
6. CI passes without regression.

This backlog is expected to evolve throughout development, while the six core specification documents remain the stable engineering contract for the MVP.
