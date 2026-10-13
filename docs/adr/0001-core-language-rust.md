# ADR-0001 — Rust workspace for the core engine

- **Status:** Accepted
- **Date:** 2026-07-17

## Context

AMC must convert English into a **verified**, **deterministic**, **reproducible** Minecraft mod
on a consumer desktop, offline after install (`SPEC-001` NFR-001, NFR-004; `SPEC-004` MCL-004).
The core carries a strongly-typed state/schema contract where correctness and determinism
dominate. `SPEC-006` Phase 0 already specifies a Rust workspace.

## Decision

Implement the core engine as a **Cargo workspace (Rust, edition 2021)**: `amc-cli`,
`amc-core`, `amc-schema`, `amc-state`, `amc-validator`, `amc-generator`, `amc-build`,
`amc-verify`, `amc-launch`, `amc-intent`, `amc-bench`.

## Consequences

- **+** Strong types make the MCL/state contract hard to violate; determinism is easy to test.
- **+** Single static binary suits offline desktop distribution.
- **+** `serde` + `jsonschema` cover the JSON-heavy MCL cleanly.
- **−** The ML ecosystem is Python; the model boundary needs an explicit bridge (see ADR-0002).
- **−** Minecraft/Gradle are JVM; the generator emits Java and shells out to Gradle.
