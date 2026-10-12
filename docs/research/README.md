# Research

Research that informs the plans. These are inputs, not the contract — the decisions they drive
live in `docs/adr/` and the backlog.

## Fabric loader port

[`fabric-loader-port-assessment.md`](./fabric-loader-port-assessment.md) — what a second mod
loader (Fabric) costs, and the multi-loader seam it needs → ADR-0007 (multi-loader target).

## Diagnostics / LSP

[`lsp-architecture-assessment.md`](./lsp-architecture-assessment.md) — whether MCE should expose an
LSP. Conclusion: MCL is JSON operations, not a DSL; adopt a shared Diagnostic model rather than an
LSP server.
