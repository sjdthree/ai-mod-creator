# Fabric loader port — assessment

*Captured 2026-07-22. The engineering assessment behind [ADR-0007](../adr/0007-multi-loader-fabric-target.md).*

## Summary

Emitting a **Fabric** mod instead of (or alongside) **NeoForge** is an **additive capability, not a
rewrite**. The pipeline `state → generate → build → verify → launch` has clean crate boundaries, and
only the *generate* step is deeply loader-specific. Roughly **half** of the generated output is
vanilla data-pack format that ports for free. Realistic effort for feature parity: **~2 weeks** for a
modder-competent dev, most of it in re-authoring ~6 Java templates + 4 Gradle/metadata templates
against fabric-api and getting Loom's GameTest run config to emit a clean pass/fail signal.

## How the loader is wired in today

| Crate | Role | Loader coupling |
|---|---|---|
| `amc-state` / `amc-validator` | canonical model, transactions | **none** — pure domain |
| `amc-generator` | state → buildable mod project | **deep** — NeoForge lives here |
| `amc-build` | `./gradlew build` + log classification | thin — invokes Gradle, classifies output |
| `amc-verify` | `./gradlew runGameTestServer` | **task-name coupled** |
| `amc-launch` | `./gradlew runClient` + readiness markers | mostly generic |
| `amc-cli` | orchestration | constructed concrete `NeoForgeGenerator` (4 call sites) |

A `Generator` trait already existed (`generator/src/lib.rs`); the CLI bypassed it and called the
concrete `NeoForgeGenerator` at `main.rs` lines 478, 562, 633, 692. The abstraction seam existed; it
just wasn't used polymorphically.

## What ports unchanged (~half of `build_files`)

All vanilla / data-pack format — identical between loaders:

- Resources: `blockstate_json`, `block_model_json`, `item_model_json`, `block_item_model_json`,
  `block_loot_table_json`, `lang_json`
- Recipes: `recipe_json` (pure `minecraft:crafting_shaped` etc.)
- Textures (`texture.rs`) and the GameTest structure NBT (`structure.rs`)
- The orchestration skeleton: `build_files` layout, `prune_stale`, manifest/hashing, all concept
  helpers, and the committed Gradle wrapper (`gradlew`, `gradle-wrapper.jar`, wrapper properties)

## What must be rewritten for Fabric

| NeoForge today | Fabric equivalent |
|---|---|
| `META-INF/neoforge.mods.toml` | `fabric.mod.json` (different schema, entrypoints) |
| `build.gradle` w/ `net.neoforged.moddev` + `neoForge { runs { gameTestServer } }` | `fabric-loom` plugin + loom run configs |
| `@Mod` + `IEventBus` constructor | `ModInitializer.onInitialize()` (+ `ClientModInitializer`) |
| `DeferredRegister` / `DeferredItem` / `DeferredBlock` | `Registry.register(Registries.ITEM, id, …)` — no deferred registers |
| `BuildCreativeModeTabContentsEvent` | `ItemGroupEvents.modifyEntriesEvent(…)` |
| `SimpleTier`, `FMLEnvironment.production` | Fabric tool material handling, `FabricLoader.isDevelopmentEnvironment()` |
| `@GameTestHolder` / `net.neoforged.neoforge.gametest.*` | fabric-api `FabricGameTest` + `fabric-gametest` entrypoint |

The `net.minecraft.*` game classes (`SwordItem`, `Block`, `BlockBehaviour`, `FoodProperties`,
`CreativeModeTabs`) are **the same** — *provided Loom is configured for Mojang mappings (Mojmap)*.
Pick Mojmap and most of the item/block construction logic survives with only registration-API swaps;
pick Yarn and every game-class name changes too. **Decision: Mojmap** (see ADR-0007).

## Downstream changes (smaller, real)

1. **`amc-verify/gametest.rs`** hardcodes the `runGameTestServer` task. Fabric Loom exposes GameTest
   through a different run config (`runGametest`-style) that must be declared in the generated
   `build.gradle`. Task name + the "required tests passed" log parser both need Fabric verification.
2. **`amc-build`** — `classify()` is fine; `discover_jar()` needs a Fabric-aware tweak (Loom writes a
   remapped jar plus `-dev`/`-sources` siblings under `build/libs/`).
3. **`amc-launch`** — `READY_MARKERS` are generic Minecraft log lines; should work, needs one real
   run to confirm. Wrapper assets are loader-agnostic and reused.

## Effort

- **Architecture (small, ~1 day):** use the `Generator` trait in the CLI; add `--loader`; factor
  `build_files` so the shared vanilla half is common and only Java + Gradle + metadata differ; thread
  a loader id into `amc-verify` for the right Gradle task. *(This is what step (b) below does.)*
- **The Fabric generator (bulk, ~1–1.5 weeks):** rewrite the Java + Gradle + metadata templates
  against fabric-api, then the iterate-until-it-builds-and-GameTests-pass loop. Loom's GameTest run
  config emitting a clean pass/fail signal is the riskiest single item — budget real time there.
- **Total realistic estimate: ~2 weeks** for parity (item/weapon/tool/food/block + recipes +
  creative tabs + GameTests), including a genuine end-to-end `amc request` on Fabric verified by
  GameTest and a live client.

## Sinytra Connector

Not a shortcut here. Connector lets *NeoForge* load *Fabric* mods at runtime — the opposite direction
from generating a Fabric artifact. Irrelevant to emitting Fabric.

## Recommendation

Do the trait-seam refactor first (low risk, valuable regardless), then build `FabricGenerator` as a
peer of `NeoForgeGenerator` sharing the vanilla-data half. The clean crate boundaries and existing
`Generator` trait make this additive — the state/validator/CLI-orchestration layers don't move.

## Status of this branch (`feat/fabric-loader-support`)

- **(a)** ADR-0007 + this assessment — **done**.
- **(b)** Loader seam: `Loader` enum, `generator_for()` factory, `shared`/`neoforge`/`fabric`
  generator modules, `--loader` CLI flag, loader-aware verify task — **done**; NeoForge output
  byte-identical.
- **(c)** `FabricGenerator` scaffold emitting `fabric.mod.json`, a Loom `build.gradle`, and Fabric
  Java for all supported concept types — **done as a scaffold**; not yet CI-proven against a real
  Loom build (requires JDK 21 + Loom toolchain). Promotion to "supported" is gated on that run.
