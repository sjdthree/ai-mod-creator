# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to adhere to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
