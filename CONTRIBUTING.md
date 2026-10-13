# Contributing to AI-mod creator

Thanks for helping! Bug reports, ideas, docs fixes and code are all welcome. By taking part you agree
to follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## Ways to help

- **Report a bug or share an idea:** [open an issue](https://github.com/sjdthree/ai-mod-creator/issues/new/choose).
  The exact sentence you typed and a screenshot make a bug much easier to fix.
- **Find something to work on:** issues labeled
  [`good first issue`](https://github.com/sjdthree/ai-mod-creator/labels/good%20first%20issue) or
  [`help wanted`](https://github.com/sjdthree/ai-mod-creator/labels/help%20wanted). For anything big,
  comment on the issue (or open one) before you start, so we can agree on the approach.
- **Security problems:** report them privately, as described in [SECURITY.md](SECURITY.md).

## Sending a pull request

1. Fork the repo and create a branch from `main`.
2. Make your change, with tests for any behavior you change or add.
3. Run `./scripts/ci.sh` (see [Local checks](#local-checks-before-pushing)). It runs the same checks as CI.
4. Open a pull request against `main` and fill in the template. CI must pass before it can merge, and
   your branch must be up to date with `main`.

Contributions are licensed under the project's [Apache-2.0 license](LICENSE), as described in its
section 5. You keep the copyright to your work.

The rest of this guide covers the engineering rules the project follows. The core promise is
**0% false-success** (`SPEC-005 §18`): the app never claims a mod works unless it was checked.

## The specs are the contract

The six documents in `AI_Mod_Creator_Engineering_Package_v1.0/` are the stable
engineering contract. Don't edit them to reflect implementation drift — instead update the
**living backlog** (`implementation/`) and, for decisions, add an **ADR** (`docs/adr/`).

## Branching & commits

- Trunk-based: `main` is always releasable. Work on short-lived branches
  (`phaseN-<topic>`), open a PR, merge, delete.
- **Conventional Commits**: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`,
  `ci:`. Reference the work item: e.g. `feat(state): reference resolution (impl:phase1/0002)`.
- No direct commits to `main`: it is protected, and every change goes through a pull request.

## Definition of Done (`SPEC-006`)

A change is Done only when:

1. Implementation merged.
2. Required unit tests pass.
3. Required integration tests pass.
4. Required benchmark cases pass.
5. Docs updated if interfaces changed.
6. CI green, no regression.

## The false-success rule (non-negotiable)

Never report success for unverified behavior. In practice:

- Unimplemented paths return a structured error with a **non-zero** exit code — never a
  success placeholder.
- A failed build/generate/verify must leave the last verified state **intact**
  (`SPEC-001 NFR-003`); tests must prove this.
- "It compiled" is not "it works" (`SPEC-003 §11`). Gameplay claims require runtime verification.

## Failure ownership

Every failing benchmark/CI case is assigned exactly one primary owner from the taxonomy in
`SPEC-005 §17` (e.g. `MODEL_INTERPRETATION`, `GENERATOR`, `BUILD_ENVIRONMENT`, `GAMETEST`). Only
**model-owned** failures become model regression cases (`SPEC-005 §17`).

## Local checks before pushing

Run the local gate — it mirrors `.github/workflows/ci.yml` exactly, so green here means green on
CI (and is the source of truth if GitHub Actions is disabled):

```bash
./scripts/ci.sh
```

Equivalent individual commands:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p amc-cli -- bench benchmarks/ --json
```

## Toolchain

- Rust stable for the engine (pinned in `rust-toolchain.toml`); the desktop app pins nightly in
  `apps/desktop/src-tauri/rust-toolchain.toml`. Plus `cargo fmt` and `cargo clippy`.
- Phase 3+: JDK 21 (Temurin), Gradle wrapper (ADR-0003).
