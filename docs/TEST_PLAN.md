# AMC Test Plan — full system, multi-loader

*Living document. Started 2026-07-22 on branch `feat/fabric-loader-support` (PR #18). Covers the
whole engine through the multi-loader lens (NeoForge default, Fabric scaffold per ADR-0007).*

The plan is organized as **tiers** (fast → slow, cheap → toolchain-heavy) mirroring the CI tiers in
`SPEC-005 §28`, then broken into **CLI** and **GUI/system** suites. Each case lists what it proves,
how to run it, and the loader(s) it applies to.

---

## 0. Prerequisites — the stuff we need

### Toolchain inventory (probed 2026-07-22 on this Mac)

| Tool | Status | Notes |
|---|---|---|
| **JDK 21** | ✅ present | Temurin `21.0.2` at `/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home`. **Not** the shell default. |
| Default `java` (PATH) | ⚠️ JDK 17 | Homebrew `openjdk@17`. Fine to *launch* Gradle; compile toolchain is pinned to 21. |
| `JAVA_HOME` (shell) | ⚠️ JDK 8 | Points at `adoptopenjdk-8`. **Must override for direct CLI tests** (see gotcha below). |
| System `gradle` | ➖ absent | Not needed — the generator embeds the Gradle **wrapper**. |
| Node / npm / pnpm | ✅ | Node 22.22, npm 10.9, pnpm — for the Tauri GUI. |
| Rust / cargo-tauri | ✅ | rustc 1.97, `cargo-tauri` installed. |
| `gh` | ✅ authed | account `sjdthree`. |
| `xvfb-run` | ➖ absent | Expected on macOS (real display); only the Linux headless smoke needs it. |

**Verdict on "do we have JDK 21?": yes.** No install needed. The one hazard was environment wiring,
which is now **fixed** (see below).

### JDK 21 machine setup — RESOLVED (2026-07-22)

The shell config was cleaned up so JDK 21 is the default for **every** shell type. Root cause: three
dotfiles disagreed and none set 21 — `~/.zshenv` forced JDK 8 onto all non-interactive shells (what
`amc`/Gradle inherit), `~/.zshrc` set JDK 17 for interactive terminals, nothing set 21.

Fix (single source of truth): a new `~/.java-env` sets `JAVA_HOME` to Temurin 21 (idempotent, no PATH
bloat) and defines a `use-java <version>` switcher; it's sourced from `~/.zshenv` (all zsh) and
`~/.bashrc` + `~/.bash_profile` (bash). The stale JDK 8 (`~/.zshenv`) and JDK 17 (`~/.zshrc`) lines
were removed. All originals backed up as `*.bak-jdk21-cleanup`. Verified: non-interactive zsh,
interactive login zsh, and login bash all report `JAVA_HOME=…/temurin-21…` and `java 21.0.2`;
`use-java 17`/`use-java 21` switch cleanly; nested shells show no PATH duplication.

**Consequence for testing:** the T-1 workaround (manually exporting `JAVA_HOME` before builds) is no
longer needed on this machine — a fresh shell already has 21. GitHub Actions still pins its own JDK
via `setup-java`.

### The `JAVA_HOME` gotcha (critical for CLI testing)

The **desktop app** already does the right thing: `java_home()` runs `/usr/libexec/java_home -v 21`
and injects `JAVA_HOME` into every `amc` invocation (`apps/desktop/src-tauri/src/main.rs`). But when
we run the **CLI directly** in a shell, `amc` inherits the shell's `JAVA_HOME` — which is **JDK 8**
here. Gradle 8.10 can *run* on 8 but it's deprecated and its toolchain must still locate JDK 21 to
compile. To make CLI tests deterministic and match production, **export JDK 21 before any build/verify
test**:

```bash
export JAVA_HOME="$(/usr/libexec/java_home -v 21)"
export PATH="$JAVA_HOME/bin:$PATH"
```

This should become a line in `scripts/ci.sh`'s build-tier and in any test harness that shells to
Gradle. (Action item T-1.)

### Intent model (needed only for tests that go through English → ops)

`amc request` needs an OpenAI-compatible endpoint (`$AMC_INTENT_ENDPOINT`). Two options:
- **Desktop path:** the bundled `llama-server` sidecar + `ai-mod-create-270smol` gguf (started by the app).
- **CLI path:** `llama-server` run by hand with the same gguf + grammar (see `docs/GETTING_STARTED.md`).

**For deterministic CLI tests we should avoid the model entirely** where possible (small models are
non-deterministic across prompts). Two model-free routes already exist:
1. `amc validate` — applies hand-written ops JSON against state. Proves
   validate→state without the model.
2. Generation/build/verify operate on **canonical state**, not English — so a fixture project with a
   committed state exercises generate→build→verify→play with zero model calls.

*Gap:* there is no `amc generate`/offline subcommand to materialize a project from a fixture state
without going through `request`. A tiny fixture-driven test binary (or a `--from-state <file>` on
`test`) would make the build/verify tiers fully model-independent. (Action item T-2.)

### First-run network

The build tier downloads the loader + Minecraft + mappings on first run (NeoForge MDK ~hundreds of
MB; Fabric via Loom similar). Budget one slow first run per loader; subsequent runs hit the Gradle
cache. CI needs network + a warm `~/.gradle` cache to stay under time limits.

---

## 1. Tier 0 — unit / logic (no toolchain)

Fast, hermetic, run on every commit. **Status: green (37 tests).**

| ID | Case | Loader | How |
|---|---|---|---|
| U-1 | Generator unit + golden-content tests | both | `cargo test -p amc-generator` |
| U-2 | Shared files **byte-identical** across loaders | both | `fabric::tests::shared_vanilla_resources_are_identical_to_neoforge` |
| U-3 | Output hash deterministic per loader | both | `*::output_hash_is_deterministic` / `output_is_deterministic` |
| U-4 | `Loader` parse/round-trip + factory target/task | both | `lib::tests::*` |
| U-5 | Build-log classification, GameTest-log parse | both | `cargo test -p amc-build -p amc-verify` |
| U-6 | State/validator/intent units | n/a | `cargo test --workspace` |

Command: `cargo test --workspace` (plus `cargo fmt --check`, `cargo clippy -- -D warnings`).

---

## 2. Tier 1 — generation / golden trees (no toolchain)

Proves each loader emits a **complete, correct file tree** from a fixed state, deterministically.

| ID | Case | Loader | Expectation |
|---|---|---|---|
| G-1 | Full tree present (item/weapon/tool/food/block + recipe) | neoforge | `neoforge.mods.toml`, `build.gradle`, Java, resources |
| G-2 | Full tree present | fabric | `fabric.mod.json`, Loom `build.gradle`, Fabric Java, resources |
| G-3 | Regenerate to same dir → identical `output_hash` | both | reproducibility (`SPEC-005 §22`) |
| G-4 | Prune: remove recipe / delete / rename concept clears stale files | both | `regeneration_prunes_*` (extend to Fabric) |
| G-5 | Metadata sanity: entrypoints, deps, mappings | fabric | `fabric-gametest` entrypoint, `officialMojangMappings` |

*Action item T-3:* add a Fabric analogue of the NeoForge prune test.

---

## 3. Tier 2 — real build (JDK 21 + network)

The premise-validating tier: **does the generated project actually compile & package?**

| ID | Case | Loader | Command (in generated project) |
|---|---|---|---|
| B-1 | Clean build → JAR in `build/libs/` | neoforge | `./gradlew --no-daemon build` (baseline; MVP-verified) |
| B-2 | Clean build → remapped JAR | fabric | `./gradlew --no-daemon build` (**scaffold — under test now**) |
| B-3 | `discover_jar` finds the right artifact (ignores `-sources`/`-dev`) | both | `amc test` reports a `jar` path |
| B-4 | Build-failure classification maps to correct owner | both | inject a bad property, expect `CompileError`/GENERATOR |

Run via CLI (`amc test`) once model-free fixtures exist (T-2), or directly with `./gradlew build`.
**Always with `JAVA_HOME=21`.**

*Fabric-specific risks to watch (B-2):* Loom `runs.gametest` config shape, remapped-jar naming under
`build/libs/` (affects `discover_jar`), and whether `Item.Properties` needs a registry id on 1.21.1
(it does not until 1.21.2 — we target 1.21.1).

---

## 4. Tier 3 — runtime verification / GameTest (JDK 21 + network)

Proves the mod **loads and behaves** in a real server, per the verification levels (`SPEC-005 §15`:
ServerBoot → Registry → GameTest → Persistence).

| ID | Case | Loader | Task |
|---|---|---|---|
| V-1 | GameTests pass (registry + behavioural) | neoforge | `runGameTestServer` (baseline) |
| V-2 | GameTests pass | fabric | `runGametest` — **PASS (6/6, 2026-07-22)** after two fixes (below) |
| V-3 | Log parser reports pass/fail + count correctly | both | `parse_gametest_log` against real output |
| V-4 | A deliberately-broken concept fails verification (no false pass) | both | negative control |

**Fabric V-2 is the gate** for promoting Fabric scaffold → supported. Unknowns to confirm against a
real run: (a) `fabric-gametest` entrypoint auto-discovers `@GameTest` methods; (b) template-namespace
resolution finds `data/amc_ruby/structure/empty.nbt`; (c) the `runGametest` task name and its exit
code / summary lines match what `parse_gametest_log` expects. If the summary format differs, the
parser needs a Fabric branch.

---

## 5. Tier 4 — client smoke (`amc play`)

| ID | Case | Loader | Notes |
|---|---|---|---|
| P-1 | Client boots to ready marker, then stops (headless smoke) | both | Linux/CI via `xvfb-run`; macOS uses interactive |
| P-2 | Refuses to launch unverified work | both | `GradleLauncher` guard (unit-covered; confirm live) |
| P-3 | `READY_MARKERS` actually appear in a Fabric client log | fabric | markers are generic MC lines — confirm once |

---

## 6. Tier 5 — GUI / full system (Tauri desktop app)

The desktop app (`apps/desktop`) is a thin shell: 12 Tauri commands each shell out to bundled
`amc --json`. Testing it = testing the IPC layer + the CLI it drives + the model server bring-up.

### 6a. Command coverage (map each Tauri command to a CLI path)

| Command | Drives | Test |
|---|---|---|
| `list_projects` | scans projects home | empty + N projects |
| `init_project` | `amc init` | creates `.amc/state.json` |
| `list_items` | `amc list --json` | reflects committed concepts |
| `request` | `amc request` | English → verified creation (needs model) |
| `undo` | `amc undo` | reverts last change |
| `install_neoforge` | downloads/points a MC loader | **NeoForge-only today** |
| `minecraft_info` / `pick_mods_folder` / `export` | `amc export` | JAR lands in mods folder |
| `play` | `amc play` | launches client |
| `model_status` / `start_model` | llama-server sidecar | model reachable |

### 6b. Model bring-up

- `start_model` launches the `llama-server` sidecar with the gguf + grammar; `model_status` polls.
- Test: cold start → `model_status` true within timeout; `request` then succeeds.

### 6c. GUI automation options

- **Manual scripted walkthrough** (fastest to stand up): init → request "a ruby sword" → see item →
  play → export. Record pass/fail per step.
- **Automated:** drive the WebView UI (`apps/desktop/ui/index.html`) via a browser-automation MCP or
  Tauri's WebDriver (`tauri-driver`); assert on the rendered result JSON. Heavier setup.
- **IPC-level:** call the Tauri commands directly in a Rust integration test with a fake/echo `amc`
  on `$AMC_BIN`, asserting arg construction and JSON parsing without a real build. (Cheapest signal
  for the shell logic; recommended first.)

### 6d. GUI gaps this branch surfaces (multi-loader)

1. **No loader selector.** `request`/`play` don't pass `--loader`; the GUI is NeoForge-only. Add a
   loader toggle → thread `--loader` into `run_amc` args. (Action item T-4.)
2. **`install_neoforge` is loader-specific.** Fabric needs an `install_fabric` equivalent (or a
   loader-parameterized installer). (Action item T-5.)
3. The bundled `amc` sidecar must be rebuilt from this branch before the GUI can target Fabric.

---

## 7. Initial test results (this session)

- **Toolchain probe:** JDK 21 present; env-wiring gotcha documented (§0).
- **Tier 0:** `cargo build --workspace --all-targets` clean, clippy clean, **37 tests pass**.
- **Tier 1:** both loaders generate complete trees (NeoForge + Fabric verified by eye and tests).
- **Tier 2 (B-2, Fabric Loom build): PASS.** A real `./gradlew build` (Loom 1.7.4, Mojang mappings,
  JDK 21) compiled and packaged a remapped `amc_ruby-0.1.0.jar`. The first attempt **failed** and
  caught a genuine scaffold bug: vanilla `SwordItem.createAttributes(Tier, int, float)` takes an
  **int** damage bonus, but NeoForge patched it to a `float` — the Fabric generator had copied the
  `8.0F` float literal. Fixed to emit an int (`fabric.rs`), locked in by a regression assertion, and
  the rebuild succeeded in 5s. *This is the value of the build tier: only a real compile surfaces
  loader-API divergences the golden tests can't.*
- **JDK 21 machine setup:** resolved (see §0).

### Fabric API divergences found so far (watch-list for further build/verify)

| Symbol | NeoForge | Vanilla/Fabric | Status |
|---|---|---|---|
| `SwordItem.createAttributes` damage bonus | `float` | **`int`** | fixed (B-2) |
| Tool `*.createAttributes` damage bonus | `float` | `float` | ok (no change) |
| GameTest entrypoint class | static scan (private ctor ok) | **instantiated** (needs public ctor) | fixed (V-2) |
| GameTest `template` namespace | `@GameTestHolder` supplies it → bare `"empty"` | no holder → must be `"<modid>:empty"` | fixed (V-2) |
| Registration | `DeferredRegister`/`DeferredItem` | `Registry.register` | handled |
| Creative tabs | `BuildCreativeModeTabContentsEvent` | `ItemGroupEvents` | handled |

**Tier 3 (V-2) now PASSES:** `runGametest` reported `6 GAME TESTS COMPLETE … All 6 required tests
passed :)`. The summary format is identical to NeoForge's, so `parse_gametest_log` handles it
unchanged. Remaining before full promotion: client smoke (`runClient`, P-3) and a CI matrix (T-6).

---

## 8. Action items (test enablement)

| ID | Item | Why |
|---|---|---|
| T-1 | ~~Export `JAVA_HOME` in `scripts/ci.sh`~~ — **DONE via machine fix**; JDK 21 is now the shell default (see §0) | CLI builds no longer inherit JDK 8 |
| T-2 | Model-free generation route (fixture state → project) for build/verify tiers | deterministic, no model dependency |
| T-3 | Fabric prune test (mirror NeoForge) | parity coverage |
| T-4 | Loader selector in GUI → thread `--loader` | GUI can target Fabric |
| T-5 | `install_fabric` (or loader-parameterized installer) | Fabric runtime in GUI |
| T-6 | CI matrix: `{neoforge, fabric} × {build, gametest}` jobs | catch loader regressions |
| T-7 | Fabric `parse_gametest_log` confirmation / branch | summary format may differ |
