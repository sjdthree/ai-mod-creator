# ADR-0002 — The model lives outside the engine, behind a narrow bridge

- **Status:** Accepted (revised 2026-10-12)
- **Date:** 2026-07-17

## Context

A core architectural success criterion is that **model checkpoints can be replaced without
changing generator code** and **every model can be tested independently** (`SPEC-003 §20`).

## Decision

Model training lives outside this repository. The engine consumes only a **released model** —
a GGUF plus an `amc-model.json` manifest, published on Hugging Face and pinned by `model.lock` —
over a narrow runtime interface (any OpenAI-compatible chat endpoint; the desktop app runs
`llama-server`). The contract between them is the MCL **operation** schema (`SPEC-004 §19`), the
system prompt (`crates/amc-intent/contract/contract.txt`) and the decoding grammar
(`crates/amc-intent/contract/amc-ops.gbnf`). The manifest pins the sha256 of both, and
`amc model verify` refuses a model built against different bytes.

## Consequences

- **+** Model releases are decoupled from engine builds; a new model is a `model.lock` bump.
- **+** Anyone can serve the released model with stock llama.cpp.
- **−** Contract or grammar changes require a matching model release.
