# ADR-0007 — Support a second mod loader (Fabric) behind a loader-agnostic generator seam

- **Status:** Proposed
- **Date:** 2026-07-22
- **Supersedes:** none (extends ADR-0003)

## Context

ADR-0003 pinned a single loader/version — **Minecraft 1.21.1 / NeoForge 21.1.x / Java 21** — to
bound the "Minecraft API changes" risk (`SPEC-006`) for the MVP. That pin is still correct as the
*default* target. But NeoForge and Fabric split the modern-modding ecosystem, and a large class of
users (QoL / performance / "vanilla-plus" packs, lower-spec machines, players who want the newest
minor patch fastest) live on **Fabric**. We want MCE to be able to emit a Fabric mod from the same
canonical state, without forking the pipeline.

A review of the generator (`crates/mce-generator`) found the coupling is **shallower than it
looks**. The pipeline is `state → generate → build → verify → launch`, and only the *generate* step
is deeply loader-specific:

- **Loader-agnostic today (ports for free, ~half of the emitted files):** all data-pack resources —
  item/block models, blockstates, loot tables, lang, recipes — plus placeholder textures, the shared
  GameTest structure NBT, and the committed Gradle wrapper. These are vanilla formats identical
  across loaders.
- **Loader-specific (must be authored per loader):** the mod metadata file
  (`META-INF/neoforge.mods.toml` vs `fabric.mod.json`), the Gradle build (`net.neoforged.moddev` vs
  `fabric-loom`), and every generated Java class (`@Mod`/`DeferredRegister`/event-bus vs
  `ModInitializer`/`Registry.register`/`ItemGroupEvents`).
- **Downstream, task-name coupling only:** `mce-verify` hardcodes the `runGameTestServer` Gradle
  task; `mce-build`/`mce-launch` invoke generic Gradle tasks (`build`, `runClient`) and parse mostly
  loader-neutral logs.

A `Generator` trait already exists (`generator/src/lib.rs`); the CLI simply constructed the concrete
`NeoForgeGenerator` directly at four call sites.

### On Sinytra Connector

Connector lets a **NeoForge** runtime load **Fabric** mods; it does not help us *emit* a Fabric
artifact (the direction we need), so it is not a shortcut for this work. Noted here to preempt the
question.

### Mappings choice

Fabric projects can use **Yarn** or **Mojang official mappings (Mojmap)**. We choose **Mojmap via
`loom.officialMojangMappings()`** so the `net.minecraft.*` game-class names (`SwordItem`, `Block`,
`BlockBehaviour`, `FoodProperties`, `CreativeModeTabs`, `DiggerItem`, …) match the ones the NeoForge
generator already emits. This lets the item/block/tool/food *construction* logic and helper tables
(`tool_class`, `vanilla_tab`, `shape_for`, …) be shared verbatim; only the registration API and
metadata differ.

## Decision

1. **Introduce a `Loader` enum** (`NeoForge`, `Fabric`) in `mce-generator` and select a generator
   through a `generator_for(loader)` factory returning `Box<dyn Generator>`. NeoForge remains the
   default; its emitted output stays **byte-identical** (the determinism guarantee of `SPEC-005 §22`
   is preserved and asserted by existing tests).
2. **Refactor `build_files` around a shared seam.** A `shared` module produces every loader-agnostic
   file (resources, recipes, textures, structure, Gradle wrapper) plus the write/prune/manifest
   `materialize` step. Each loader module (`neoforge`, `fabric`) starts from the shared set and adds
   only its metadata + Gradle build + Java.
3. **Thread the loader through the CLI and verify step.** A global `--loader` flag (default
   `neoforge`) selects the generator; `Loader::gametest_task()` supplies the correct Gradle task name
   to `mce-verify` so Fabric's GameTest run config is invoked correctly.
4. **Add a Fabric target pin** alongside ADR-0003: **Minecraft 1.21.1 / Fabric Loader + Fabric API /
   `fabric-loom` / Java 21 / Mojmap**. Exact versions are locked in the Fabric generator constants
   and recorded in the generation manifest's target field (`fabric-1.21.1`).

## Consequences

- **+** Multi-loader becomes an **additive capability, not a fork**: the state, validator, and CLI
  orchestration layers do not move; the shared half of generation is written once.
- **+** The `Generator` trait finally earns its keep; adding a third target later (e.g. a future
  NeoForge calendar line) is a new module, not a rewrite.
- **+** NeoForge output is unchanged, so no regression to the verified MVP path.
- **−** A second Java surface to maintain: Minecraft API churn now costs two edits where it touched
  one. Mitigated by Mojmap keeping game-class names identical, so most churn is registration-API
  only.
- **−** Fabric's end-to-end path (real Loom build + GameTest pass + live client) must be verified on
  a machine with the toolchain before the Fabric target is promoted from **scaffold** to
  **supported**. **Update (2026-07-22):** the real Loom build (JDK 21) and `runGametest` **both
  pass** (6/6 GameTests) — three NeoForge→vanilla divergences were found and fixed (int sword bonus;
  public gametest-entrypoint ctor; namespace-qualified GameTest template). Remaining for full
  promotion: client smoke (`runClient`) and a CI matrix job.
- **Action:** stand up a Fabric build in CI (JDK 21 + Loom) and confirm `runGametest` (or the
  declared run config) yields the same pass/fail signal the harness parses for NeoForge.

## Notes

Rollout is staged: (a) this ADR, (b) the loader seam + `--loader` flag with NeoForge unchanged,
(c) the `FabricGenerator` scaffold. Feature parity target for promotion: `item`, `weapon`, `tool`,
`food`, `block`, plus recipes, creative-tab placement, and GameTests — the same scope NeoForge
supports today.
