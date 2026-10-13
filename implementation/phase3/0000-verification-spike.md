# 0000 — Spike: NeoForge GameTest + headless verification

- **Phase:** 3
- **Workstream:** mc-integration
- **Effort:** Medium (timeboxed)
- **Owner:** unassigned
- **Status:** build verified ✅ (levels 1–2); runtime levels 3–6 still open. See ADR-0005.

## Objective

De-risk the single most important assumption of the project before building the harness: that we
can run automated gameplay verification in the pinned runtime and report pass/fail by exit code.

## Background

ADR-0004. `SPEC-005 §12`, §15. This is the crux of the 0%-false-success guarantee.

## Prerequisites (environment)

- **JDK 21 (Temurin)** — current dev box has Java 17.
- Network access to `maven.neoforged.net` for the first Gradle sync.
- For client smoke: `HeadlessMC` + `Xvfb` (Linux CI); not straightforward on macOS.

## Tasks

1. Generate the Ruby Blade project (done — `amc-generator`), add the Gradle wrapper, and run
   `./gradlew build` to confirm it compiles against NeoForge 21.1.x.
2. Register a trivial GameTest and run it on a dedicated server; confirm discovery works in
   production (community reports say it may not — have an RCON-driven server-assertion mod as a
   fallback).
3. Stand up a headless client boot under Xvfb that loads the mod and exits with a status code.

## Acceptance

- `./gradlew build` produces a mod JAR.
- A registered GameTest reports pass/fail via process exit code.
- A headless client boots, loads the mod, and exits cleanly.

## Outcome

Record findings in a follow-up ADR that supersedes ADR-0004 and fixes the harness mechanism.

## Traceability

- Product requirement(s): `FR-005`, acceptance `SPEC-001 §8`.
- Component(s): `amc-build`, `amc-verify`, `amc-launch`.
