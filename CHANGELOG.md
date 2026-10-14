# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to adhere to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- **Fix: re-making something you already have showed the wrong name and then lost its card.**
  Asking for a creation that already exists (say, tapping the pumpkin tart example twice) updates
  the existing one, but the engine's reply only identified it by an internal id. The app fell back to
  a guess like "your tart", couldn't find that in the sidebar, and cleared the card with "no longer in
  your project" at the next refresh (such as pressing Quick Test). `amc request --json` now includes
  the creation it created, changed or deleted as `item`, and the app only tracks names that are in
  the sidebar.
## [0.3.0-alpha.4] — 2026-10-14

- **Quick Test and builds retry a failed download.** The first Quick Test (and the first build)
  downloads Minecraft and the loader, and one dropped connection used to fail the whole thing. Now a
  failure caused by a network hiccup (a timeout, a reset connection, a mirror's 5xx) is tried again,
  up to 3 times in all, and the app says it's retrying. Real errors, like a compile error, aren't
  retried. If the download keeps failing, the app says to check your internet connection instead of
  blaming the mod.
- **The build shows how long it's been running.** The active step now counts up ("working… 0:42")
  and, after a few seconds, notes that a build usually takes a minute or two, so a normal build no
  longer looks stuck.
- **Dependencies updated** to their current major versions: ureq 3, jsonschema 0.58, sha2 0.11 and
  thiserror 2 in the engine, plus the pending minor/patch updates for the engine and the desktop app
  and `actions/checkout` v7 in CI. jsonschema no longer pulls in an HTTP client, since the engine only
  validates its own bundled schemas.
- **Fix: the installers didn't include the Rust crate license notices** that NOTICE promised. The
  bundle scripts now generate `THIRD_PARTY_LICENSES.html` (desktop app) and
  `THIRD_PARTY_LICENSES-engine.html` (engine) with cargo-about, so packaging needs
  `cargo install cargo-about --locked --features cli`.
- **Fix: a change could be silently lost.** Two `amc` processes working on the same project (the app
  and the CLI, or two app windows) could both load it, and the one that saved last dropped the other's
  creation, even though both reported success. Commands that change a project now hold an exclusive
  lock on it, and a second one gets `project_busy` and changes nothing. The desktop app explains it,
  holds Quick Test while a create is running, and clears a result card whose creation is no longer
  in the project. Building from source now needs Rust 1.89 or newer.
- **Fix: the "a spiced pumpkin tart that heals 9" example failed.** The request parser didn't know
  "tart", so the request went to the model, which couldn't read it. It now knows tart and more food
  words (cupcake, pastry, biscuit, pudding, sandwich, burger, pizza, taco, burrito, noodles, jerky,
  cheese, chocolate, porridge, salad, smoothie), and a test checks that every example button in the
  app parses without the model.
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
