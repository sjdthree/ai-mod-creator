# LSP Architecture Assessment

**Status:** Research / decision input — not a committed direction.
**Date:** 2026-07-21
**Scope:** Evaluate whether the Language Server Protocol (LSP) is a good fit for this project's
validation / AI-repair architecture. Read-only investigation; no code was changed.
**Method:** Grounded in the actual code (`crates/*`, `schemas/`, `apps/desktop/`) and the engineering
specs (`SPEC-003` System Architecture, `SPEC-004` MCL Specification, ADR-0002/0007/0008). Every claim is
cited to `file:line` where it comes from code. Statements that are inference rather than a direct read are
tagged **[INFERENCE]**. Statements about external tools (JDT.LS, Spyglass, tower-lsp) come from general
knowledge, not this repo, and are tagged **[EXTERNAL]**.

---

## TL;DR

- **The premise needs one correction up front.** There is **no textual DSL** in this project, and the
  term "MDM / Mod Description Model" **does not appear anywhere in the code or specs**. The central schema
  the summary calls the "MDM" is this project's **MCL — Mod Creation Language**: a **JSON operations
  language** (a 15-verb enum) plus a **typed canonical state**, defined by `SPEC-004` and the seven JSON
  Schemas under `schemas/0.1.0/`. Non-programmer creators never edit MCL text; they type **English** in a
  GUI, which an intent model turns into MCL operations, which mutate `state.json`. (`docs/traceability.md:14`,
  `crates/amc-state/src/model.rs`, `schemas/0.1.0/operation.schema.json`.)

- **LSP is a protocol for humans editing *text documents*.** In this product the human never opens a text
  document, and the MCL operations are ephemeral machine artifacts. So the classic LSP surface
  (completion / hover / go-to-def over a source file, driven by a tree-sitter grammar) has **no document
  and no editor client** on the creator path. A tree-sitter grammar "for the DSL" would be grammar for a
  language nobody types.

- **What is genuinely valuable is LSP's *diagnostic data model*** — a structured `{severity, code, source,
  message, range, relatedInformation, codeActions}` record — as the internal error contract that both the
  GUI and (future) AI-repair loop consume. But you can adopt that data shape **without** adopting
  LSP-the-server. The validator already emits ~80% of it (`ValidationError { code, path, message,
  owning_component }`, `crates/amc-validator/src/lib.rs:27-35`); what it lacks is **source positions** and
  **an AI feedback edge** (there is no repair loop today).

- **Recommendation: neither option (a) nor (b) as written.** Adopt the LSP *diagnostic model* as an
  internal structured-diagnostics contract at the existing CLI `--json` seam; spend the effort on the
  cross-file **integrity gaps** and a **structured AI-repair channel**, which is where the real value is.
  Reserve actual LSP servers (JDT.LS, Spyglass, JSON-schema LS) for a *future developer/debug surface* over
  the **generated artifacts**, behind a thin orchestrator, **if** a text-editing workflow ever appears.

- **That "if" is addressed in Phase 4 (the no-code→code bridge addendum).** When the product's growth path
  is to let creators graduate into editing real code, the LSP case **flips positive** — but as
  *orchestration of existing servers (JDT.LS/Spyglass/JSON-schema) over the generated project*, and only
  after solving a **round-trip/ownership problem** (`prune_stale` deletes hand-edits) that LSP itself does
  not touch. Still no bespoke MCL language server. See §11–§18.

---

# Phase 1 — Current architecture (read-only map)

## 1. The DSL / MDM: it's MCL (JSON operations + typed state), not a textual DSL

**Terminology check (important):** the project's own name for the central schema is **MCL — Mod Creation
Language** (`docs/traceability.md:14`, `docs/lessons-learned.md:30`, `SPEC-004`). The strings
"MDM" and "Mod Description Model" appear **nowhere** in the codebase — that is the caller's framing.
**[INFERENCE]** "MDM" ≙ *MCL operations + canonical state*.

**Schema definition** — `schemas/0.1.0/` holds seven JSON Schema (draft 2020-12) files, all titled
"MCL …", `$id` base `https://amc.dev/schemas/0.1.0/`:

| File | Role |
|---|---|
| `operation.schema.json` | The **verb language** the intent model emits: `operation` is a 15-value enum, each with a `payload`; defines the shared `reference` `$def` |
| `envelope.schema.json` | Common fields on every MCL *document* (`schema_version`, `document_type`, `document_id`, `revision`) |
| `concept.schema.json` | Canonical item/weapon/tool/food/block; `namespace_id` pattern `^[a-z0-9_]+:[a-z0-9_/]+$` |
| `recipe.schema.json` | shaped / shapeless / smelting / blasting, conditional required fields per type |
| `appearance.schema.json` | template / material_theme / palette / style_tags (enums) |
| `behavior.schema.json` | **restricted** trigger→condition→action, enums only ("no arbitrary code") |
| `transaction.schema.json` | transaction record, 8-state lifecycle enum |

**Parser / validator** — there is no text parser; MCL arrives as `serde_json::Value`. Schema validation
lives in `crates/amc-schema/src/lib.rs`: schemas are `include_str!`-embedded at compile time (`:29-35`),
`SCHEMA_VERSION = "0.1.0"` (`:14`), entry points `validate(doc_type, &Value)` (`:93-104`) and `is_valid`
(`:107-109`). Semantic validation is a separate crate (`amc-validator`, see §2).

**The canonical data model** — `crates/amc-state/src/`:
- `model.rs`: `CanonicalState { schema_version, name, namespace, concepts: BTreeMap, recipes: BTreeMap }`
  (`:105-119`), `Concept` (`:36-55`), `Recipe` (`:70-94`), `ConceptType`/`RecipeType`/`Status` enums.
  `state_hash()` gives the determinism hash (`:143-189`).
- `reference.rs`: `Reference { reference_type, value }` + `Resolution` (Concept / Vanilla /
  NeedsClarification / NotFound). Ambiguity never resolves silently (`:131-140`).
- `manager.rs`: `ProjectStateManager` — transactional `open_candidate` → `commit(verified)` → `revert`;
  canonical state mutates **only if verified** (`:178-205`); `apply_operation` dispatch (`:418-441`).

**Where MCL / state is read or written:**

| Location | Role |
|---|---|
| `manager.rs:142` `open_candidate` | apply ops → candidate state |
| `manager.rs:178` `commit` | candidate → canonical (only if verified) |
| `manager.rs:418-441` `apply_operation` | reads `op["operation"]`/`["payload"]` |
| `model.rs:143-189` `state_hash` | serialize concepts/recipes for the determinism hash |
| `amc-cli/src/project.rs:112-132` `save` / `:86-109` `load` | persist/restore `.amc/state.json` (atomic) |
| `amc-cli/src/main.rs:450,467,527` | request flow: validate → open_candidate → commit |
| `apps/desktop/src-tauri/src/main.rs:410,464` | **desktop shells out to the `amc` CLI `--json`** — it does *not* link the crates |

The desktop app talking to the engine only through the CLI `--json` contract is a load-bearing fact for
the LSP question (see §9, §10): there is already a stable machine-readable seam.

## 2. The validation story — where errors are caught, and the complete rule list

Errors are detected at **four** distinct times:

| When | Where | Error shape |
|---|---|---|
| **Intent / parse time** | `crates/amc-intent/src/http.rs` | **free-text strings** in `AmcError::validation(Component::IntentAdapter, …)` (e.g. "model output had no valid operations array", `http.rs:195-200`) |
| **Validation time** (Stages 1–8) | `crates/amc-validator` (Stages 1–6) + `crates/amc-state` (Stages 7–8) | **structured** `ValidationError` / `Clarification` / `Unsupported` |
| **Compile time** | `crates/amc-build` — Gradle build | `BuildOutcome` classified by **substring match** on the log; `CompileError → owner "GENERATOR"` (`lib.rs:39-48,145-176`) |
| **Game runtime** | `crates/amc-verify` — `runGameTestServer` | pass/fail parsed from GameTest summary lines (`gametest.rs:24-42`); registry + behavior asserted, **recipes not asserted** |

The validator's declared **stages** (`SPEC-003 §7`, inline comments in `amc-validator/src/lib.rs`) are
JSON syntax → Schema → References → Properties → Semantics → Capability (Stages 1–6), then
candidate-state schema + referential integrity (Stages 7–8, enforced in `amc-state`). **[INFERENCE]** these
are conceptual comment markers, not a reified pipeline object an LSP could iterate as data.

**Complete list of semantic validation rules** (from `crates/amc-validator/src/lib.rs`; each carries a
`code`, an `operation_index`, and a hand-authored dotted `path` — but **no line/column**):

| # | Check | Function:line | code | path |
|---|---|---|---|---|
| 1 | op not a JSON object | `validate_op:117` | `NOT_AN_OBJECT` | `""` |
| 2 | missing `operation` | `validate_op:127` | `MISSING_OPERATION` | `operation` |
| 3 | operation schema invalid | `validate_op:139` | `SCHEMA_INVALID` | `""` |
| 4 | duplicate/namespace collision | `validate_create:209` | `DUPLICATE_CONCEPT` | `payload.display_name` |
| 5 | concept type unsupported | `validate_create:230` | `UNSUPPORTED_CONCEPT` (→Unsupported) | — |
| 6 | update missing `target` | `validate_update:254` | `MISSING_TARGET` | `payload.target` |
| 7 | delete missing `target` | `validate_delete:284` | `MISSING_TARGET` | `payload.target` |
| 8 | target reference unparseable | `resolve_target:492` | `BAD_REFERENCE` | `payload.target` |
| 9 | target ambiguous | `resolve_target:512` | `AMBIGUOUS_TARGET` (→Clarify) | — |
| 10 | target not found | `resolve_target:517` | `TARGET_NOT_FOUND` | `payload.target` |
| 11 | target is vanilla, not a concept | `resolve_target:525` | `TARGET_NOT_A_CONCEPT` | `payload.target` |
| 12 | add_recipe missing `recipe` | `validate_add_recipe:325` | `MISSING_RECIPE` | `payload.recipe` |
| 13 | recipe type unsupported | `validate_add_recipe:341` | `UNSUPPORTED_RECIPE_TYPE` (→Unsupported) | — |
| 14 | recipe not an object | `validate_add_recipe:351` | `BAD_RECIPE` | `payload.recipe` |
| 15 | recipe missing `result` | `validate_add_recipe:366` | `MISSING_RESULT` | `payload.recipe.result` |
| 16–20 | recipe result / key / ingredients / smelt-ingredient ambiguous or unresolved | `:380-481` | `AMBIGUOUS_REFERENCE` (→Clarify) / `REFERENCE_UNRESOLVED` | `payload.recipe.*` |
| 21 | relative term unresolved (e.g. "stronger") | `check_properties:585` | `UNRESOLVED_VALUE` | `properties.{field}` |
| 22–24 | numeric must-be-int / min / max | `check_properties:599-629` | `PROPERTY_CONSTRAINT` | `properties.{field}` |

Numeric constraint table (`check_properties:554-577`) covers weapon/tool/item/food/block property ranges
(e.g. `attack_damage > 0`, `light_level 0..=15`, `max_stack_size == 1` for weapons/tools).

**Two limitations that matter for LSP** (both from the validator code):
- **One finding per operation.** `check_properties` returns the *first* violation (`Option<ValidationError>`),
  and each op yields a single `OpOutcome` — so a diagnostics publish would be incomplete without a refactor
  to accumulate.
- **`clarify_request` / `unsupported_request` carry only `operation_index`**, no `path` — they'd map to a
  whole-operation range at best.

## 3. The AI loop — and the fact that there is no repair loop

- **Input:** the system prompt is the **CONTRACT**, `include_str!`'d from `crates/amc-intent/contract/contract.txt`
  (`amc-intent/src/lib.rs:31`) and **SHA-256-pinned by a test** to prevent train/serve skew. The user
  message mirrors training exactly. The call is a single OpenAI-compatible `/v1/chat/completions` request,
  temperature 0, greedy (`http.rs:67-78,172-190`).
- **Output:** parsed by `extract_ops` (scan balanced `[...]`, take the last valid ops array), then
  sanitized (`resolve_numeric_properties`, `normalize_stack_size`). Output is *proposed, never trusted*.
- **Deterministic fast-path:** `interpret()` tries deterministic parsers **first**
  (`deterministic_create/update/rename/delete/recipe/unsupported`) and **only calls the LLM if all decline**
  (`http.rs:92-208`). **[INFERENCE]** the common create/update/rename/delete/recipe cases are now fully
  model-free; the LLM is a long-tail fallback. (Hit-rate split not measured here.)

**There is NO loop that feeds a validation/build/verify error back to the LLM for a repair attempt.**
Confirmed by grep across `crates/` + `apps/`: the only "retry" is an *infrastructure-flake* GameTest re-run
of a byte-identical build (`runner.rs:32-46`), not a model re-prompt. A `repairable: bool` field exists on
`ValidationError` (`amc-validator/src/lib.rs:34`) but is **written and never read** — the original design
anticipated automated repair that was never built. Every downstream error terminates by being surfaced to
the user as a `--json` payload + non-zero exit code (`main.rs` `fail`/`finish`, `:242-251` / `:845-871`).

**Format of errors today** (this is the "what would LSP replace?" baseline):
- Validation → **structured JSON** (`{status:"rejected", errors:[ValidationError], clarifications, unsupported}`).
- Intent → **free-text** strings.
- Build → `{status:"build_failed", outcome:"<Rust Debug>", owner, log_tail:<40 raw lines>}` — **unstructured**.
- Verify → `{status:"verification_failed", failed_assertions, failure_owner}`.

The desktop GUI already *consumes the structured `errors` array* (unpacks `.code`/`.message`, special-cases
`DUPLICATE`, offers "Update X instead" / "Try again") — `apps/desktop/ui/index.html:563-664`. **[INFERENCE]**
"Try again" is the closest thing to repair, but it re-runs the *same* prompt with **no error context** — a
fresh one-shot, not a feedback loop.

## 4. Cross-file / referential integrity — strong within a concept, gaps at the reference boundary

The generator (`crates/amc-generator/src/lib.rs`) is a **full re-materialization** from one `CanonicalState`:
`build_files` emits every artifact, then `prune_stale` deletes any file under `src/main/java` /
`src/main/resources` not in the fresh set (`:98,116`). Every emitter iterates the *same*
`sorted_concepts(state)` and derives names from the *same* `path_of(namespace_id)`.

**Guaranteed by construction** (no second copy exists to drift): a registered item **always** gets a model
+ texture + lang entry; model↔texture paths agree; block blockstate↔model↔item-model↔loot-table↔texture
agree; creative-tab accessor and GameTest id match the registered id. `prune_stale` guarantees no orphan
survives a rename/delete. A lang file (`assets/{modid}/lang/en_us.json`) and creative-tab placement
(Java, via `BuildCreativeModeTabContentsEvent`) both exist.

**Gaps** (all at the *reference boundary*, none caught before Gradle build or Minecraft runtime):

| # | Gap | Where | Consequence |
|---|---|---|---|
| GAP 1 | **Vanilla-id existence is never checked.** `resolve` returns `Vanilla` for *any* `minecraft:*` string | `amc-state/src/reference.rs:67` | `minecraft:doesnotexist` passes validation → invalid recipe/loot ref; only a runtime datapack load would notice, and generated GameTests don't test recipes |
| GAP 2 | **Deleting a concept used by a recipe is not blocked or cascaded.** The delete guard scans *concepts only* | `amc-state/src/manager.rs:562-572` | Stale recipe survives; generator emits a recipe JSON with a bogus `concept_…` id. Runtime-only failure |
| GAP 3 | **Two recipes with the same result silently overwrite.** Recipe file is named after the result path | `amc-generator/src/lib.rs:273-277` | A shaped + a smelting recipe for the same item → second overwrites first, no warning |
| GAP 4 | **Slug → Java identifier legality is unchecked.** Uppercased slug, no identifier validation | `lib.rs:582`, `namespace.rs:11` | e.g. a leading-digit name → uncompilable Java; caught only at Gradle build (classified `GENERATOR`) |
| — | **Generated asset JSON is never schema-validated.** Recipe/model/lang/loot JSON are hand-built strings, emitted unchecked | `recipe_json:323`, `item_model_json:813`, `lang_json:854` | Malformed generated JSON reaches the build/runtime unvalidated |

No external Java analysis exists (no `javac`/JDT/SpotBugs) — generated Java is type-checked **only** by
`compileJava` inside the full Gradle build. The `jsonschema` Rust crate validates the **MCL input model**,
never the **generated Minecraft output**.

---

# Phase 2 — LSP fit assessment

## 5. The two options, honestly compared

### Option (a) — LSP as an additive layer (MDM stays source of truth)

*Build a language server for the MCL files that both a human editor and the AI agents consume; diagnostics
become the structured error channel for the LLM repair loop.*

- **What's right about it:** the *diagnostic channel* idea is the single highest-value piece. The validator
  already produces structured `ValidationError`s; turning them into a stable diagnostic stream that an AI
  agent consumes is exactly the missing feedback edge (§3).
- **What breaks the premise:** there is **no MCL file a human edits**, so "a human editor consumes it" has
  no client on the creator path. The MCL ops array is ephemeral; the persisted `state.json` is
  machine-managed. LSP features that assume a live text document (completion, hover, go-to-def, semantic
  tokens) have nothing to attach to. You would be standing up a *protocol* whose only real client is your
  own agent — which is a strange shape for LSP (that's what MCP or an in-process API is for).
- **Honest verdict:** the *data model* of (a) is worth adopting; **the *server/protocol* of (a) is
  overhead** until there is a human text-editing surface.

### Option (b) — LSP-centric refactor (the language server IS the canonical validator)

*Restructure so the language server is the source of truth for validation; codegen and AI call into it.*

- **This inverts a clean, working architecture.** The canonical validator already exists, is in-process, is
  deterministic, and is called by the CLI and state manager (`SPEC-003 §7`; `amc-validator`). It is
  unit-tested at "Layer 1" of the test pyramid (`SPEC-003 §14`). Making an LSP server the source of truth
  would push a **text-document + async-doc-sync + position** machinery underneath a system whose source of
  truth is a **typed Rust state model**, add latency and a long-running server process to a CLI that is
  currently a pure function of its input, and put the JSON-RPC boundary on the hot path of every generate.
- **Determinism cost:** `SPEC-005 §22` determinism guarantees and byte-identical rebuilds are easy today
  because validation/generation are pure. An LSP server as the arbiter introduces stateful document
  lifecycle that is harder to make byte-reproducible and to test at Layer 1.
- **Honest verdict:** **actively harmful.** It rewrites a strength into a liability and serves no human
  client the product actually has.

## 6. Reuse vs. rewrite, effort, dependencies, risks

**Legend:** S ≈ ≤1 wk, M ≈ 1–3 wk, L ≈ 1+ mo, per component. **[INFERENCE]** on all sizings.

### Option (a) — additive

| Component | Reuse vs new | Effort |
|---|---|---|
| `ValidationError`/`ValidationReport` structs, all Stage 1–6 semantic logic, schemas, CLI `--json` seam | **Reuse as-is** | — |
| Unify error types into an LSP-`Diagnostic`-shaped record (severity, code, source, message, optional range, relatedInformation) | New, small refactor | S–M |
| Refactor validator to **accumulate all findings per op** (today: one-per-op) | Rewrite of the accumulation logic | M |
| **Spanned JSON parse + `path`→`range` mapper** (attach real positions) | New | M |
| Structure build/verify errors (parse Gradle/GameTest logs → diagnostics) | New | M–L |
| Transport: `tower-lsp` server **or** an MCP tool **or** just the existing `--json` | New | S (MCP/JSON) – M (LSP) |

- **New deps:** `tower-lsp` **[EXTERNAL]** (the Rust-native LSP framework — *not* `vscode-languageserver`,
  which is Node) **only if** you build a real server; a spanned JSON parser (e.g. a span-preserving
  `serde_json` variant / `jsonc`-style parser) for positions. A **tree-sitter grammar is not applicable** —
  there is no textual DSL to parse.
- **Risks:** building position machinery for an ops array the user never sees (positions may be meaningless
  in the GUI); maintaining a second error representation in parallel with `ValidationError`; standing up a
  protocol whose only client is your own agent.

### Option (b) — LSP-centric refactor

| Component | Reuse vs rewrite | Effort |
|---|---|---|
| Schemas, some semantic rules | Reuse | — |
| Validator → LSP server as source of truth; async document lifecycle | **Rewrite** | L |
| CLI / state / generator call *into* the server instead of an in-process fn | **Rewrite** call sites | L |
| Determinism/test harness (Layer 1) adapted to a stateful server | Rewrite | M–L |

- **New deps:** LSP server framework + JSON-RPC transport + document-sync layer.
- **Risks:** **high** — inverts a determinism/testability strength; adds a long-running process + latency to
  a pure pipeline; serves no human client. Not recommended.

## 7. How much current validation maps to LSP diagnostics with positions — the crux

- **Structure:** essentially **all** semantic `ValidationError`s already have the non-positional half of an
  LSP `Diagnostic`: `code` → `Diagnostic.code`, `owning_component` → `Diagnostic.source`,
  `message` → `Diagnostic.message`, and severity is derivable (errors=Error, `Clarification`=Info/Hint,
  `Unsupported`=Warning). Good news.
- **Positions: none exist. Every validator lacks line/column/byte-offset.** The richest location any finding
  carries is `operation_index: usize` + a hand-authored dotted `path: String`
  (`amc-validator/src/lib.rs:27-35`). `Clarification`/`Unsupported` carry only `operation_index`. The
  jsonschema `instance_path` (a real JSON pointer) exists transiently but is `format!`'d into a string and
  **discarded** (`amc-schema/src/lib.rs:98-101`). **This is the entire retrofit cost.**
- **The bridge:** because `path` strings already resemble JSON pointers (`payload.recipe.key.R`,
  `properties.durability`), the lowest-cost retrofit is: re-parse the operations array (or `state.json`)
  with a **span-preserving** JSON parser, then map `(operation_index, path)` → a source `range`. Caveats
  from the code: (1) the `path` grammar is **ad-hoc** — it mixes `.` and `[i]` and sometimes omits the
  `payload.` prefix (rule #4 `payload.display_name` vs rule #21 `properties.field`), so it must be
  normalized first; (2) the **one-finding-per-op** short-circuit must be removed to publish complete
  diagnostics.
- **Bottom line:** ~100% of the *diagnostic content* is reusable; **0% of position info exists today**, and
  building the `path`→`range` mapper over a spanned parse is the main (M-sized) retrofit — worth doing only
  against a document a client actually renders (the generated JSON, or a future MCL text view — *not* the
  invisible ops array).

## 8. Integrating existing ecosystem servers instead of rebuilding

The place these pay off is **the generated artifacts**, not the MCL input.

- **JSON Schema for asset files [EXTERNAL/low-cost]:** the generated recipe/model/lang/loot JSON are
  emitted **unvalidated** (§4). The `jsonschema` Rust crate is **already a dependency** (`amc-schema`).
  Bundling Minecraft data-format schemas and validating generated JSON *before* the Gradle build is a cheap,
  high-value static gate — and needs **no LSP and no new runtime**. **This is the single best "borrow,
  don't build" win.**
- **Spyglass (MIT) for the datapack/mcfunction layer [EXTERNAL]:** Spyglass is a Minecraft datapack
  language server + lint library that understands resource-location existence, recipe/loot/advancement
  schemas, and mcfunction. It could validate the generated **datapack** (recipes, loot tables, structures)
  and directly close **GAP 1** (vanilla-id existence). Caveat: it is **Node/TypeScript**, so integrating it
  means bundling a **Node runtime** and spawning it as an LSP client from Rust — which **conflicts with the
  product's "self-contained, no-Python, no-extra-runtime" ethos** (see the desktop bundle, which ships only
  a static `llama-server` + `amc`). **[INFERENCE]** for the *few* checks you actually need, extracting the
  relevant vanilla-id list into a bundled data file and checking it in Rust is lighter than adopting Node.
- **Eclipse JDT.LS for generated Java [EXTERNAL]:** JDT.LS gives Java compile diagnostics with positions
  *without* a full Gradle build — but it needs the project's classpath (NeoForge deps) resolved, which is
  ~as heavy as the Gradle build the pipeline already runs, plus a JVM language-server process. Marginal
  value on the creator path (the user never reads Java); potentially useful on a **developer/debug surface**.
- **What a thin orchestrator would look like here:** a new `crates/amc-diagnostics` (or a subcommand) that,
  post-generation, (1) validates generated asset JSON against bundled Minecraft schemas in-process, and
  (2) optionally shells out to Spyglass/JDT.LS *only in a developer mode*, then **normalizes all results
  into the one `Diagnostic` shape** and emits them on the existing `--json` seam. The orchestrator is the
  value; the external servers are optional back-ends behind it.

## 9. Exposing the language server to the AI agents

- **What changes in the current feedback path:** today the path is *one-shot intent → validate/build/verify
  → terminate at the user* (§3); nothing re-enters the model. Adding a structured-diagnostics channel lets
  you build the **repair loop that doesn't exist**: `emit ops → diagnose → if errors, re-prompt the model
  with the structured diagnostics → retry (bounded)`. This is where the unused `repairable` flag was always
  meant to be consumed.
- **MCP wrapper vs. direct LSP client:** since the agent loop is **in-process Rust** and the desktop already
  speaks to the engine via the **CLI `--json`** contract, the lighter integration is an **MCP tool** (e.g.
  `amc_diagnose(ops|state) → [Diagnostic]`) or a direct in-process call — **not** a full LSP client. A real
  LSP client in the agent loop only earns its keep if you're *also* driving JDT.LS/Spyglass for the
  generated artifacts.
- **Expected win, sized honestly:** structured diagnostics (code + normalized path + message + severity)
  are a strictly better repair signal than today's free-text intent errors and raw `log_tail`. **But** the
  marginal win is **bounded by how often the LLM is even on the path** — the deterministic fast-path already
  handles the common create/update/rename/delete/recipe cases (§3), so a model-repair loop mostly helps the
  **long-tail fuzzy requests**. **[INFERENCE]** the bigger, more certain win is using diagnostics to catch
  the **cross-file gaps (§4)** *before* the build — which helps every request, deterministic or not.

---

# Phase 3 — Recommendation

## 10. Recommend: **neither (a) nor (b)** — adopt the diagnostic *model*, not the *server*

**Reasoning, tied to findings:**

1. **The product has no human text-editor client** (creators type English in a GUI; MCL ops are ephemeral;
   `state.json` is machine-managed) — so LSP-the-protocol's core features have nothing to attach to on the
   creator path (§1, §5a). A tree-sitter grammar "for the DSL" is grammar for a non-existent language.
2. **Option (b) inverts a genuine strength.** The canonical validator is already in-process, deterministic,
   layered, and Layer-1 tested (§2, §5b). Making an LSP server the source of truth adds async document
   state and a JSON-RPC hot-path for zero human benefit.
3. **The valuable, missing pieces are not LSP-shaped in delivery, only in *data*:** a structured diagnostic
   record (LSP `Diagnostic`), the **cross-file integrity checks** (§4 GAP 1–4 + generated-JSON validation),
   and a **structured AI-repair edge** (§3, §9). All three are deliverable at the existing CLI `--json`
   seam + an MCP tool, reusing ~all of the validator.

**So: adopt LSP's `Diagnostic` data model as the internal error contract; do not stand up an LSP server for
MCL.** Keep MCL/state as source of truth. Reserve real LSP servers (JDT.LS, Spyglass, JSON-schema LS) for a
*future developer/debug surface over the generated artifacts*, behind the thin orchestrator in §8, **only if**
a text-editing workflow appears.

## Minimal first milestone — "Structured Diagnostics v0" (diagnostics-only, no server)

A diagnostics-only capability that unifies error output and adds the five highest-value cross-file checks —
**no LSP server, no new runtime.** Deliberately scoped to prove value before any protocol investment.

**Deliverables:**
1. A single `Diagnostic { severity, code, source, message, operation_index, path, range: Option<Range> }`
   type (LSP-shaped). `range` starts `None`; a later phase adds a span-preserving re-parse + `path`→`range`
   mapper.
2. Refactor the validator to **accumulate all findings per op** (remove the one-per-op short-circuit) and
   emit `Vec<Diagnostic>`.
3. **The five highest-value cross-file checks:**
   - **C1 — Vanilla-id existence** (closes GAP 1): a `minecraft:*` reference must be in a bundled vanilla
     registry-id set. *Touches:* `amc-state/src/reference.rs` (`resolve`), a new bundled data file of
     vanilla ids, `amc-validator`.
   - **C2 — Recipe survives concept deletion** (closes GAP 2): extend the delete guard to scan **recipes**,
     and either block the delete or cascade-delete dependent recipes. *Touches:*
     `amc-state/src/manager.rs` (`find_referrer` / delete path).
   - **C3 — Duplicate recipe result** (closes GAP 3): reject/warn when two active recipes resolve to the
     same result path before generation. *Touches:* `amc-validator` or
     `amc-generator/src/lib.rs:273-277`.
   - **C4 — Java-identifier legality** (closes GAP 4): validate that a concept's slug yields a legal Java
     identifier (no leading digit, no collision) at create time. *Touches:* `amc-state/src/namespace.rs`,
     `amc-validator`.
   - **C5 — Generated asset-JSON validity**: validate generated recipe/model/lang/loot JSON against bundled
     Minecraft schemas *before* the Gradle build, using the already-present `jsonschema` crate. *Touches:* a
     new `crates/amc-diagnostics` (or a generator post-pass), bundled schemas.
4. Surface all of the above on the existing CLI `--json` contract (`amc-cli/src/main.rs` `finish`), and add
   an **MCP tool `amc_diagnose`** so agents consume the same structured stream.

**Explicitly out of scope for v0** (revisit only if a text-editing surface emerges): a `tower-lsp` server,
tree-sitter grammar, JDT.LS/Spyglass integration, source-range positions, and any model-repair loop (build
the structured channel first; wire the loop second, once C1–C5 prove the diagnostic contract).

**Files the milestone touches:** `crates/amc-validator/src/lib.rs`, `crates/amc-state/src/{manager,reference,namespace}.rs`,
`crates/amc-generator/src/lib.rs`, `crates/amc-core/src/error.rs` (or a new `crates/amc-diagnostics`),
`crates/amc-cli/src/main.rs`, plus new bundled data (vanilla-id set, Minecraft asset schemas).

---

# Phase 4 — Growth scenario: bridging no-code creators and classic modders (addendum, 2026-07-21)

*Added in response to a follow-up: consider the future where the no-code tool is a **gateway** — users
**start** a mod here, then graduate into **editing the real code** and installing it like classic modders
always did. Does an LSP make sense as the backend that enables this bridge?*

**Short answer: yes — this is the scenario that flips the recommendation.** The base assessment argued
against LSP mainly because *no human edits text*. The bridge **creates that human**, and it creates them
over exactly the artifacts where mature LSPs already excel (NeoForge Java, Minecraft datapack/asset JSON).
**But** the bridge's make-or-break is **not** the language server — it's a **source-of-truth / round-trip
problem** the current generator design actively fights, and **LSP does nothing to solve that**. So: LSP
becomes a *recommended track for the bridge* — as **orchestration of existing ecosystem servers over the
generated code**, contingent on first designing the ownership model. A bespoke *MCL* language server is
still not the play.

## 11. The user-tier spectrum this introduces

| Tier | Who | Editing surface | LSP relevance |
|---|---|---|---|
| T0 — Creator | today's target: non-programmer, English + GUI | none (MCL is invisible) | ~none (base assessment) |
| T1 — Tinkerer | curious; wants to *peek* and tweak a value | reads generated code; small guided edits | **medium** — hover/diagnostics teach; read-mostly |
| T2 — Graduate | outgrowing the no-code ceiling (e.g. wants real custom behavior) | **owns and edits** generated Java + datapack JSON | **high** — full JDT.LS/Spyglass value |
| T3 — Classic modder | already a modder; may import/extend | a normal NeoForge project | **high**, but standard tooling already serves them |

The strategic bet is moving people **T0 → T1 → T2** without a cliff. **[INFERENCE]** this mirrors the
project's existing *tiered* instinct (ADR-0007 tiers the model by difficulty); here you tier the **user**.

## 12. Why the bridge strengthens the LSP case — and exactly which LSP, over what

The single biggest asset for this bridge is already true in the code: **the generated project at
`<project>/mod/` is a complete, standalone NeoForge Gradle project** (it emits `gradlew`, `build.gradle`,
`settings.gradle`, `src/main/java/**`, and a datapack under `src/main/resources/data/**` — `build_files`,
`amc-generator/src/lib.rs:162-281`). So "graduating" a user is *structurally* almost free: hand them
`<project>/mod/` and it opens, builds, and runs like any mod project.

That means the T2/T3 editing surface is **standard Minecraft-modding tooling**, which already has mature,
mostly-MIT/EPL language servers — **you would borrow, not build**:

- **Eclipse JDT.LS [EXTERNAL]** over `src/main/java/**` → real Java diagnostics, completion, hover,
  go-to-def, rename — the whole classic-IDE experience for the generated NeoForge code. (This is the value
  that was "marginal on the creator path" in §8 and becomes **first-class** for T2/T3.)
- **Spyglass (MIT) [EXTERNAL]** over `src/main/resources/data/**` → datapack-aware editing of the generated
  recipes/loot/structures with resource-location awareness (also closes GAP 1). Its Node runtime cost
  (§8) is far easier to justify for a *desktop developer surface* than for the shipped no-code app.
- **JSON Schema [in-repo dep]** over generated model/blockstate/lang JSON → completion + validation with the
  `jsonschema` crate already present.

A **bespoke MCL language server is still not warranted** even here: the human is editing *generated Java and
datapack JSON*, not MCL. The only MCL-specific tooling worth adding is a thin **context/provenance provider**
(“this file was generated from concept *Ruby Blade*; here's the MCL that produced it”) layered on top of the
stock servers.

## 13. The real obstacle — and it is NOT the language server: round-trip drift

This is the crux, and it is a **source-of-truth** problem the current design is structurally hostile to:

- **Generation is a full re-materialization from canonical state, and it prunes.** `generate()` writes the
  entire file set from `CanonicalState`, then `prune_stale()` **deletes any file under `src/main/java` or
  `src/main/resources` that isn't in the freshly generated set** (`MANAGED_ROOTS`,
  `amc-generator/src/lib.rs:111`; `prune_stale`, `:116`). This is *exactly* the behavior that makes the
  no-code loop correct (a removed recipe truly disappears) — and it is *exactly* what will **silently
  destroy a graduating user's hand-edits** the next time they run one more no-code request.
- So the moment a user crosses into T2 and edits generated code, the two sources of truth (canonical
  `state.json` vs. their edited files) **diverge, and the current pipeline resolves that divergence by
  obliterating the edits.** An LSP gives them a lovely editor for files the tool will then delete. **LSP
  does not touch this problem at all.**
- **Good news — a drift-*detection* primitive already exists.** The generator emits a **generation
  manifest with per-file hashes** (`GenerationManifest::build`, `manifest.rs`; `Generated.manifest`,
  `lib.rs:100-104`). Comparing on-disk file hashes to the manifest before regenerating turns "silent
  destruction" into "detect the user touched this file → warn / merge / refuse to prune." That primitive is
  the seed of any safe bridge and is nearly free to exploit.

**Therefore the ordering is non-negotiable: design the ownership/round-trip model first; add LSP second.**
LSP is the *easy, mostly-solved* half (borrow JDT.LS/Spyglass). The hard, bespoke half is reconciling
hand-edits with regeneration — and it must be solved *before* you invite users to edit, or you train them to
lose work.

## 14. Three bridge models (and their LSP implications)

- **(A) Eject / graduate (one-way, à la `create-react-app eject`).** At an explicit "I want to code this"
  moment, freeze the project: stop the no-code pipeline for it, hand over `<project>/mod/`, and never
  regenerate/prune again. *Cleanest and safest.* The generated project is already standalone, so the eject
  is nearly a no-op structurally. **LSP fit:** trivial — post-eject it's a normal NeoForge project; point
  VS Code + JDT.LS/Spyglass at it. **Cost:** the user loses the no-code tool for that project (acceptable
  as a v1; it's how most low-code tools handle this).
- **(B) Hybrid coexistence (two-way; the hard, high-value path).** Keep the no-code tool live while the user
  hand-edits. Requires a **strict ownership boundary**: generated-and-owned files (freely re-materialized +
  pruned) vs. user-owned files that `prune_stale` **must never touch**. Concrete mechanisms available in
  this codebase: (1) carve a never-pruned zone (add a user source set / a `…/custom/**` package excluded
  from `MANAGED_ROOTS`); (2) use **protected regions** — generate class stubs with clearly-marked
  user-editable method bodies that regeneration preserves (a classic codegen pattern; a natural first target
  is **custom behavior**, since `behavior.schema.json` is deliberately enums-only/"no arbitrary code" —
  the graduation surface *is* the escape hatch from that restriction); (3) use the manifest hashes (§13) to
  detect and refuse to clobber user-touched generated files. **LSP fit:** highest value *and* highest
  complexity — the AI + human + generator all operate on one tree, and LSP diagnostics become the shared
  language among them.
- **(C) Fork / import (T3).** Let a classic modder bring their own project or fork a generated one, with the
  no-code tool as an occasional assistant. **LSP fit:** they already have their IDE; your value is the AI
  agent reading the same LSP diagnostics (§15), not the servers themselves.

**[INFERENCE]** Recommended sequence: ship **(A) eject** first (safe, cheap, unlocks T2/T3 immediately with
stock LSP tooling), then invest in **(B)** only once the ownership boundary + manifest-drift handling are
proven.

## 15. The AI angle gets much stronger in the bridge

Recall the base finding: **there is no AI repair loop today** (§3), and the deterministic fast-path already
covers the common no-code cases (so a *no-code* repair loop has bounded value). In the bridge, that
calculus inverts:

- Once a human is editing **real code**, an AI agent that **reads the same LSP diagnostics** (JDT.LS
  compile errors, Spyglass datapack errors — all normalized into the `Diagnostic` shape from §10) becomes a
  genuine **pair-programmer for the graduating user**: "you renamed the item but the recipe still points at
  the old id — here's the fix." This is a differentiated capability precisely at the T1→T2 hand-off where
  users are most likely to get stuck and churn.
- Integration shape is unchanged from §9: the agent consumes diagnostics via an **MCP tool / the CLI
  `--json` seam**, not by being an LSP client itself — *unless* it needs live JDT.LS/Spyglass sessions, in
  which case a real **LSP client in the agent** earns its keep for the first time. This is the one scenario
  in the whole assessment where "AI as a direct LSP client" is justified.

## 16. Revised recommendation for the growth path

The base recommendation (§10) stands for the **no-code product**: adopt the diagnostic *model*, don't build
an MCL server. The **bridge adds a second, parallel track**:

> **Adopt LSP for the generated artifacts by orchestrating existing ecosystem servers (JDT.LS + Spyglass +
> JSON-schema) behind a thin provenance-aware layer — but gate it behind a solved ownership/round-trip
> model. Do not build a bespoke MCL language server. Sequence: eject (one-way) first; hybrid coexistence
> only after the ownership boundary + manifest-drift detection exist.**

What changes vs. the base report:
- JDT.LS / Spyglass move from a footnoted "future dev surface" to a **first-class, recommended track** (for
  T2/T3).
- "AI as a direct LSP client" moves from *unjustified* to *justified* — but only in the bridge (§15).
- A new, higher-priority-than-LSP workstream appears: **the ownership/round-trip model** (§13–14). This is
  the actual long-pole; LSP is downstream of it.

What does **not** change: no bespoke MCL LSP; MCL/state remains source of truth for the no-code loop; the
`Diagnostic` shape remains the unifying contract across validator, JDT.LS, and Spyglass.

## 17. Added milestones for the bridge

Sequenced after "Structured Diagnostics v0" (§10):

- **Milestone B1 — "Graduate / Eject" (model A).** A `amc graduate <project>` (or GUI button) that marks a
  project hand-owned: stop regenerating/pruning it, print/emit VS Code + JDT.LS + Spyglass setup, and record
  the eject in state. *Touches:* `crates/amc-cli/src/main.rs` (new subcommand), `crates/amc-cli/src/project.rs`
  (a "graduated" flag in `.amc/state.json`), the generator's entry (`generate()` becomes a no-op / refuses
  to prune a graduated project — `amc-generator/src/lib.rs:75-105`). **Low effort, high unlock; no LSP code
  of our own — we hand off to stock servers.**
- **Milestone B2 — Ownership boundary + drift detection (prerequisite for model B).** Carve a never-pruned
  user zone and/or protected regions; exploit `GenerationManifest` hashes to detect user-touched generated
  files and refuse to clobber them (warn/merge instead). *Touches:* `MANAGED_ROOTS` / `prune_stale`
  (`amc-generator/src/lib.rs:111,116`), `manifest.rs`, generator emitters (protected-region markers). **This
  is the real bespoke work — do it before inviting two-way editing.**
- **Milestone B3 — Diagnostic orchestrator over generated artifacts.** A thin layer that runs JDT.LS
  (Java) + Spyglass or `jsonschema` (datapack/assets), normalizes results into the `Diagnostic` shape, adds
  MCL provenance, and exposes them on the `--json`/MCP seam for both the graduating human and the AI agent.
  *Touches:* a new `crates/amc-diagnostics` orchestrator; optional bundled Node for Spyglass (weigh against
  the no-extra-runtime ethos — JSON-schema-in-Rust may suffice for v1).

## 18. Bridge-specific risks

- **Work-destruction (severe):** shipping any two-way editing before B2 trains users to lose edits to
  `prune_stale`. Eject (B1) sidesteps this entirely and should ship first.
- **Two-source-of-truth complexity (model B):** reconciling `state.json` ↔ hand-edited code is genuinely
  hard; protected-region merges and partial regeneration are a known maintenance tax.
- **Runtime bloat:** Spyglass drags in Node; JDT.LS drags in a JVM language-server process. Fine for an
  opt-in developer surface, at odds with the lean no-code bundle — keep them **out of the default app**,
  behind graduation.
- **Support-surface widening:** the moment users edit code, "it doesn't build" becomes your problem across
  the whole NeoForge/Java/Gradle surface, not just MCL. The AI-diagnostic pair-programmer (§15) is partly a
  mitigation, not just a feature.
- **[INFERENCE] Scope creep away from the wedge:** the no-code creator (T0) is the differentiated product;
  the bridge is valuable but competes for effort with a market (classic modding IDEs) that already has good
  tooling. Recommend the *cheap* bridge (eject + stock servers + AI diagnostics) before the *expensive* one
  (hybrid coexistence).

---

### Appendix — provenance

Investigation used three parallel read-only code passes (DSL/MCL + validator; AI/error loop; generator +
verification + integrity) plus a direct read of `SPEC-003` (System Architecture), `SPEC-004` (MCL
Specification), and ADR-0002/0007/0008. Phase 4 (2026-07-21 addendum) reasons from the same code findings —
notably `prune_stale`/`MANAGED_ROOTS` (`amc-generator/src/lib.rs:111,116`), the standalone generated Gradle
project (`build_files`), and `GenerationManifest` hashes (`manifest.rs`) — applied to the no-code→code
bridge scenario. Inference vs. read is tagged inline. No files were modified during this assessment.
