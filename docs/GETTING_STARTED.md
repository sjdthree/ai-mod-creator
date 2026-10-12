# Getting started

Mod Creation Engine (MCE) turns plain-English requests into **real, verified Minecraft mods** —
locally, with no coding. This guide gets you from a clone to your first working mod.

There are two ways to use MCE: the **desktop app** (recommended) and the **`mce` command line**.
Both run the same engine.

---

## 1. What you need

| Requirement | Why | Notes |
|---|---|---|
| **macOS, Linux, or Windows** | host OS | The visible Minecraft client runs best on a desktop OS. macOS Apple-Silicon is the primary tested target. |
| **JDK 21 (Temurin)** | builds the mod against NeoForge 1.21.1 | macOS: MCE auto-detects `/Library/Java/JavaVirtualMachines/temurin-21.jdk`. Otherwise set `JAVA_HOME`. |
| **Rust (stable)** | builds the engine | [rustup.rs](https://rustup.rs) |
| **The MCE Model** (~0.5 GB) | interprets your English | Our fine-tuned intent model (a Gemma-3-270M base), served locally (below). Runs on a normal laptop. |
| **~2 GB disk, first run** | NeoForge + Minecraft assets | Downloaded once and cached. |

> **Offline after setup.** Once the model and NeoForge cache are in place, creating mods needs no
> internet.

---

## 2. Get the code and build the engine

```bash
git clone https://github.com/sjdthree/ai-mod-creator
cd ai-mod-creator
cargo build -p mce-cli            # builds the `mce` engine → target/debug/mce
```

Put it on your `PATH` (optional but convenient):
```bash
export PATH="$PWD/target/debug:$PATH"      # or copy target/debug/mce somewhere on PATH
mce --help
```

---

## 3. Start the intent model

MCE reads your English with the **MCE Model** — a small local model (a fine-tuned Gemma-3-270M) served
over a standard (OpenAI-compatible) endpoint. The model is distributed separately from the engine;
`./scripts/fetch_model.sh` installs it into `models/ai-mod-create-270smol/` and checks it against this engine.

**Using the desktop app? Skip this section — the app starts and health-checks the model for you** on
its own private port (you'll see the status flip to “engine ready”). You only need the steps below for
the `mce` CLI, or to run the model yourself.

```bash
./scripts/fetch_model.sh      # one-time: models/ai-mod-create-270smol/{mce-model.json,*.gguf}
llama-server -m models/ai-mod-create-270smol/ai-mod-create-270smol-q8_0.gguf --port 8080 --temp 0 \
    --grammar-file crates/mce-intent/contract/mce-ops.gbnf \
    --dry-multiplier 0.8 --dry-base 1.75 --dry-allowed-length 3
```

Leave that running. (Any OpenAI-compatible endpoint works — point MCE at it with
`--intent-endpoint` or `$MCE_INTENT_ENDPOINT`.)

> The model's weights are covered by the Gemma Terms of Use (see `apps/desktop/licenses/GEMMA_NOTICE.txt`),
> not by this repository's Apache-2.0 license.

---

## 4a. Run the desktop app (recommended)

```bash
cargo install tauri-cli --version "^2.0" --locked      # one-time
cd apps/desktop/src-tauri
MCE_BIN="$(git rev-parse --show-toplevel)/target/debug/mce" cargo tauri dev
```

A window opens. Type a request, watch it build and verify, then **Play** or **Export**. See the
[User Guide](USER_GUIDE.md).

Build an installable app (`.app` / `.dmg` / `.msi` / `.AppImage`):
```bash
cd apps/desktop/src-tauri && cargo tauri build
```

## 4b. …or use the command line

```bash
mce init myworld
mce request "a ruby sword called Ruby Blade with 1200 durability and 9 damage" --project myworld
mce play   --project myworld          # opens Minecraft with your mod (auto-Creative)
mce export --project myworld          # drops the .jar into your real Minecraft mods folder
```

Full command reference and the create → refine → play loop are in the [User Guide](USER_GUIDE.md).

---

## 5. Building in isolation (optional)

If you don't want JDK 21 / Gradle on your host, MCE ships a dev container (JDK 21 + Gradle + headless
libs). See [`docs/dev-container.md`](dev-container.md). The desktop app uses your host JDK 21 directly.

---

## Troubleshooting

- **"engine unreachable" / the status dot is grey** — the intent model isn't running. Start it
  (step 3); the app expects it on `127.0.0.1:8080`.
- **Build fails with a Java/toolchain error** — you need **JDK 21**. Check `java -version`, or set
  `JAVA_HOME` to a Temurin 21 install.
- **First `play`/`request` is slow** — the first build downloads NeoForge + Minecraft assets (~2 GB),
  cached afterward.
- **I can't find my item in the game** — you must be in **Creative** mode to see the creative menu.
  `mce play` puts you in Creative automatically; if you made a world manually, run `/gamemode creative`,
  then press **E** and look in the matching tab (a sword is under **Combat**) or search its name.
