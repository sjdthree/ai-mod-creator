---
name: Bug
about: A defect in the engine, generator, build, or verification path
title: "[bug] <summary>"
labels: ["bug"]
---

## What happened

## Expected

## Reproduction

<!-- Prefer the narrowest reproducing test (SPEC-005 §30). Include request text, initial
     state fixture, and the first incorrect pipeline stage if known. -->

## Failure ownership (SPEC-005 §17)

Primary owner: <!-- MODEL_INTERPRETATION | SCHEMA_VALIDATION | STATE_TRANSITION | GENERATOR |
BUILD_ENVIRONMENT | MINECRAFT_ADAPTER | GAMETEST | CLIENT_LAUNCH | BENCHMARK_HARNESS | ... -->

## Artifacts

<!-- For full-loop failures, attach/link the SPEC-005 §27 artifact set. -->

## False-success? (SPEC-005 §18)

- [ ] This bug caused MCE to report success for unverified behavior (release-blocking)
