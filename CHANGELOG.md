# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to adhere to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- **Desktop:** after a create, the result card and sidebar use the name the engine gave the creation
  (it showed a guessed name like "Pickaxe Durability"), and the new creation is highlighted in the
  sidebar. A successful create no longer shows the failure buttons, including an empty green one.
  The result card and Quick Test say which creative tab each creation is in.

## [0.3.0-alpha.3] — 2026-10-13

- **Renamed throughout to AI-mod creator / AMC.** The CLI is now `amc`, environment variables are
  `AMC_*` (were `MCE_*`), and new projects get `amc_` mod ids. Existing projects keep their mod ids,
  and their `.mce/` folder is moved to `.amc/` the first time they're opened.
- **Desktop:** the model server now stops when the app quits, and Quick Test only reports the game
  as open once its window is up.
- **Windows:** the build scripts are plain ASCII, so Windows PowerShell 5.1 can run them.
- **Docs:** install and user docs now say what the desktop app actually builds: Fabric mods (Fabric
  Loader + Fabric API). The `amc` CLI still defaults to NeoForge.
- **Security:** `rustls` 0.23.45 ([GHSA-2mjx-qc3c-rqvc](https://github.com/advisories/GHSA-2mjx-qc3c-rqvc)).

## [0.3.0] — 2026-10-12 — first public alpha

The first public release. Earlier development happened in a private repository.

- **Desktop app** (Tauri) for macOS (Apple Silicon) and Windows: type a request, watch it build and
  verify, then play or export the mod.
- **`amc` CLI:** `init`, `request`, `list`, `test`, `play`, `export`, `undo`, `revert`, `validate`, `model`, `bench`.
- **Supported content:** items, weapons, tools, food, blocks and crafting recipes, created, refined,
  renamed and deleted through plain-English requests.
- **Minecraft 1.21.1 / NeoForge**, with runtime verification through GameTests. Fabric support is
  in progress.
- **Model:** [`ai-mod-create-270smol`](https://huggingface.co/sjdthree/ai-mod-create-270smol) v1.0.0,
  pinned by `model.lock` and checked against the engine's contract and grammar by
  `amc model verify`.
