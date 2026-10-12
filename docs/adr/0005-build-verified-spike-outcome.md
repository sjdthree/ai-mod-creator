# ADR-0005 — Phase 3 spike outcome: generated mod builds against NeoForge

- **Status:** Accepted
- **Date:** 2026-07-17
- **Partially supersedes:** ADR-0004 (the build/toolchain half is now proven; GameTest + headless
  client remain open)

## Context

ADR-0004 flagged runtime verification as the project's top risk and called for a spike before
committing to the harness. The Phase 3 spike (`implementation/phase3/0000`) has now run the first
half: does a generator-emitted project actually compile against NeoForge and package a mod JAR?

## What was done

Inside the isolated dev container (PR #7 — JDK 21 / Gradle 8.10.2 / no host impact):

1. Generated the full Ruby Blade project (ruby ingredient + ruby sword + shaped recipe) from
   canonical state via `mce-generator` (`cargo run -p mce-generator --example ruby_world`).
2. Ran `gradle --no-daemon build` with the ModDevGradle plugin.

## Outcome — success

- Pinned versions confirmed from the official `MDK-1.21.1-ModDevGradle` template and applied to
  the generator: **ModDevGradle `2.0.141`, NeoForge `21.1.235`, Java 21, `loaderVersion=[1,)`**.
- ModDevGradle resolved, decompiled Minecraft, and compiled NeoForge's sources; then
  `> Task :compileJava` compiled **our** generated `MceRubyWorld.java` + `ModItems.java` with no
  errors. `BUILD SUCCESSFUL`.
- The JAR `build/libs/mce_ruby_world-0.1.0.jar` contains the compiled classes plus every generated
  resource: recipe JSON, item models, `en_us.json`, textures, and `neoforge.mods.toml`.

This proves the generator's `SwordItem` / `SimpleTier` / `DeferredRegister` templates and resource
layout are valid for NeoForge 1.21.1. **Build & packaging (SPEC-005 §15 levels 1–2) are
de-risked.**

## Still open (tracked under ADR-0004 / phase3 `0002`–`0005`, `0000`)

- Dedicated-server boot + registry verification (L3).
- GameTest discovery in production (may need manual registration) (L4).
- Persistence (L5) and headless client smoke under Xvfb (L6).

## Consequences

- The generator's version pins and templates are validated; keep them in lockstep with the MDK.
- Next: wire `mce-build` to run the wrapper on a generated project (needs the wrapper jar
  materialized — the generator emits `gradle-wrapper.properties`; add a `gradle wrapper` step or
  point the build manager at the system Gradle in the container), then implement the runtime
  `LevelRunner`s and rerun the spike for L3–L6.
