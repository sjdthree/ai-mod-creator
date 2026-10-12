# Architecture Decision Records

Short, immutable records of significant decisions. Format: one file per decision,
`NNNN-title.md`. A decision is never edited once **Accepted** — instead it is **Superseded** by
a later ADR. See [Michael Nygard's template](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions).

| ADR | Title | Status |
|---|---|---|
| [0001](./0001-core-language-rust.md) | Rust workspace for the core engine | Accepted |
| [0002](./0002-ml-subsystem-boundary.md) | Python ML subsystem behind a narrow model bridge | Accepted |
| [0003](./0003-minecraft-target-pin.md) | Pin Minecraft 1.21.1 / NeoForge 21.1.x / Java 21 | Accepted |
| [0004](./0004-verification-risk.md) | Verification approach & GameTest/headless risk | Proposed |
| [0005](./0005-build-verified-spike-outcome.md) | Phase 3 spike: generated mod builds against NeoForge | Accepted |
| [0006](./0006-runtime-verification-gametest.md) | Runtime verification works: GameTest end-to-end | Accepted |
| [0007](./0007-intent-model-tiered-inference.md) | Intent model: FunctionGemma 270M + tiered escalation | Accepted |

