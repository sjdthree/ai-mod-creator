# ADR-0003 — Pin Minecraft 1.21.1 / NeoForge 21.1.x / Java 21

- **Status:** Accepted
- **Date:** 2026-07-17

## Context

`SPEC-006` mitigates "Minecraft API changes" by pinning **one loader/version** for the MVP.
`SPEC-004 §7` already sets `java_version: 21`. In 2026 NeoForge moved to a calendar-versioned
line (26.1, targeting Java 25), while the 1.21.x line (NeoForge 21.x) remains the mature,
widely-documented target with the strongest GameTest and headless-CI tooling — the parts of
this project with the most integration risk.

## Decision

Pin the generated target to **Minecraft 1.21.1**, **NeoForge 21.1.x (latest stable in that
line)**, **Java 21 (Temurin)**, with a committed **Gradle wrapper**. The `minecraft_target` in
project documents (`SPEC-004 §7`) resolves to these concrete values.

## Consequences

- **+** Mature, well-documented runtime; best chance of reliable automated verification.
- **+** Matches the Java 21 assumption already in the MCL spec.
- **−** Not the newest Minecraft; upgrading is deferred post-MVP (`SPEC-006` Deferred).
- **Action:** install JDK 21 (current dev machine has 17) before Phase 3.

## Notes

Exact NeoForge 21.1.x patch and Gradle version are locked in the Phase 3 build work item and
recorded in the generator manifest (`SPEC-004 §31` `adapter_version`).
