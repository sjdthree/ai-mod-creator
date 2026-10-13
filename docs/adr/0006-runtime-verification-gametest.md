# ADR-0006 — Runtime verification works: GameTest end-to-end in the container

- **Status:** Accepted
- **Date:** 2026-07-17
- **Supersedes:** the open concerns in ADR-0004 (GameTest discovery + headless verification)

## Context

ADR-0004 flagged runtime verification as the top project risk, citing community reports that
NeoForge GameTest discovery "does not work in production" and that headless verification is hard.
ADR-0005 proved build/packaging (levels 1–2). This ADR covers the runtime spike (levels 3–4).

## What was done

Inside the dev container, the generated Ruby Blade mod was run with `gradle runGameTestServer`.
The generator now emits:

- a `gameTestServer` run in `build.gradle` with `neoforge.enabledGameTestNamespaces = <modid>`;
- a `ModGameTests` class (`@GameTestHolder(MOD_ID)` + `@PrefixGameTestTemplate(false)`) with one
  `@GameTest(template = "empty")` per concept asserting `BuiltInRegistries.ITEM.containsKey(id)`;
- a shared empty 3×3×3 structure template NBT at `data/<modid>/structure/empty.nbt`
  (hand-built gzipped NBT — no dependency on NeoForge's test framework / `@EmptyTemplate`).

## Outcome — success

```
Enabled Gametest Namespaces: [amc_ruby_world]
2 tests are now running ...
========= 2 GAME TESTS COMPLETE IN 211 ms =========
All 2 required tests passed :)
GAMETEST_EXIT=0
```

The full runtime loop works: **generated mod → headless GameTest server → tests discovered and run
→ in-game registry assertions pass → exit code 0.**

## Findings (correcting ADR-0004)

- **GameTest discovery works** with `@GameTestHolder` + `neoforge.enabledGameTestNamespaces`. The
  "discovery is broken" worry did not reproduce on NeoForge 21.1.235.
- The real friction was mundane: **Minecraft 1.21 singularized data-pack folders** — GameTest
  structures load from `data/<ns>/structure/` (singular), not `structures/` (matching the
  `recipes`→`recipe` change we already handle).
- A hand-authored empty structure NBT is sufficient; the heavier `net.neoforged.testframework`
  (`@EmptyTemplate`, `@TestHolder`, `DynamicTest`) is not required for plain assertions.
- `runGameTestServer` exit code (0 = all required passed) is the clean pass/fail signal for the
  harness — wired into `amc-verify::gametest`.

## Consequences

- Runtime verification levels 1–4 (static, build, registry, GameTest) are proven and reproducible
  in the container.
- The generator emits GameTests from the registry-existence requirement automatically
  (`implementation/phase3/0003`).
- Still open: persistence (L5) and headless **client** smoke under Xvfb (L6); dedicated-server
  boot as its own level (L3) if wanted separately from GameTest.
