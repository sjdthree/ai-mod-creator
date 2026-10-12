# Contributing to MCE

MCE is developed **solo + Claude Code**, but with team-grade discipline because the product's
core promise is **0% false-success** (`SPEC-005 §18`). These rules keep work auditable and
traceable.

## The specs are the contract

The six documents in `Minecraft_Creation_Engine_Engineering_Package_v1.0/` are the stable
engineering contract. Don't edit them to reflect implementation drift — instead update the
**living backlog** (`implementation/`) and, for decisions, add an **ADR** (`docs/adr/`).

## Branching & commits

- Trunk-based: `main` is always releasable. Work on short-lived branches
  (`phaseN-<topic>`), open a PR, merge, delete.
- **Conventional Commits**: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`,
  `ci:`. Reference the work item: e.g. `feat(state): reference resolution (impl:phase1/0002)`.
- No direct commits to `main`.

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
cargo run -p mce-cli -- bench benchmarks/ --json
```

## Toolchain

- Rust stable (see `rust-toolchain.toml` when added), `cargo fmt`, `cargo clippy`.
- Phase 3+: JDK 21 (Temurin), Gradle wrapper (ADR-0003).
