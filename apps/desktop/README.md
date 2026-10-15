# AI-mod creator — desktop app

A Tauri desktop app: the web UI in `ui/` wrapped by a small Rust shell in `src-tauri/` that drives
the `amc` engine. The same `ui/index.html` also runs standalone in a browser (demo mode) — open it
directly to preview the interface with no engine.

```
apps/desktop/
├── ui/index.html        # the whole front-end (self-contained; demo mode when not in the app)
└── src-tauri/           # Rust shell: shells to `amc --json`, manages JDK 21 + the AMC Model
    ├── src/main.rs      # #[tauri::command]s: request · list_items · undo · export · play ·
    │                    #   model_status/start_model · minecraft_info · install_neoforge
    ├── tauri.conf.json  # window + bundle config; frontendDist = ../ui
    ├── binaries/        # bundled `amc` sidecar (staged at package time; gitignored)
    ├── resources/       # bundled AMC Model + grammar + license notices (staged at package time; gitignored)
    └── icons/           # app icons (regenerate with `cargo tauri icon icons/source.png`)
```

## What the app does for you automatically
- **Starts & health-checks the AMC Model** on its own private port (8770) — no terminal. The footer
  shows “starting the AMC Model… → AMC Model ready”. It prefers the **no-Python** runtime — native
  `llama-server` on the quantized GGUF (`models/ai-mod-create-270smol/`, ~278 MB) with the grammar + DRY sampler —
  and falls back to the Python MLX adapter (`models/ai-mod-create-270smol/adapter/`) only if the GGUF or
  `llama-server` isn’t present. Before starting it, the app runs `amc model verify` and refuses a model
  whose manifest (`amc-model.json`) was built for a different contract or grammar.
- **Finds a real JDK 21** for the build (via `/usr/libexec/java_home -v 21`), ignoring a stale
  `$JAVA_HOME`.
- **Quick Test** launches a self-contained **Minecraft 1.21.1 + NeoForge** sandbox — no setup, never
  touches your own worlds.
- **Add to my Minecraft** copies the mod into your game and, if you don’t have Minecraft 1.21.1 +
  NeoForge yet, offers to **install that profile for you** (official NeoForge installer, headless).

## Install & run (end users)

Not building from source? See **[docs/INSTALL.md](../../docs/INSTALL.md)** — the download → install →
run guide for **macOS and Windows 11**, covering the one prerequisite (**Java 21**, auto-detected), the
SmartScreen/per-user install notes, and first-run expectations (the first Quick Test does a one-time
Minecraft/NeoForge download). To *build* the Windows installer yourself, see the section below.

## Run it (dev)
```bash
cd apps/desktop/src-tauri
AMC_BIN="$(git rev-parse --show-toplevel)/target/debug/amc" cargo tauri dev
```
Dev prerequisites: **Rust** (nightly — see `rust-toolchain.toml`), the **Tauri CLI**
(`cargo install tauri-cli --version "^2.0" --locked`), **JDK 21**, and the **AMC Model runtime**
fetched once with `./scripts/fetch_model.sh` into `models/ai-mod-create-270smol/` (from Hugging Face) plus a `llama-server` on PATH. The app starts the model itself — you don’t run a
server by hand. `$AMC_MODEL_DIR` points it at a different model build.

## Build the Windows installer (.msi / .exe)

Tauri installers are **built on the target OS** — you cannot cross-compile a Windows `.msi` from macOS
(it needs the Windows SDK + WebView2). Build it on a Windows 11 box.

**One-time prerequisites on the Windows machine:**
- **Visual Studio 2022 Build Tools** with the *Desktop development with C++* workload (MSVC + CMake)
- **Rust** ([rustup.rs](https://rustup.rs) — installs the `x86_64-pc-windows-msvc` target)
- **Tauri CLI**: `cargo install tauri-cli --version "^2.0" --locked`
- **cargo-about** (generates the license notices): `cargo install cargo-about --locked --features cli`
- **Git**; WebView2 Runtime is preinstalled on Windows 11. **No Python needed.**

**Then, from the repo root,** in a *Developer PowerShell for VS 2022* (x64) so MSVC and CMake are on
PATH:
```powershell
git pull
powershell -ExecutionPolicy Bypass -File apps\desktop\scripts\build_windows.ps1
```
That one script: builds `amc.exe`, builds a **static** `llama-server.exe`, fetches the model pinned in
`model.lock` from Hugging Face and checks it with
`amc model verify`, stages the
grammar + license notices, and runs `cargo tauri build`. Output:
```
apps\desktop\src-tauri\target\release\bundle\msi\    → AI-mod creator_x.y.z_x64_en-US.msi
apps\desktop\src-tauri\target\release\bundle\nsis\   → AI-mod creator_x.y.z_x64-setup.exe
```
The resulting installer is self-contained (engine + model runtime + model), **no Python**. End users of
the installed app still need **JDK 21** (Temurin) for the Gradle build step.

## Build the macOS app / installer
```bash
# 0) one-time: fetch the AMC Model, build a STATIC llama-server (see below), and install
#    cargo-about for the license notices: cargo install cargo-about --locked --features cli
#    ./scripts/fetch_model.sh                    → models/ai-mod-create-270smol/{amc-model.json,*.gguf}
#    (static llama-server build → export AMC_LLAMA_SERVER=<path to build/bin/llama-server>)

# 1) stage engine + model runtime + model into the bundle
AMC_LLAMA_SERVER=/path/to/llama-server apps/desktop/scripts/stage_bundle.sh

# 2) bundle
cd apps/desktop/src-tauri
cargo tauri build --bundles app     # → target/release/bundle/macos/AI-mod creator.app (~300 MB)
# (use `--bundles dmg` or omit for a .dmg; .msi on Windows; .deb/.AppImage on Linux)
```
The `.app` carries its own `amc` engine + `llama-server` (sidecars in `Contents/MacOS/`) and the GGUF
model + grammar (in `Contents/Resources/`); the app self-locates all of them — no `AMC_BIN`, no PATH,
no Python. Verified by launching with a clean env.

**Building a static, dependency-free `llama-server`** (only macOS system frameworks — needed so the
sidecar runs on any Mac without Homebrew):
```bash
git clone --depth 1 https://github.com/ggml-org/llama.cpp && cd llama.cpp
cmake -B build -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DGGML_METAL=ON \
      -DLLAMA_CURL=OFF -DLLAMA_OPENSSL=OFF -DGGML_OPENMP=OFF
cmake --build build --target llama-server -j
# → build/bin/llama-server  (~15 MB; `otool -L` shows only System/Library + /usr/lib system libs)
```

## Packaging status — self-contained on macOS (Apple Silicon)

| Piece | In the `.app`? | Notes |
|---|---|---|
| `amc` engine (10 MB) | ✅ bundled sidecar | self-locating; no PATH/`AMC_BIN` |
| `llama-server` (15 MB, static) | ✅ bundled sidecar | no-Python model runtime; system frameworks only |
| **AMC Model** GGUF (~278 MB) + manifest | ✅ bundled resource | quantized Q8_0; from Hugging Face, pinned by `model.lock` |
| JSON grammar (`amc-ops.gbnf`) | ✅ bundled resource | guarantees parseable output |
| AMC Model MLX adapter (1.4 MB) | optional (`$AMC_ADAPTER_DIR`) | fallback if llama-server/GGUF absent |
| License notices (Gemma, llama.cpp, app) | ✅ bundled resource | `resources/licenses/` — required to redistribute the weights |
| Gradle | ✅ per project | generator embeds the wrapper |
| **Minecraft 1.21.1 + NeoForge** | ✅ handled in-app | Quick Test bundles it; “Add to my Minecraft” can install it |
| **JDK 21** | ❌ prerequisite | auto-detected if installed; the only remaining external dep (ship/link Temurin 21 to close it) |

**No Python at runtime.** The model is a quantized **Q8_0 GGUF** served by the bundled static
**llama.cpp** `llama-server` with a grammar + DRY sampler. The GGUF is built and released by the separate
model repository; this repo only consumes it (`scripts/fetch_model.sh`). Validated on-par with the MLX adapter, and the app auto-starts it.

**Remaining gap to a single double-click download:** bundle the `llama-server` binary (+ its dylibs)
and the GGUF into the `.app`, and ship/link a JDK 21. The engine, the model itself (GGUF), Gradle, and
the whole Minecraft/NeoForge side are already handled; today the app finds `llama-server` via Homebrew/
PATH rather than a bundled copy. The MLX/Python path remains as an automatic fallback.

## How it works
- **Create / Refine** → `amc request "<text>" --project <p>` (the model decides create vs. update),
  pinned to the app’s own model endpoint.
- **Your creations** → `amc list`. **↶ undo** → `amc undo`. **Add to my Minecraft** → `amc export`.
  **Quick Test** → `amc play --interactive`.
- Heavy pipeline runs off the UI thread, so the window stays responsive during build + verify.
- Failures are shown in plain language with a one-tap **Try again** (the small model is
  non-deterministic, so a retry often just works) and a “What happened?” detail toggle.
