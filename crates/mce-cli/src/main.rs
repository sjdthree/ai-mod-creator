//! The `mce` command-line interface (`SPEC-003 §3`).
//!
//! The CLI is the user-facing entry point and orchestration layer. It exposes the full command
//! surface from the acceptance scenario (`SPEC-005 §31`) — `init`, `request`, `test`, `play`,
//! `revert` — plus `bench` (`SPEC-005 §25`). `request` chains the whole pipeline: intent → validate
//! → state → generate → build → verify → commit, and commits canonical state **only** when
//! verification passes (`SPEC-005 §18` — nothing is ever mistaken for success).
//!
//! Prohibited (`SPEC-003 §3`): the CLI must not itself interpret intent, mutate canonical state,
//! generate code, or determine gameplay correctness — it *orchestrates* the components that do.

mod project;
mod runner;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use mce_bench::{run_suite, BenchOptions};
use mce_build::{BuildManager, GradleBuildManager};
use mce_generator::{generator_for, Loader};
use mce_intent::{HttpIntentAdapter, IntentAdapter, ModelManifest, MANIFEST_FILE};
use mce_launch::{GradleLauncher, Launcher};
use mce_state::CanonicalState;
use serde_json::{json, Value};

use runner::GameTestLevelRunner;

/// Exit code for a runtime error.
const EXIT_ERROR: u8 = 1;

/// Default OpenAI-compatible endpoint of the promoted intent model (local `llama-server`).
const DEFAULT_INTENT_ENDPOINT: &str = "http://127.0.0.1:8080/v1/chat/completions";
const DEFAULT_INTENT_MODEL: &str = "default_model";
// The intent model's provenance defaults to the model pinned in `model.lock`. It lands in every
// generated mod's state, so it's user-visible.

#[derive(Parser, Debug)]
#[command(
    name = "mce",
    version,
    about = "Mod Creation Engine — build Minecraft mods from plain English."
)]
struct Cli {
    /// Emit machine-readable JSON instead of human-readable text.
    #[arg(long, global = true)]
    json: bool,

    /// Mod loader to target: `neoforge` (default, ADR-0003) or `fabric` (scaffold, ADR-0007). Falls
    /// back to `$MCE_LOADER`, then `neoforge`.
    #[arg(long, global = true)]
    loader: Option<String>,

    /// Intent model endpoint (OpenAI-compatible). Falls back to `$MCE_INTENT_ENDPOINT`, then a
    /// local `llama-server`. From inside the dev container, point at `host.docker.internal`.
    #[arg(long, global = true)]
    intent_endpoint: Option<String>,
    /// Served model id for the intent endpoint (`$MCE_INTENT_MODEL`).
    #[arg(long, global = true)]
    intent_model: Option<String>,
    /// Promoted intent model version recorded in the transaction (`$MCE_INTENT_VERSION`).
    #[arg(long, global = true)]
    intent_version: Option<String>,

    #[command(subcommand)]
    command: Command,
}

/// Resolved intent-model connection (flag → env → default).
struct IntentConfig {
    endpoint: String,
    model: String,
    version: String,
}

impl IntentConfig {
    fn resolve(cli: &Cli) -> Self {
        let pick = |flag: &Option<String>, env: &str, default: &str| {
            flag.clone()
                .or_else(|| std::env::var(env).ok())
                .unwrap_or_else(|| default.to_string())
        };
        let locked = mce_intent::locked_model().name;
        IntentConfig {
            endpoint: pick(
                &cli.intent_endpoint,
                "MCE_INTENT_ENDPOINT",
                DEFAULT_INTENT_ENDPOINT,
            ),
            model: pick(&cli.intent_model, "MCE_INTENT_MODEL", DEFAULT_INTENT_MODEL),
            version: pick(&cli.intent_version, "MCE_INTENT_VERSION", &locked),
        }
    }

    fn adapter(&self) -> HttpIntentAdapter {
        HttpIntentAdapter::new(&self.endpoint, &self.model, &self.version)
    }
}

/// Resolve the target loader: `--loader` flag → `$MCE_LOADER` → default (NeoForge, ADR-0003).
fn resolve_loader(cli: &Cli) -> mce_core::Result<Loader> {
    match cli
        .loader
        .clone()
        .or_else(|| std::env::var("MCE_LOADER").ok())
    {
        Some(name) => name.parse(),
        None => Ok(Loader::default()),
    }
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Create a new project.
    Init {
        /// Project name, e.g. `ruby_world`.
        project_name: String,
    },
    /// Describe a creation or modification in English.
    Request {
        /// The English request.
        text: String,
        /// Project path.
        #[arg(long, default_value = ".")]
        project: PathBuf,
        /// Build the mod but skip the in-game GameTest verification (faster; commits on build only).
        #[arg(long)]
        no_verify: bool,
        /// Scope this request as an UPDATE to an existing creation (by display name), so an edit like
        /// "do 12 damage" targets that concept instead of trying to create a new one with the same name.
        #[arg(long)]
        refine: Option<String>,
    },
    /// Verify the current project: regenerate from canonical state, build, and run GameTests.
    Test {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Launch the pinned Minecraft client (after verification gates pass).
    Play {
        #[arg(long, default_value = ".")]
        project: PathBuf,
        /// Run a visible, interactive client (foreground) instead of the bounded headless smoke.
        #[arg(long)]
        interactive: bool,
    },
    /// Revert to a previously verified transaction.
    Revert {
        /// Target transaction id, e.g. `tx-000001`.
        transaction_id: String,
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Undo the most recent change (revert to the previous committed transaction).
    Undo {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// List the project's creations (concepts) and their key properties.
    List {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
    /// Export the built mod JAR into a Minecraft mods folder so you can play it for real.
    Export {
        #[arg(long, default_value = ".")]
        project: PathBuf,
        /// Target mods directory (defaults to the standard Minecraft mods folder for this OS).
        #[arg(long)]
        to: Option<PathBuf>,
    },
    /// Validate operations against a state, without a model.
    ///
    /// Reads `{ "namespace": "...", "setup": [ops], "operations": [ops] }` JSON from a file or
    /// stdin (`-`); applies `setup` to build the initial state, then validates `operations` and
    /// (if valid) applies them, emitting a JSON verdict. Useful for testing
    /// hand-written operations against the real engine.
    Validate {
        /// Input JSON file, or `-` for stdin.
        #[arg(default_value = "-")]
        input: String,
    },
    /// Package and check MCE Model builds (`mce-model.json` manifests).
    Model {
        #[command(subcommand)]
        action: ModelAction,
    },
    /// Run a benchmark suite.
    Bench {
        /// Path to the benchmark corpus (or a subtree).
        corpus: PathBuf,
        /// Directory to write the report into (defaults to `<corpus>/reports`).
        #[arg(long)]
        report_dir: Option<PathBuf>,
        /// Restrict to a single category.
        #[arg(long)]
        category: Option<String>,
        /// Restrict to a single case id.
        #[arg(long)]
        case: Option<String>,
        /// Stop at the first failing/errored case.
        #[arg(long)]
        fail_fast: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ModelAction {
    /// Write `mce-model.json` next to a GGUF, pinning it to this engine's contract and grammar.
    Manifest {
        /// The model weights (GGUF).
        gguf: PathBuf,
        /// Model name recorded as provenance (defaults to the model pinned in `model.lock`).
        #[arg(long)]
        name: Option<String>,
        /// Model release version, e.g. `1.0.0`.
        #[arg(long)]
        version: String,
        /// Upstream base model the weights derive from.
        #[arg(long, default_value = "google/functiongemma-270m-it")]
        base_model: String,
        /// License identifier governing the weights.
        #[arg(long, default_value = "gemma")]
        license: String,
    },
    /// Check a model directory's manifest against this engine (contract, grammar, weights hash).
    Verify {
        /// Directory containing `mce-model.json` and the weights it names.
        dir: PathBuf,
        /// Skip re-hashing the weights file (fast startup check).
        #[arg(long)]
        skip_weights: bool,
    },
}

fn main() -> ExitCode {
    mce_core::init_logging();
    let cli = Cli::parse();
    let as_json = cli.json;
    let intent = IntentConfig::resolve(&cli);
    let loader = match resolve_loader(&cli) {
        Ok(l) => l,
        Err(e) => return fail(as_json, "loader", e.to_string()),
    };
    match &cli.command {
        Command::Validate { input } => run_validate(input),
        Command::Bench {
            corpus,
            report_dir,
            category,
            case,
            fail_fast,
        } => run_bench(
            as_json,
            corpus.clone(),
            report_dir.clone(),
            category.clone(),
            case.clone(),
            *fail_fast,
        ),
        Command::Init { project_name } => run_init(as_json, project_name),
        Command::Request {
            text,
            project,
            no_verify,
            refine,
        } => run_request(
            as_json,
            text,
            project,
            &intent,
            !no_verify,
            refine.as_deref(),
            loader,
        ),
        Command::Test { project } => run_test(as_json, project, loader),
        Command::Play {
            project,
            interactive,
        } => run_play(as_json, project, *interactive, loader),
        Command::Revert {
            transaction_id,
            project,
        } => run_revert(as_json, transaction_id, project, loader),
        Command::Undo { project } => run_undo(as_json, project, loader),
        Command::List { project } => run_list(as_json, project),
        Command::Export { project, to } => run_export(as_json, project, to.as_deref()),
        Command::Model { action } => run_model(as_json, action),
    }
}

/// `mce model manifest|verify` — the engine side of the engine/model compatibility contract.
fn run_model(as_json: bool, action: &ModelAction) -> ExitCode {
    match action {
        ModelAction::Manifest {
            gguf,
            name,
            version,
            base_model,
            license,
        } => {
            let name = name
                .clone()
                .unwrap_or_else(|| mce_intent::locked_model().name);
            let manifest = match ModelManifest::for_gguf(gguf, &name, version, base_model, license)
            {
                Ok(m) => m,
                Err(e) => return fail(as_json, "model", format!("{}: {e}", gguf.display())),
            };
            let out = gguf
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(MANIFEST_FILE);
            let text = serde_json::to_string_pretty(&manifest).unwrap() + "\n";
            if let Err(e) = std::fs::write(&out, text) {
                return fail(as_json, "model", format!("{}: {e}", out.display()));
            }
            if as_json {
                let payload = json!({ "status": "ok", "command": "model",
                    "manifest": out.display().to_string(), "model": manifest });
                println!("{}", serde_json::to_string_pretty(&payload).unwrap());
            } else {
                println!("wrote {}", out.display());
            }
            ExitCode::SUCCESS
        }
        ModelAction::Verify { dir, skip_weights } => {
            let manifest = match ModelManifest::load(dir) {
                Ok(m) => m,
                Err(e) => return fail(as_json, "model", e),
            };
            let problems = manifest.verify(dir, !skip_weights);
            if as_json {
                let status = if problems.is_empty() { "ok" } else { "error" };
                let payload = json!({ "status": status, "command": "model",
                    "name": manifest.name, "version": manifest.version, "problems": problems });
                println!("{}", serde_json::to_string_pretty(&payload).unwrap());
            } else if problems.is_empty() {
                println!(
                    "{} {} is compatible with this engine",
                    manifest.name, manifest.version
                );
            } else {
                for p in &problems {
                    eprintln!("mce model: {p}");
                }
            }
            if problems.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(EXIT_ERROR)
            }
        }
    }
}

// ---- pipeline orchestration -----------------------------------------------------------------

/// Emit a structured error and fail with the runtime-error exit code.
fn fail(as_json: bool, command: &str, message: impl std::fmt::Display) -> ExitCode {
    if as_json {
        let payload =
            json!({ "status": "error", "command": command, "message": message.to_string() });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap());
    } else {
        eprintln!("mce {command}: {message}");
    }
    ExitCode::from(EXIT_ERROR)
}

/// `mce init <name>` — scaffold a new project with an empty canonical state.
fn run_init(as_json: bool, project_name: &str) -> ExitCode {
    let dir = PathBuf::from(project_name);
    match project::init(&dir, project_name) {
        Ok(()) => {
            let namespace = project::namespace_for(project_name);
            if as_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "status": "ok", "command": "init",
                        "project": dir.display().to_string(), "namespace": namespace,
                    }))
                    .unwrap()
                );
            } else {
                println!(
                    "initialized MCE project '{project_name}' ({namespace}) at {}",
                    dir.display()
                );
                println!(
                    "next: mce request \"<describe your creation>\" --project {}",
                    dir.display()
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => fail(as_json, "init", e),
    }
}

/// The compact project context the intent model was trained on: existing concepts by display name.
fn build_context(state: &CanonicalState, refine: Option<&str>) -> Value {
    let concepts: Vec<Value> = state
        .active_concepts()
        .map(|c| json!(c.display_name))
        .collect();
    let mut ctx = json!({ "concepts": concepts });
    // The refine target (the creation the user is updating) rides along in the context so the intent
    // adapter can scope the request to it — an edit to an existing concept, never a duplicate create.
    if let Some(name) = refine {
        ctx["refine_target"] = json!(name);
    }
    ctx
}

/// Stamp `schema_version` onto a model-proposed operation if missing (system envelope completion).
fn stamp_schema_version(mut op: Value) -> Value {
    if let Some(obj) = op.as_object_mut() {
        obj.entry("schema_version")
            .or_insert_with(|| json!(mce_schema::SCHEMA_VERSION));
    }
    op
}

/// `mce request "<english>"` — the full pipeline. Commits canonical state only if verification
/// passes; a failed candidate leaves the prior verified state untouched (`SPEC-004 §31`).
/// Capitalize the name that follows `called`/`named` — proper nouns the intent model expects
/// capitalized. A lowercase name (e.g. "called sting") makes the small model misfire (it tries to
/// spell the letters into a recipe). Capitalizes following words until a stop-word, a number, or end.
fn normalize_names(text: &str) -> String {
    const STOP: &[&str] = &[
        "with", "that", "and", "for", "which", "having", "at", "of", "to", "made", "so", "but",
    ];
    let mut out: Vec<String> = Vec::new();
    let mut capitalizing = false;
    for tok in text.split_whitespace() {
        let bare = tok
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase();
        if bare == "called" || bare == "named" {
            out.push(tok.to_string());
            capitalizing = true;
            continue;
        }
        if capitalizing
            && !bare.is_empty()
            && !STOP.contains(&bare.as_str())
            && !tok.chars().any(|c| c.is_ascii_digit())
        {
            out.push(capitalize_first(tok));
        } else {
            capitalizing = false;
            out.push(tok.to_string());
        }
    }
    out.join(" ")
}

fn capitalize_first(w: &str) -> String {
    let mut chars = w.chars();
    match chars.next() {
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Classify a committed transaction's operations into a user-facing action verb, so the success
/// message/banner reads correctly ("updated"/"deleted"/"renamed") instead of always "created". A
/// create wins if present (the headline act); otherwise a delete, a rename (an update that changes the
/// display name), or a plain update.
fn action_verb(ops: &[Value]) -> &'static str {
    let is = |name: &str| {
        ops.iter()
            .any(|o| o.get("operation").and_then(Value::as_str) == Some(name))
    };
    if is("create_concept") {
        "created"
    } else if is("delete_concept") {
        "deleted"
    } else if ops.iter().any(|o| {
        o.get("operation").and_then(Value::as_str) == Some("update_concept")
            && o.pointer("/payload/changes/display_name").is_some()
    }) {
        "renamed"
    } else {
        "updated"
    }
}

fn run_request(
    as_json: bool,
    text: &str,
    project: &Path,
    intent: &IntentConfig,
    do_verify: bool,
    refine: Option<&str>,
    loader: Loader,
) -> ExitCode {
    // 1. Load persisted canonical state.
    let mut sm = match project::load(project) {
        Ok(sm) => sm,
        Err(e) => return fail(as_json, "request", e),
    };

    // 2. Interpret English into operations against compact project context. Names are capitalized
    //    first (a lowercase name confuses the small model into spelling it out as a recipe).
    let normalized = normalize_names(text);
    let context = build_context(sm.canonical(), refine);
    let result = match intent.adapter().interpret(&normalized, &context) {
        Ok(r) => r,
        // The model produced nothing usable (rare, but small models slip). Guide, don't dump JSON.
        Err(_) => {
            return finish(
                as_json,
                "request",
                false,
                json!({
                    "status": "needs_clarification",
                    "message": "I couldn't turn that into a creation. Try giving it a Capitalized name \
                                and being specific — e.g. \"a ruby sword called Sting with 8 attack damage\".",
                }),
            )
        }
    };
    if result.needs_clarification {
        return finish(
            as_json,
            "request",
            false,
            json!({
                "status": "needs_clarification",
                "message": "the request is ambiguous; please rephrase with specifics",
                "operations": result.operations,
            }),
        );
    }
    if let Some(reason) = &result.unsupported_reason {
        return finish(
            as_json,
            "request",
            false,
            json!({
                "status": "unsupported", "reason": reason,
                "message": format!("unsupported request: {reason}"),
            }),
        );
    }

    // 3. Validate the proposed operations against canonical state (never trust the model directly).
    let ops: Vec<Value> = result
        .operations
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(stamp_schema_version)
        .collect();
    if ops.is_empty() {
        return finish(
            as_json,
            "request",
            false,
            json!({
                "status": "no_op", "message": "the request implies no changes",
            }),
        );
    }
    let report = mce_validator::validate_operations(sm.canonical(), &ops);
    if !report.ok {
        return finish(
            as_json,
            "request",
            false,
            json!({
                "status": "rejected",
                "errors": report.errors,
                "clarifications": report.clarifications,
                "unsupported": report.unsupported,
                "message": "operations failed validation",
            }),
        );
    }

    // 4. Open a candidate transaction (canonical state unchanged until commit).
    let tx = match sm.open_candidate(&report.normalized_operations) {
        Ok(tx) => tx,
        Err(e) => return fail(as_json, "request", format!("state apply failed: {e}")),
    };

    // 5. Generate the mod project (for the selected loader) from the *candidate* state.
    let mod_dir = project::mod_dir(project);
    let candidate = sm
        .candidate_state(&tx.transaction_id)
        .expect("candidate just opened")
        .clone();
    if let Err(e) = generator_for(loader).generate(&candidate, &mod_dir) {
        return fail(as_json, "request", format!("generation failed: {e}"));
    }

    // 6. Build the mod.
    let build = match GradleBuildManager.build(&mod_dir) {
        Ok(b) => b,
        Err(e) => return fail(as_json, "request", format!("build could not run: {e}")),
    };
    if !build.outcome.is_success() {
        return finish(
            as_json,
            "request",
            false,
            json!({
                "status": "build_failed",
                "outcome": format!("{:?}", build.outcome),
                "owner": build.outcome.owner(),
                "message": "generated mod failed to build",
                "log_tail": tail(&build.log, 40),
            }),
        );
    }

    // 7. Verify in-game (build → server boot → registry → GameTest) unless skipped.
    let (verified, verify_summary) = if do_verify {
        let runner = GameTestLevelRunner::run(&mod_dir, loader.gametest_task());
        let vreport = mce_verify::verify(&tx.verification_requirements, true, &runner);
        (vreport.passed, Some(vreport))
    } else {
        (true, None)
    };

    // 8. Commit only if verified; persist canonical state.
    if !verified {
        // Candidate discarded: canonical state is left byte-identical (SPEC-001 NFR-003).
        return finish(
            as_json,
            "request",
            false,
            json!({
                "status": "verification_failed",
                "transaction": tx.transaction_id,
                "failed_assertions": verify_summary.as_ref().map(|r| r.failed_assertions.clone()),
                "failure_owner": verify_summary.as_ref().and_then(|r| r.failure_owner.clone()),
                "message": "mod built but did not pass in-game verification; state unchanged",
            }),
        );
    }
    if let Err(e) = sm.commit(&tx.transaction_id, true) {
        return fail(as_json, "request", format!("commit failed: {e}"));
    }
    if let Err(e) = project::save(project, &sm) {
        return fail(as_json, "request", format!("failed to persist state: {e}"));
    }

    let action = action_verb(&tx.operations);
    finish(
        as_json,
        "request",
        true,
        json!({
            "status": if do_verify { "verified" } else { "built" },
            "transaction": tx.transaction_id,
            "operations": tx.operations,
            "action": action,
            // A convenience flag the desktop banner reads: anything that isn't a fresh create.
            "updated": action != "created",
            "jar": build.jar_path,
            "checksum": build.artifact_checksum,
            "gametests_passed": verify_summary.as_ref().map(|r| r.passed),
            "project": mod_dir.display().to_string(),
            "message": format!("{action} and verified — run `mce play` to launch the client"),
        }),
    )
}

/// `mce test` — regenerate the current canonical project, build, and run GameTests (no new intent).
fn run_test(as_json: bool, project: &Path, loader: Loader) -> ExitCode {
    let sm = match project::load(project) {
        Ok(sm) => sm,
        Err(e) => return fail(as_json, "test", e),
    };
    let mod_dir = project::mod_dir(project);
    if let Err(e) = generator_for(loader).generate(sm.canonical(), &mod_dir) {
        return fail(as_json, "test", format!("generation failed: {e}"));
    }
    let build = match GradleBuildManager.build(&mod_dir) {
        Ok(b) => b,
        Err(e) => return fail(as_json, "test", format!("build could not run: {e}")),
    };
    if !build.outcome.is_success() {
        return finish(
            as_json,
            "test",
            false,
            json!({
                "status": "build_failed", "outcome": format!("{:?}", build.outcome),
                "owner": build.outcome.owner(), "log_tail": tail(&build.log, 40),
            }),
        );
    }
    let requirements = build_requirements(sm.canonical());
    let runner = GameTestLevelRunner::run(&mod_dir, loader.gametest_task());
    let vreport = mce_verify::verify(&requirements, true, &runner);
    finish(
        as_json,
        "test",
        vreport.passed,
        json!({
            "status": if vreport.passed { "passed" } else { "failed" },
            "jar": build.jar_path,
            "levels": vreport.level_results,
            "failed_assertions": vreport.failed_assertions,
            "failure_owner": vreport.failure_owner,
        }),
    )
}

/// Re-derive verification requirements for the whole canonical state (for `mce test`, which has no
/// transaction of its own): re-open the create/recipe ops implied by current state.
fn build_requirements(state: &CanonicalState) -> Vec<mce_state::VerificationRequirement> {
    // A no-diff open against a fresh manager reproduces per-concept requirements from state; simplest
    // is to ask the state layer for each active concept's registry requirement via a rebuild.
    let mut sm = mce_state::ProjectStateManager::new(&state.name, &state.namespace);
    let ops: Vec<Value> = state
        .active_concepts()
        .map(|c| {
            json!({"operation":"create_concept","payload":{
                "concept_type": c.concept_type, "subtype": c.subtype,
                "display_name": c.display_name, "namespace_hint": c.namespace_id.split(':').next_back(),
                "properties": c.properties,
            }})
        })
        .collect();
    match sm.open_candidate(&ops) {
        Ok(tx) => tx.verification_requirements,
        Err(_) => Vec::new(),
    }
}

/// `mce play` — launch the client, gated on the project having been verified/built.
fn run_play(as_json: bool, project: &Path, interactive: bool, loader: Loader) -> ExitCode {
    if !project::is_initialized(project) {
        return fail(
            as_json,
            "play",
            format!("{} is not an MCE project", project.display()),
        );
    }
    let mod_dir = project::mod_dir(project);
    // Regenerate the source from the COMMITTED canonical state before launching. Otherwise a stale or
    // half-written build on disk (e.g. a failed create left a broken candidate) would break Quick Test
    // even though the saved project is fine. The gradle build inside the launcher recompiles it.
    if let Ok(sm) = project::load(project) {
        if let Err(e) = generator_for(loader).generate(sm.canonical(), &mod_dir) {
            return fail(
                as_json,
                "play",
                format!("could not prepare the mod to play: {e}"),
            );
        }
    }
    // The bounded headless smoke needs `xvfb-run` (Linux/CI only). On a desktop OS (macOS/Windows)
    // there is a real display, so run the visible interactive client — the default there.
    let use_interactive = interactive || !cfg!(target_os = "linux");
    let launcher = if use_interactive {
        GradleLauncher::interactive()
    } else {
        GradleLauncher::default()
    };
    if !as_json && use_interactive {
        // The #1 gotcha: creative tabs (and thus your items) only exist in Creative mode; a new
        // world defaults to Survival. Tell the player exactly how to see their creation.
        println!("launching client… once in-world, press T and run:  /gamemode creative");
        println!(
            "then press E → the Combat/Tools/Building-Blocks tab (or search) to find your items."
        );
    }
    // The project must have been built+verified by a prior `request`/`test`; the gradlew wrapper's
    // presence is our proxy for "was built", and the launcher refuses unverified work.
    match launcher.launch(&mod_dir.display().to_string(), true) {
        Ok(res) => finish(
            as_json,
            "play",
            res.started,
            json!({
                "status": if res.started { "launched" } else { "did_not_start" },
                "client_log": res.client_log_path,
                "message": if res.started { "client closed" }
                           else { "client did not reach ready state" },
            }),
        ),
        Err(e) => fail(as_json, "play", e),
    }
}

/// `mce revert <tx>` — restore a previously verified transaction's state (FR-006, `SPEC-004 §30`).
/// Regenerates + rebuilds + re-verifies the reverted state and commits it only if it still passes,
/// so a revert can never corrupt the current verified state (NFR-003).
fn run_revert(as_json: bool, transaction_id: &str, project: &Path, loader: Loader) -> ExitCode {
    let mut sm = match project::load(project) {
        Ok(sm) => sm,
        Err(e) => return fail(as_json, "revert", e),
    };
    let tx = match sm.revert(transaction_id) {
        Ok(tx) => tx,
        Err(e) => return fail(as_json, "revert", e),
    };
    let mod_dir = project::mod_dir(project);
    let candidate = sm
        .candidate_state(&tx.transaction_id)
        .expect("candidate just opened")
        .clone();
    if let Err(e) = generator_for(loader).generate(&candidate, &mod_dir) {
        return fail(as_json, "revert", format!("generation failed: {e}"));
    }
    let build = match GradleBuildManager.build(&mod_dir) {
        Ok(b) => b,
        Err(e) => return fail(as_json, "revert", format!("build could not run: {e}")),
    };
    let verified = if build.outcome.is_success() {
        let runner = GameTestLevelRunner::run(&mod_dir, loader.gametest_task());
        mce_verify::verify(&build_requirements(&candidate), true, &runner).passed
    } else {
        false
    };
    if !verified {
        return finish(
            as_json,
            "revert",
            false,
            json!({
                "status": "revert_failed",
                "message": "reverted state did not build/verify; current state left unchanged",
            }),
        );
    }
    if let Err(e) = sm.commit(&tx.transaction_id, true) {
        return fail(as_json, "revert", format!("commit failed: {e}"));
    }
    if let Err(e) = project::save(project, &sm) {
        return fail(as_json, "revert", format!("failed to persist state: {e}"));
    }
    finish(
        as_json,
        "revert",
        true,
        json!({
            "status": "reverted",
            "to": transaction_id,
            "transaction": tx.transaction_id,
            "jar": build.jar_path,
            "message": format!("reverted to {transaction_id} and re-verified"),
        }),
    )
}

/// `mce undo` — revert to the transaction just before the current head (one-step undo, FR-006).
fn run_undo(as_json: bool, project: &Path, loader: Loader) -> ExitCode {
    let sm = match project::load(project) {
        Ok(sm) => sm,
        Err(e) => return fail(as_json, "undo", e),
    };
    let hist = sm.committed_history();
    if hist.len() < 2 {
        return finish(
            as_json,
            "undo",
            false,
            json!({ "status": "nothing_to_undo", "message": "there's no earlier change to undo" }),
        );
    }
    let target = hist[hist.len() - 2].0.clone();
    drop(sm);
    run_revert(as_json, &target, project, loader)
}

/// `mce list` — the project's creations (active concepts) with type and key properties.
fn run_list(as_json: bool, project: &Path) -> ExitCode {
    let sm = match project::load(project) {
        Ok(sm) => sm,
        Err(e) => return fail(as_json, "list", e),
    };
    let items: Vec<Value> = sm
        .canonical()
        .active_concepts()
        .map(|c| {
            json!({
                "name": c.display_name,
                "type": format!("{:?}", c.concept_type).to_lowercase(),
                "subtype": c.subtype,
                "properties": c.properties,
            })
        })
        .collect();
    if as_json {
        println!("{}", serde_json::to_string(&items).unwrap());
    } else if items.is_empty() {
        println!("no creations yet — try `mce request \"...\"`");
    } else {
        for it in &items {
            println!(
                "  {} ({})",
                it["name"].as_str().unwrap_or("?"),
                it["type"].as_str().unwrap_or("?")
            );
        }
    }
    ExitCode::SUCCESS
}

/// The standard Minecraft `mods` folder for this OS.
fn default_mods_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    if cfg!(target_os = "macos") {
        PathBuf::from(home).join("Library/Application Support/minecraft/mods")
    } else if cfg!(target_os = "windows") {
        PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join(".minecraft/mods")
    } else {
        PathBuf::from(home).join(".minecraft/mods")
    }
}

/// `mce export` — copy the built mod JAR into a Minecraft mods folder so it can be played for real.
fn run_export(as_json: bool, project: &Path, to: Option<&Path>) -> ExitCode {
    let mod_dir = project::mod_dir(project);
    let (jar, checksum) = match mce_build::discover_jar(&mod_dir) {
        Some(j) => j,
        None => {
            return fail(
                as_json,
                "export",
                "no built mod found — run `mce request` or `mce test` first",
            )
        }
    };
    let dest_dir = to.map(PathBuf::from).unwrap_or_else(default_mods_dir);
    if let Err(e) = std::fs::create_dir_all(&dest_dir) {
        return fail(
            as_json,
            "export",
            format!("cannot create {dest_dir:?}: {e}"),
        );
    }
    let file_name = Path::new(&jar)
        .file_name()
        .map(|f| f.to_owned())
        .unwrap_or_default();
    let dest = dest_dir.join(&file_name);
    if let Err(e) = std::fs::copy(&jar, &dest) {
        return fail(as_json, "export", format!("failed to copy jar: {e}"));
    }
    finish(
        as_json,
        "export",
        true,
        json!({
            "status": "exported",
            "jar": dest.display().to_string(),
            "checksum": checksum,
            "message": format!("exported to {} — it will load next time you launch Minecraft", dest.display()),
        }),
    )
}

/// Print a final structured result and map success to the exit code.
fn finish(as_json: bool, command: &str, ok: bool, payload: Value) -> ExitCode {
    if as_json {
        println!("{}", serde_json::to_string_pretty(&payload).unwrap());
    } else {
        let status = payload.get("status").and_then(Value::as_str).unwrap_or("");
        let message = payload
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or(status);
        let mark = if ok { "✓" } else { "✗" };
        println!("{mark} mce {command}: {message}");
        for (k, label) in [
            ("transaction", "tx"),
            ("jar", "jar"),
            ("client_log", "client log"),
        ] {
            if let Some(v) = payload.get(k).and_then(Value::as_str) {
                println!("  {label}: {v}");
            }
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_ERROR)
    }
}

/// The last `n` lines of a log, for compact error reporting.
fn tail(log: &str, n: usize) -> String {
    let lines: Vec<&str> = log.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// Run a benchmark suite and print its summary.
fn run_bench(
    as_json: bool,
    corpus: PathBuf,
    report_dir: Option<PathBuf>,
    category: Option<String>,
    case: Option<String>,
    fail_fast: bool,
) -> ExitCode {
    let mut opts = BenchOptions::for_corpus(&corpus);
    if let Some(dir) = report_dir {
        opts.report_dir = dir;
    }
    opts.category = category;
    opts.case = case;
    opts.fail_fast = fail_fast;

    match run_suite(&corpus, &opts) {
        Ok(outcome) => {
            let s = &outcome.summary;
            if as_json {
                println!("{}", serde_json::to_string_pretty(s).unwrap());
            } else {
                println!(
                    "benchmark {suite}: {total} case(s) — {passed} passed, {failed} failed, {errored} errored, {skipped} skipped",
                    suite = s.suite,
                    total = s.total_cases,
                    passed = s.passed,
                    failed = s.failed,
                    errored = s.errored,
                    skipped = s.skipped,
                );
                println!("report: {}", s.report_dir);
            }
            // A run with failures/errors is itself a non-zero outcome.
            if s.failed + s.errored == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(EXIT_ERROR)
            }
        }
        Err(e) => {
            if as_json {
                let payload = json!({ "status": "error", "message": e.to_string() });
                println!("{}", serde_json::to_string_pretty(&payload).unwrap());
            } else {
                eprintln!("mce bench: {e}");
            }
            ExitCode::from(EXIT_ERROR)
        }
    }
}

/// Validate (and apply) operations against a state, emit a verdict.
///
/// Input JSON: `{ "namespace"?, "setup"?: [ops], "operations": [ops] }`. Always prints a JSON
/// verdict and exits 0 when a verdict was produced (accepted or not); non-zero only on bad input.
fn run_validate(input: &str) -> ExitCode {
    use std::io::Read;

    let text = if input == "-" {
        let mut s = String::new();
        if std::io::stdin().read_to_string(&mut s).is_err() {
            eprintln!("mce validate: failed to read stdin");
            return ExitCode::from(EXIT_ERROR);
        }
        s
    } else {
        match std::fs::read_to_string(input) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("mce validate: {e}");
                return ExitCode::from(EXIT_ERROR);
            }
        }
    };

    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("mce validate: invalid JSON: {e}");
            return ExitCode::from(EXIT_ERROR);
        }
    };

    let namespace = value
        .get("namespace")
        .and_then(|v| v.as_str())
        .unwrap_or("mce_ds");
    let mut sm = mce_state::ProjectStateManager::new("Dataset", namespace);

    // Apply optional setup operations to build the initial state.
    if let Some(setup) = value.get("setup").and_then(|v| v.as_array()) {
        if !setup.is_empty() {
            match sm.open_candidate(setup) {
                Ok(tx) => {
                    let _ = sm.commit(&tx.transaction_id, true);
                }
                Err(e) => {
                    let payload = json!({ "accepted": false, "reason": "setup_failed",
                        "message": e.to_string() });
                    println!("{}", serde_json::to_string(&payload).unwrap());
                    return ExitCode::SUCCESS;
                }
            }
        }
    }

    // The intent model proposes operations without the system-stamped envelope fields
    // (`schema_version`, canonical `operation_id`). Stamp `schema_version` here — the same
    // envelope completion the core does between intent output and validation (`SPEC-004 §34`).
    let ops: Vec<serde_json::Value> = value
        .get("operations")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(stamp_schema_version)
        .collect();

    let report = mce_validator::validate_operations(sm.canonical(), &ops);

    let mut verdict = json!({
        "accepted": false,
        "normalized_operations": report.normalized_operations,
        "errors": report.errors,
        "clarifications": report.clarifications,
        "unsupported": report.unsupported,
    });

    if report.ok {
        // Confirm the normalized operations actually apply to the state (stages 7–8).
        match sm.open_candidate(&report.normalized_operations) {
            Ok(tx) => {
                verdict["accepted"] = json!(true);
                verdict["candidate_state_hash"] = json!(tx.candidate_state_hash);
                verdict["verification_requirements"] = json!(tx.verification_requirements);
            }
            Err(e) => {
                verdict["reason"] = json!("state_apply_failed");
                verdict["message"] = json!(e.to_string());
            }
        }
    }

    println!("{}", serde_json::to_string(&verdict).unwrap());
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_capitalizes_names_after_called_or_named() {
        // A lowercase name is what made the small model misfire; capitalize it.
        assert_eq!(
            normalize_names("a ruby sword called sting with 8 attack damage"),
            "a ruby sword called Sting with 8 attack damage"
        );
        assert_eq!(
            normalize_names("make a dagger named night fang, 6 damage"),
            "make a dagger named Night Fang, 6 damage"
        );
        // No name to touch → unchanged.
        assert_eq!(
            normalize_names("a strong ruby pickaxe"),
            "a strong ruby pickaxe"
        );
        // Already capitalized → unchanged.
        assert_eq!(
            normalize_names("a sword called Excalibur"),
            "a sword called Excalibur"
        );
    }
}
