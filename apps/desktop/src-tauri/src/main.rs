// AI-mod creator — desktop app backend.
//
// A thin Tauri shell over the `amc` engine: each command shells out to the `amc` CLI with `--json`
// (so the app and the CLI can never drift), runs the heavy pipeline off the UI thread, and points
// the toolchain at the host JDK 21. The frontend (../ui) is the same web UI that runs standalone.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Value};
use tauri_plugin_dialog::DialogExt;

/// Suppress the console window Windows pops up when this GUI app spawns a console subprocess (the
/// `amc` engine, `llama-server`, `curl`, `java`). Without it every action flashes a terminal; with a
/// windowless engine, its own child tools (`gradlew.bat`, `java`) stay windowless too. No-op off Windows.
fn hide_console(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

/// The OS-default Minecraft `mods` folder (the vanilla launcher location).
fn minecraft_mods_default() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    if cfg!(target_os = "macos") {
        PathBuf::from(home).join("Library/Application Support/minecraft/mods")
    } else if cfg!(target_os = "windows") {
        PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join(".minecraft/mods")
    } else {
        PathBuf::from(home).join(".minecraft/mods")
    }
}

/// Where the app keeps projects: `$AMC_PROJECTS_DIR`, else `~/ai-mod-creator` (moved from the
/// pre-rename `~/ModCreationEngine` on first run). Having one stable
/// home means a project name always resolves the same way regardless of the app's working directory
/// (the bug this fixes: bare `--project name` was resolved against the app bundle's CWD).
fn projects_root() -> PathBuf {
    if let Ok(d) = std::env::var("AMC_PROJECTS_DIR") {
        return PathBuf::from(d);
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    let home = PathBuf::from(home);
    let root = home.join("ai-mod-creator");
    let legacy = home.join("ModCreationEngine");
    // One-time move of the pre-rename folder; if the move fails, keep using it where it is.
    if !root.exists() && legacy.is_dir() && std::fs::rename(&legacy, &root).is_err() {
        return legacy;
    }
    root
}

/// Resolve a project reference to an absolute path: a bare name → `<projects_root>/<name>`; an
/// explicit path is used as given. Always hand the engine an absolute path.
fn resolve_project(name: &str) -> String {
    let p = Path::new(name);
    if p.is_absolute() || name.contains(std::path::MAIN_SEPARATOR) {
        name.to_string()
    } else {
        projects_root().join(name).display().to_string()
    }
}

/// Directories that may hold app-bundled files, across platforms and dev/prod: next to the executable
/// (sidecars land here — `Contents/MacOS` in a `.app`, `<install>` on Windows, the target dir in dev),
/// the `resources/` subdir Tauri writes relative resource paths into (Windows/Linux), and the macOS
/// `.app` `Contents/Resources` (+ its `resources/` subdir).
fn bundled_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
            // Windows/Linux: `bundle.resources` land under `<exe dir>/resources/…`.
            dirs.push(dir.join("resources"));
            // macOS .app: Contents/MacOS/<exe> → Contents/Resources (+ its `resources/` subdir).
            if let Some(contents) = dir.parent() {
                let res = contents.join("Resources");
                dirs.push(res.join("resources"));
                dirs.push(res);
            }
        }
    }
    dirs
}

/// Path to the `amc` binary: `$AMC_BIN`, else bundled next to the app / in Resources, else `amc` on PATH.
fn amc_bin() -> String {
    if let Ok(p) = std::env::var("AMC_BIN") {
        return p;
    }
    let name = if cfg!(windows) { "amc.exe" } else { "amc" };
    for dir in bundled_dirs() {
        let side = dir.join(name);
        if side.exists() {
            return side.display().to_string();
        }
    }
    "amc".to_string()
}

/// A **JDK 21** for the Gradle build. NeoForge requires 21, so we do NOT blindly trust `$JAVA_HOME`
/// (it commonly points at an old JDK — that was the "build silently fails on Java 1.8" bug). Prefer a
/// real 21 from the known install locations per-OS, and only fall back to `$JAVA_HOME`.
fn java_home() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = Command::new("/usr/libexec/java_home")
            .args(["-v", "21"])
            .output()
        {
            if out.status.success() {
                let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !p.is_empty() {
                    return Some(p);
                }
            }
        }
        let temurin = "/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home";
        if Path::new(temurin).exists() {
            return Some(temurin.to_string());
        }
    }
    // Windows: scan the standard JDK install roots for a jdk-21* whose bin/java.exe exists.
    #[cfg(windows)]
    {
        let roots = [
            r"C:\Program Files\Eclipse Adoptium",
            r"C:\Program Files\Microsoft",
            r"C:\Program Files\Java",
            r"C:\Program Files\Zulu",
            r"C:\Program Files\Amazon Corretto",
        ];
        for root in roots {
            if let Ok(rd) = std::fs::read_dir(root) {
                for e in rd.flatten() {
                    let n = e.file_name().to_string_lossy().to_lowercase();
                    if n.contains("21") && e.path().join("bin").join("java.exe").exists() {
                        return Some(e.path().display().to_string());
                    }
                }
            }
        }
    }
    // Linux: common Temurin/system location.
    #[cfg(target_os = "linux")]
    {
        for p in [
            "/usr/lib/jvm/temurin-21-jdk-amd64",
            "/usr/lib/jvm/java-21-openjdk",
        ] {
            if Path::new(p).exists() {
                return Some(p.to_string());
            }
        }
    }
    // Last resort: whatever $JAVA_HOME is (only useful if it happens to be 21).
    std::env::var("JAVA_HOME").ok()
}

/// The port our own intent model server listens on. We deliberately do NOT reuse the engine's default
/// `8080` — that's a public well-known slot other tools (LM Studio, etc.) grab, and talking to the
/// wrong model there produces silently-wrong mods. The app owns this port and starts our model on it.
fn intent_port() -> u16 {
    std::env::var("AMC_INTENT_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8770)
}

/// The chat-completions URL for our model server — handed to `amc` via `$AMC_INTENT_ENDPOINT` so the
/// engine always talks to the model WE started, never whatever happens to be on `8080`.
fn intent_endpoint() -> String {
    format!("http://127.0.0.1:{}/v1/chat/completions", intent_port())
}

/// Is our intent model server accepting connections?
fn model_reachable() -> bool {
    format!("127.0.0.1:{}", intent_port())
        .parse()
        .ok()
        .and_then(|addr| {
            std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(400)).ok()
        })
        .is_some()
}

/// The model this app ships with (from the repo's `model.lock`) — the directory name under `models/`.
const PROMOTED_MODEL: &str = env!("AMC_MODEL_NAME");

/// The source checkout, when running from a dev build (`<repo>/target/{debug,release}/amc`).
fn repo_root() -> Option<PathBuf> {
    PathBuf::from(amc_bin())
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
}

/// The AMC Model directory: `amc-model.json` + the GGUF it names (+ an optional `adapter/` for the MLX
/// fallback). The model is distributed separately from the engine, so this is the one place the app
/// looks for it: `$AMC_MODEL_DIR`, then bundled (`models/<promoted>` next to the app / in Resources),
/// then a dev checkout's `models/<promoted>` (gitignored; fetched by `scripts/fetch_model.sh`).
fn model_dir() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("AMC_MODEL_DIR") {
        let p = PathBuf::from(d);
        if p.exists() {
            return Some(p);
        }
    }
    let mut cands: Vec<PathBuf> = bundled_dirs()
        .into_iter()
        .map(|d| d.join("models").join(PROMOTED_MODEL))
        .collect();
    if let Some(repo) = repo_root() {
        cands.push(repo.join("models").join(PROMOTED_MODEL));
    }
    cands.into_iter().find(|p| p.exists())
}

/// The fine-tuned LoRA adapter for the MLX fallback runtime: `$AMC_ADAPTER_DIR`, else `<model dir>/adapter`.
fn adapter_dir() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("AMC_ADAPTER_DIR") {
        let p = PathBuf::from(d);
        if p.exists() {
            return Some(p);
        }
    }
    model_dir()
        .map(|d| d.join("adapter"))
        .filter(|p| p.exists())
}

/// How to launch `mlx_lm.server`: `$AMC_MODEL_CMD` (an explicit program), else `mlx_lm.server` on PATH,
/// else the common radioconda install, else `python3 -m mlx_lm.server`. Returns (program, leading args).
fn model_server_cmd() -> (String, Vec<String>) {
    if let Ok(c) = std::env::var("AMC_MODEL_CMD") {
        return (c, vec![]);
    }
    // A radioconda install puts mlx_lm.server here.
    if let Ok(home) = std::env::var("HOME") {
        let cand = PathBuf::from(&home).join("radioconda/bin/mlx_lm.server");
        if cand.exists() {
            return (cand.display().to_string(), vec![]);
        }
    }
    // `mlx_lm.server` on PATH if present, else fall back to `python3 -m mlx_lm.server`.
    if Command::new("mlx_lm.server").arg("--help").output().is_ok() {
        ("mlx_lm.server".into(), vec![])
    } else {
        ("python3".into(), vec!["-m".into(), "mlx_lm.server".into()])
    }
}

/// The quantized AMC Model GGUF — the **no-Python** runtime (served by llama.cpp). `$AMC_MODEL_GGUF`,
/// else the file the model directory's manifest names.
fn gguf_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("AMC_MODEL_GGUF") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    let dir = model_dir()?;
    let name = std::fs::read_to_string(dir.join("amc-model.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|m| m["gguf"].as_str().map(str::to_string))?;
    Some(dir.join(name)).filter(|p| p.exists())
}

/// Check the model directory's manifest against the engine (`amc model verify`): a model trained on a
/// different contract or grammar than this engine embeds produces silently-wrong operations, so refuse
/// it up front. Weights hashes are checked at staging time, so startup skips re-hashing them.
fn model_compatible(dir: &Path) -> Result<(), String> {
    let out = hide_console(Command::new(amc_bin()).args([
        "--json",
        "model",
        "verify",
        &dir.display().to_string(),
        "--skip-weights",
    ]))
    .output()
    .map_err(|e| format!("couldn't run the engine to check the model: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let v: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    let detail = v["problems"]
        .as_array()
        .map(|ps| {
            ps.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("; ")
        })
        .or_else(|| v["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| String::from_utf8_lossy(&out.stderr).trim().to_string());
    Err(format!(
        "the installed AI model doesn't match this engine version ({detail})"
    ))
}

/// The GBNF grammar that constrains the model to well-formed, bounded JSON (no truncated output, no
/// runaway decimals) — the robustness backstop so ANY request parses. Engine-owned (the model manifest
/// pins its hash). `$AMC_GRAMMAR`, then bundled, then the dev checkout's `crates/amc-intent/contract/`.
fn grammar_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("AMC_GRAMMAR") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    let mut cands: Vec<PathBuf> = bundled_dirs()
        .into_iter()
        .map(|d| d.join("amc-ops.gbnf"))
        .collect();
    if let Some(repo) = repo_root() {
        cands.push(repo.join("crates/amc-intent/contract/amc-ops.gbnf"));
    }
    cands.into_iter().find(|p| p.exists())
}

/// A native `llama-server` (llama.cpp) binary — the no-Python model runtime. `$AMC_LLAMA_SERVER`,
/// then bundled beside the app, then Homebrew, then PATH.
fn llama_server_bin() -> Option<String> {
    if let Ok(p) = std::env::var("AMC_LLAMA_SERVER") {
        if PathBuf::from(&p).exists() {
            return Some(p);
        }
    }
    for dir in bundled_dirs() {
        let cand = dir.join(if cfg!(windows) {
            "llama-server.exe"
        } else {
            "llama-server"
        });
        if cand.exists() {
            return Some(cand.display().to_string());
        }
    }
    for p in [
        "/opt/homebrew/bin/llama-server",
        "/usr/local/bin/llama-server",
    ] {
        if PathBuf::from(p).exists() {
            return Some(p.to_string());
        }
    }
    if hide_console(Command::new("llama-server").arg("--version"))
        .output()
        .is_ok()
    {
        return Some("llama-server".into());
    }
    None
}

/// The model server this app started, if any — stopped on exit so closing the app doesn't leave
/// `llama-server` running and holding the port (where the next launch, possibly an upgraded app with a
/// different model, would mistake it for its own). A server we found already running isn't ours to stop.
static MODEL_SERVER: Mutex<Option<Child>> = Mutex::new(None);

fn stop_model() {
    if let Some(mut child) = MODEL_SERVER
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take()
    {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Start our intent model server if it isn't already up. Idempotent: a reachable server short-circuits.
/// Prefers the **no-Python** path — native `llama-server` on the quantized GGUF — and falls back to the
/// Python MLX runtime (base + adapter) only when the GGUF or llama-server isn't available. Returns a
/// status the UI can show: already running, starting, or unavailable (with the reason).
fn ensure_model() -> Value {
    if model_reachable() {
        return json!({ "state": "running" });
    }
    let log = std::env::temp_dir().join("amc-model-server.log");
    let out = std::fs::File::create(&log).ok();

    // A model built for a different contract/grammar must never be served (silently-wrong mods).
    if let Some(dir) = model_dir() {
        if let Err(detail) = model_compatible(&dir) {
            return json!({ "state": "unavailable", "detail": detail });
        }
    }

    // Build the launch command: llama.cpp (no Python) first, else MLX.
    let (prog, args, backend) = match (llama_server_bin(), gguf_path()) {
        (Some(llama), Some(gguf)) => {
            // A GBNF grammar (added below when present) is the real robustness backstop: it forces
            // well-formed, bounded JSON so a model slip can never emit unparseable output for ANY
            // request. The DRY sampler is belt-and-suspenders against degenerate decimal loops.
            let mut args = vec![
                "-m".into(),
                gguf.display().to_string(),
                "--host".into(),
                "127.0.0.1".into(),
                "--port".into(),
                intent_port().to_string(),
                "--temp".into(),
                "0".into(),
                "--dry-multiplier".into(),
                "0.8".into(),
                "--dry-base".into(),
                "1.75".into(),
                "--dry-allowed-length".into(),
                "3".into(),
            ];
            if let Some(grammar) = grammar_path() {
                args.push("--grammar-file".into());
                args.push(grammar.display().to_string());
            }
            (llama, args, "llama.cpp/GGUF (no Python)")
        }
        _ => {
            let Some(adapter) = adapter_dir() else {
                return json!({ "state": "unavailable",
                    "detail": "no AI model found — fetch it with scripts/fetch_model.sh or set AMC_MODEL_DIR" });
            };
            let (prog, mut args) = model_server_cmd();
            args.extend([
                "--model".into(),
                "google/functiongemma-270m-it".into(),
                "--adapter-path".into(),
                adapter.display().to_string(),
                "--port".into(),
                intent_port().to_string(),
            ]);
            (prog, args, "MLX adapter (Python)")
        }
    };

    eprintln!("[amc-desktop] starting model backend: {backend} via {prog}");
    let mut cmd = Command::new(&prog);
    cmd.args(&args);
    hide_console(&mut cmd);
    if let (Some(o), Some(e)) = (out.as_ref().and_then(|f| f.try_clone().ok()), out) {
        cmd.stdout(o).stderr(e);
    }
    match cmd.spawn() {
        Ok(child) => {
            *MODEL_SERVER.lock().unwrap_or_else(|p| p.into_inner()) = Some(child);
            json!({ "state": "starting", "backend": backend, "log": log.display().to_string() })
        }
        Err(e) => json!({ "state": "unavailable",
            "detail": format!("couldn't launch the model server ({prog}): {e}") }),
    }
}

/// The mod loader the desktop app builds for. **Fabric-first** during the multi-loader rollout —
/// the app is Fabric-only unless `$AMC_LOADER=neoforge` is set explicitly. Passed as `--loader` on
/// every engine call so behaviour never depends on how the app was launched.
/// TODO: replace this default with the user's on-install choice (persisted) once the picker lands
/// (the eventual first-run loader selection). For now the app is Fabric-first.
fn active_loader() -> String {
    match std::env::var("AMC_LOADER")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "neoforge" | "neo" => "neoforge".to_string(),
        // default (empty / "fabric" / anything else) → Fabric
        _ => "fabric".to_string(),
    }
}

/// Run `amc --json --loader <active> <args…>` and parse its JSON result.
fn run_amc(args: &[String]) -> Result<Value, String> {
    let mut cmd = Command::new(amc_bin());
    cmd.arg("--json")
        .args(["--loader".to_string(), active_loader()])
        .args(args);
    cmd.env("AMC_INTENT_ENDPOINT", intent_endpoint());
    if let Some(jh) = java_home() {
        cmd.env("JAVA_HOME", jh);
    }
    hide_console(&mut cmd);
    let out = cmd
        .output()
        .map_err(|e| format!("could not start the engine ({}): {e}", amc_bin()))?;
    let s = String::from_utf8_lossy(&out.stdout);
    let s = s.trim();
    if s.is_empty() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("the engine produced no output: {}", err.trim()));
    }
    serde_json::from_str(s).map_err(|e| format!("engine output was not JSON: {e}\n{s}"))
}

/// Off-thread wrapper so the ~minute-long build/verify never freezes the window.
async fn run_amc_async(args: Vec<String>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || run_amc(&args))
        .await
        .map_err(|e| e.to_string())?
}

/// The names of projects in the projects home (dirs containing an `.amc/state.json`, or the
/// pre-rename `.mce/state.json`, which the engine moves to `.amc/` the first time it opens one).
#[tauri::command]
fn list_projects() -> Vec<String> {
    eprintln!(
        "[amc-desktop] IPC ok: list_projects (root={})",
        projects_root().display()
    );
    let root = projects_root();
    let _ = std::fs::create_dir_all(&root);
    let mut v = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&root) {
        for e in rd.flatten() {
            let p = e.path();
            if p.join(".amc").join("state.json").exists()
                || p.join(".mce").join("state.json").exists()
            {
                if let Some(n) = e.file_name().to_str() {
                    v.push(n.to_string());
                }
            }
        }
    }
    v.sort();
    v
}

/// Create a new project `<projects_root>/<name>`.
#[tauri::command]
fn init_project(name: String) -> Result<Value, String> {
    let root = projects_root();
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let mut cmd = Command::new(amc_bin());
    cmd.arg("--json")
        .args(["--loader", &active_loader()])
        .args(["init", &name])
        .current_dir(&root);
    hide_console(&mut cmd);
    let out = cmd.output().map_err(|e| e.to_string())?;
    let s = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(s.trim()).map_err(|e| format!("could not create project: {e}\n{s}"))
}

#[tauri::command]
async fn list_items(project: String) -> Result<Value, String> {
    run_amc_async(vec![
        "list".into(),
        "--project".into(),
        resolve_project(&project),
    ])
    .await
}

/// Create or refine — the engine (model + project context) decides between a new concept and an
/// update. `refine_target` is passed through for future explicit scoping.
#[tauri::command]
async fn request(
    project: String,
    text: String,
    refine_target: Option<String>,
) -> Result<Value, String> {
    let mut args = vec![
        "request".into(),
        text,
        "--project".into(),
        resolve_project(&project),
    ];
    // A non-empty refine target scopes this as an UPDATE to that creation (`amc request --refine <name>`),
    // so an edit like "do 12 damage" targets it instead of colliding on the name as a duplicate create.
    if let Some(t) = refine_target.filter(|t| !t.is_empty()) {
        args.push("--refine".into());
        args.push(t);
    }
    run_amc_async(args).await
}

#[tauri::command]
async fn undo(project: String) -> Result<Value, String> {
    run_amc_async(vec![
        "undo".into(),
        "--project".into(),
        resolve_project(&project),
    ])
    .await
}

/// The NeoForge version our generated mods target (must match `amc-generator`'s `NEOFORGE_VERSION`).
/// NeoForge `21.1.x` is the loader for Minecraft **1.21.1**.
const NEOFORGE_VERSION: &str = "21.1.235";

/// Whether a NeoForge-for-1.21.1 version profile is installed under a Minecraft dir — our mods only
/// load there. The official installer names the profile `neoforge-21.1.NNN` (NOT literally "1.21.1"),
/// so we match `neoforge` + the `21.1` loader series. Users on old Forge/other versions get a clear
/// heads-up (and an offer to install it) instead of a silent no-op.
fn has_compatible_neoforge(mc_dir: &Path) -> bool {
    std::fs::read_dir(mc_dir.join("versions"))
        .map(|rd| {
            rd.flatten().any(|e| {
                let n = e.file_name().to_string_lossy().to_lowercase();
                n.contains("neoforge") && n.contains("21.1")
            })
        })
        .unwrap_or(false)
}

/// Download and run the official NeoForge client installer so the user's own Minecraft gains a
/// matching 1.21.1 + NeoForge profile — turning "Add to my Minecraft" from a dead end into one click.
/// Network + install is slow (~1-2 min), so it runs off the UI thread.
#[tauri::command]
async fn install_neoforge() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(install_neoforge_blocking)
        .await
        .map_err(|e| e.to_string())?
}

fn install_neoforge_blocking() -> Result<Value, String> {
    let mc_dir = minecraft_mods_default()
        .parent()
        .map(Path::to_path_buf)
        .ok_or("couldn't locate your Minecraft folder")?;
    if has_compatible_neoforge(&mc_dir) {
        return Ok(json!({ "state": "already",
            "detail": "Minecraft 1.21.1 + NeoForge is already set up — you're good to go." }));
    }
    std::fs::create_dir_all(&mc_dir).map_err(|e| e.to_string())?;
    // The installer edits launcher_profiles.json; a real install has one, but create a stub if missing
    // (e.g. a launcher that hasn't run yet) so the headless install doesn't bail.
    let profiles = mc_dir.join("launcher_profiles.json");
    if !profiles.exists() {
        let _ = std::fs::write(&profiles, r#"{"profiles":{},"settings":{},"version":3}"#);
    }
    let url = format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{V}/neoforge-{V}-installer.jar",
        V = NEOFORGE_VERSION
    );
    let dl_dir = private_download_dir()?;
    let jar = dl_dir.join(format!("neoforge-{NEOFORGE_VERSION}-installer.jar"));
    // curl ships on macOS, Windows 10+, and Linux — avoids pulling an HTTP stack into the app.
    let dl = hide_console(
        Command::new("curl")
            .args(["-L", "--fail", "-s", "-o"])
            .arg(&jar)
            .arg(&url),
    )
    .status()
    .map_err(|e| format!("couldn't start the download: {e}"))?;
    if !dl.success() {
        return Err(
            "couldn't download the NeoForge installer — check your internet connection.".into(),
        );
    }
    let java_name = if cfg!(windows) { "java.exe" } else { "java" };
    let java = java_home()
        .map(|h| PathBuf::from(h).join("bin").join(java_name))
        .filter(|p| p.exists())
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "java".into());
    let out = hide_console(
        Command::new(&java)
            .arg("-jar")
            .arg(&jar)
            .arg("--install-client")
            .arg(&mc_dir),
    )
    .output()
    .map_err(|e| format!("couldn't run the installer (Java 21 required): {e}"))?;
    let _ = std::fs::remove_dir_all(&dl_dir);
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let last = err.trim().lines().last().unwrap_or("unknown error");
        return Err(format!("the NeoForge installer failed: {last}"));
    }
    if has_compatible_neoforge(&mc_dir) {
        Ok(json!({ "state": "installed", "detail": format!(
            "Installed Minecraft 1.21.1 + NeoForge {NEOFORGE_VERSION}. Open the Minecraft Launcher, pick the \
             \"NeoForge 1.21.1\" profile, and start a NEW world — your creations will be in it."
        ) }))
    } else {
        Ok(json!({ "state": "installed", "detail":
            "Installer finished. Open the Minecraft Launcher and choose the NeoForge 1.21.1 profile." }))
    }
}

// ---- Fabric "Add to my Minecraft" -----------------------------------------------------------
// Fabric versions our generated mods target — must match `amc-generator`'s fabric constants.
const MINECRAFT_VERSION: &str = "1.21.1";
const FABRIC_LOADER_VERSION: &str = "0.16.5";
const FABRIC_API_VERSION: &str = "0.102.0+1.21.1";
// The official Fabric installer (stable) used to create the loader profile in the user's Minecraft.
const FABRIC_INSTALLER_VERSION: &str = "1.1.1";

/// Whether a Fabric-Loader-for-1.21.1 version profile is installed under a Minecraft dir. The Fabric
/// installer names the profile `fabric-loader-<loader>-1.21.1`.
fn has_compatible_fabric(mc_dir: &Path) -> bool {
    std::fs::read_dir(mc_dir.join("versions"))
        .map(|rd| {
            rd.flatten().any(|e| {
                let n = e.file_name().to_string_lossy().to_lowercase();
                n.contains("fabric") && n.contains("1.21.1")
            })
        })
        .unwrap_or(false)
}

/// Whether the Fabric API mod jar is present in the mods folder. Unlike NeoForge, a generated Fabric
/// mod also **requires the Fabric API mod** to be installed alongside it — so "compatible" means both
/// the loader profile *and* this jar are present.
fn has_fabric_api(mods_dir: &Path) -> bool {
    std::fs::read_dir(mods_dir)
        .map(|rd| {
            rd.flatten().any(|e| {
                let n = e.file_name().to_string_lossy().to_lowercase();
                n.starts_with("fabric-api-") && n.ends_with(".jar")
            })
        })
        .unwrap_or(false)
}

/// Resolve the `java` binary (JDK 21) the installers should run with.
fn java_bin() -> String {
    let java_name = if cfg!(windows) { "java.exe" } else { "java" };
    java_home()
        .map(|h| PathBuf::from(h).join("bin").join(java_name))
        .filter(|p| p.exists())
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "java".into())
}

/// A fresh directory only this user can write, for a downloaded installer we are about to run. A
/// fixed path in the shared temp dir could be pre-created or swapped by another local user between
/// the download and `java -jar`; `create_dir` fails if the name already exists, and it is 0700 on Unix.
fn private_download_dir() -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("amc-dl-{}-{nanos}", std::process::id()));
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(&dir)
        .map_err(|e| format!("couldn't create a download folder: {e}"))?;
    Ok(dir)
}

/// Download `url` to `dest` with curl (ships on macOS/Win10+/Linux). Returns a friendly error on failure.
fn curl_download(url: &str, dest: &Path, what: &str) -> Result<(), String> {
    let ok = hide_console(
        Command::new("curl")
            .args(["-L", "--fail", "-s", "-o"])
            .arg(dest)
            .arg(url),
    )
    .status()
    .map_err(|e| format!("couldn't start the {what} download: {e}"))?
    .success();
    if ok {
        Ok(())
    } else {
        Err(format!(
            "couldn't download {what} — check your internet connection."
        ))
    }
}

/// Set up the user's own Minecraft for our generated Fabric mods: install the **Fabric Loader**
/// 1.21.1 profile (via the official installer) and drop the **Fabric API** mod into their mods
/// folder. Turns "Add to my Minecraft" from a dead end into one click. Slow (network + install), so
/// it runs off the UI thread.
#[tauri::command]
async fn install_fabric() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(install_fabric_blocking)
        .await
        .map_err(|e| e.to_string())?
}

fn install_fabric_blocking() -> Result<Value, String> {
    let mods_dir = minecraft_mods_default();
    let mc_dir = mods_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or("couldn't locate your Minecraft folder")?;
    if has_compatible_fabric(&mc_dir) && has_fabric_api(&mods_dir) {
        return Ok(json!({ "state": "already",
            "detail": "Minecraft 1.21.1 + Fabric (Loader + API) is already set up — you're good to go." }));
    }
    std::fs::create_dir_all(&mods_dir).map_err(|e| e.to_string())?;
    // The installer edits launcher_profiles.json; stub one if a launcher hasn't created it yet.
    let profiles = mc_dir.join("launcher_profiles.json");
    if !profiles.exists() {
        let _ = std::fs::write(&profiles, r#"{"profiles":{},"settings":{},"version":3}"#);
    }
    let java = java_bin();

    // 1) Fabric Loader profile (only if missing) via the official headless installer.
    if !has_compatible_fabric(&mc_dir) {
        let url = format!(
            "https://maven.fabricmc.net/net/fabricmc/fabric-installer/{V}/fabric-installer-{V}.jar",
            V = FABRIC_INSTALLER_VERSION
        );
        let dl_dir = private_download_dir()?;
        let jar = dl_dir.join(format!("fabric-installer-{FABRIC_INSTALLER_VERSION}.jar"));
        curl_download(&url, &jar, "the Fabric installer")?;
        let out = hide_console(
            Command::new(&java)
                .arg("-jar")
                .arg(&jar)
                .args([
                    "client",
                    "-mcversion",
                    MINECRAFT_VERSION,
                    "-loader",
                    FABRIC_LOADER_VERSION,
                ])
                .arg("-dir")
                .arg(&mc_dir),
        )
        .output()
        .map_err(|e| format!("couldn't run the Fabric installer (Java 21 required): {e}"))?;
        let _ = std::fs::remove_dir_all(&dl_dir);
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let last = err.trim().lines().last().unwrap_or("unknown error");
            return Err(format!("the Fabric installer failed: {last}"));
        }
    }

    // 2) Fabric API mod jar into the mods folder (required by our generated mods).
    if !has_fabric_api(&mods_dir) {
        let url = format!(
            "https://maven.fabricmc.net/net/fabricmc/fabric-api/fabric-api/{V}/fabric-api-{V}.jar",
            V = FABRIC_API_VERSION
        );
        let dest = mods_dir.join(format!("fabric-api-{FABRIC_API_VERSION}.jar"));
        curl_download(&url, &dest, "the Fabric API mod")?;
    }

    if has_compatible_fabric(&mc_dir) && has_fabric_api(&mods_dir) {
        Ok(json!({ "state": "installed", "detail": format!(
            "Installed Fabric Loader {FABRIC_LOADER_VERSION} (Minecraft 1.21.1) + Fabric API. Open the \
             Minecraft Launcher, pick the \"fabric-loader-1.21.1\" profile, and start a NEW world — your \
             creations will be in it."
        ) }))
    } else {
        Ok(json!({ "state": "installed", "detail":
            "Fabric setup finished. Open the Minecraft Launcher and choose the fabric-loader 1.21.1 profile." }))
    }
}

/// Set up whichever loader the app is currently building for (the UI calls this loader-agnostically).
#[tauri::command]
async fn install_loader() -> Result<Value, String> {
    match active_loader().as_str() {
        "neoforge" => tauri::async_runtime::spawn_blocking(install_neoforge_blocking)
            .await
            .map_err(|e| e.to_string())?,
        _ => tauri::async_runtime::spawn_blocking(install_fabric_blocking)
            .await
            .map_err(|e| e.to_string())?,
    }
}

/// Open the user's **Minecraft Launcher** so they can start the freshly-installed loader profile.
/// We can't start the game directly (the launcher owns Mojang/Microsoft login), but this gets them
/// one click from playing instead of hunting for the launcher themselves.
#[tauri::command]
fn launch_minecraft() -> Result<Value, String> {
    let cant = "couldn't find the Minecraft Launcher — open it yourself, then pick the \
                fabric-loader 1.21.1 profile and hit Play.";
    #[cfg(target_os = "macos")]
    {
        // The launcher app is "Minecraft.app"; try by name, then by bundle id.
        for args in [["-a", "Minecraft"], ["-b", "com.mojang.minecraftlauncher"]] {
            if hide_console(Command::new("open").args(args))
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
            {
                return Ok(json!({ "state": "launched" }));
            }
        }
        Err(cant.into())
    }
    #[cfg(windows)]
    {
        let exes = [
            r"C:\Program Files (x86)\Minecraft Launcher\MinecraftLauncher.exe",
            r"C:\XboxGames\Minecraft Launcher\Content\Minecraft.exe",
        ];
        for p in exes {
            if Path::new(p).exists() && hide_console(&mut Command::new(p)).spawn().is_ok() {
                return Ok(json!({ "state": "launched" }));
            }
        }
        // Fallback: the launcher registers a `minecraft:` URI.
        if hide_console(Command::new("cmd").args(["/C", "start", "", "minecraft:"]))
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Ok(json!({ "state": "launched" }));
        }
        Err(cant.into())
    }
    #[cfg(target_os = "linux")]
    {
        for prog in ["minecraft-launcher", "minecraft"] {
            if hide_console(&mut Command::new(prog)).spawn().is_ok() {
                return Ok(json!({ "state": "launched" }));
            }
        }
        Err(cant.into())
    }
}

/// The mod loader this app session targets, mirroring the CLI's `$AMC_LOADER` resolution (defaults
/// to NeoForge). Surfaced to the UI so a user can see at a glance whether they're building a
/// **Fabric** or **NeoForge** mod — otherwise leftover jars from the other loader are indistinguishable.
#[tauri::command]
fn loader_info() -> Value {
    let (loader, label) = match active_loader().as_str() {
        "neoforge" => ("neoforge", "NeoForge"),
        _ => ("fabric", "Fabric"),
    };
    json!({ "loader": loader, "label": label, "minecraft": "1.21.1" })
}

/// Where the user's Minecraft mods folder is (default), whether it exists, and whether their game is
/// set up for the **active loader** — `compatible` is loader-aware (NeoForge profile, or Fabric
/// Loader profile *and* the Fabric API mod). `neoforge1211` is kept for backward compatibility.
#[tauri::command]
fn minecraft_info() -> Value {
    let d = minecraft_mods_default();
    let mc = d.parent();
    let neoforge = mc.map(has_compatible_neoforge).unwrap_or(false);
    let loader = active_loader();
    let compatible = match loader.as_str() {
        "neoforge" => neoforge,
        _ => mc.map(has_compatible_fabric).unwrap_or(false) && has_fabric_api(&d),
    };
    json!({
        "modsDir": d.display().to_string(),
        "exists": d.exists(),
        "loader": loader,
        "compatible": compatible,
        "neoforge1211": neoforge,
    })
}

/// Open a native folder picker so the user can point us at their Minecraft `mods` folder
/// (launchers/instances put it in many places). Returns the chosen path, or null if cancelled.
#[tauri::command]
async fn pick_mods_folder(app: tauri::AppHandle) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let start = minecraft_mods_default();
        let mut b = app
            .dialog()
            .file()
            .set_title("Choose your Minecraft mods folder");
        if start.exists() {
            b = b.set_directory(start);
        }
        b.blocking_pick_folder().map(|p| p.to_string())
    })
    .await
    .ok()
    .flatten()
}

/// Copy the built mod into a mods folder. `to` = the chosen folder (from the picker); when absent,
/// the engine uses the OS-default Minecraft mods folder.
#[tauri::command]
async fn export(project: String, to: Option<String>) -> Result<Value, String> {
    let mut args = vec![
        "export".into(),
        "--project".into(),
        resolve_project(&project),
    ];
    if let Some(t) = to {
        args.push("--to".into());
        args.push(t);
    }
    run_amc_async(args).await
}

/// Launch the visible Minecraft sandbox. We do NOT fire-and-forget: the client build/launch can fail
/// (bad jar, Gradle error, JDK mismatch), and swallowing that is exactly the "it silently did nothing"
/// bug. So we watch the process until it either reaches the ready marker (→ launched, leave it running
/// for the session) or exits early with an error (→ surface the log tail). Off-thread so the UI stays live.
#[tauri::command]
async fn play(project: String) -> Result<Value, String> {
    let proj = resolve_project(&project);
    let endpoint = intent_endpoint();
    let jh = java_home();
    let bin = amc_bin();
    eprintln!("[amc-desktop] play: {proj}");
    tauri::async_runtime::spawn_blocking(move || play_watch(bin, proj, endpoint, jh))
        .await
        .map_err(|e| e.to_string())?
}

/// Is this a Windows Remote Desktop session? (`SESSIONNAME` is `RDP-Tcp#N` there, `Console` locally.)
fn remote_desktop_session() -> bool {
    cfg!(windows)
        && std::env::var("SESSIONNAME").is_ok_and(|s| s.to_ascii_uppercase().starts_with("RDP-"))
}

fn play_watch(
    bin: String,
    proj: String,
    endpoint: String,
    jh: Option<String>,
) -> Result<Value, String> {
    use std::time::Instant;
    let log = std::env::temp_dir().join("amc-play.log");
    let file =
        std::fs::File::create(&log).map_err(|e| format!("couldn't open the play log: {e}"))?;
    let err_file = file.try_clone().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(&bin);
    cmd.args([
        "--loader",
        &active_loader(),
        "play",
        "--project",
        &proj,
        "--interactive",
    ]);
    cmd.env("AMC_INTENT_ENDPOINT", endpoint);
    if let Some(jh) = jh {
        cmd.env("JAVA_HOME", jh);
    }
    cmd.stdout(file).stderr(err_file);
    hide_console(&mut cmd);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("could not start the sandbox: {e}"))?;

    // Ready markers = the client actually came up; failure = an early non-zero exit. Only lines Minecraft
    // logs AFTER its OpenGL window exists count: `Setting user:` and the `LWJGL` backend line print
    // before window creation, so a client that dies on a graphics-driver error (e.g. "LoadLibrary failed
    // with error 126" over Remote Desktop) used to be reported as open.
    const READY: &[&str] = &["Sound engine started", "textures/atlas/", "joined the game"];
    // The client logs this right before creating its window; if nothing follows, the window never came up.
    const PRE_WINDOW: &str = "Backend library: LWJGL";
    let started = Instant::now();
    let deadline = started + Duration::from_secs(300);
    let mut pre_window_at: Option<Instant> = None;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if status.success() {
                return Ok(json!({ "status": "launched", "detail": "the sandbox client closed" }));
            }
            let tail = log_tail(&log);
            return Err(format!(
                "the sandbox couldn't start.\n\n{tail}\n\n(Full log: {})",
                log.display()
            ));
        }
        if let Ok(text) = std::fs::read_to_string(&log) {
            if READY.iter().any(|m| text.contains(m)) {
                return Ok(json!({ "status": "launched" })); // up and running — leave it for the session
            }
            if pre_window_at.is_none() && text.contains(PRE_WINDOW) {
                pre_window_at = Some(Instant::now());
            }
        }
        // Over Remote Desktop there's no OpenGL driver, so the window never appears (Windows shows a
        // driver error dialog instead). Say so plainly rather than waiting out the full deadline.
        if remote_desktop_session()
            && pre_window_at.is_some_and(|t| t.elapsed() > Duration::from_secs(25))
        {
            return Err(format!(
                "Minecraft couldn't open its game window. This PC is being used over Remote Desktop, \
                 which can't run OpenGL games — run the Quick Test at the computer itself.\n\n(Full log: {})",
                log.display()
            ));
        }
        if Instant::now() > deadline {
            return Ok(json!({ "status": "starting",
                "detail": format!("Minecraft still hasn't opened its window after {} minutes. If no game \
                    window appears, check the log: {}", started.elapsed().as_secs() / 60, log.display()) }));
        }
        std::thread::sleep(Duration::from_millis(800));
    }
}

/// The last few meaningful lines of a build/launch log — the actual error, not the whole Gradle wall.
fn log_tail(path: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return "(no log output)".to_string();
    };
    let interesting: Vec<&str> = text
        .lines()
        .filter(|l| {
            let l = l.to_lowercase();
            l.contains("error")
                || l.contains("exception")
                || l.contains("build failed")
                || l.contains("caused by")
                || l.contains("what went wrong")
                || l.contains("java")
        })
        .collect();
    let pick: Vec<&str> = if interesting.is_empty() {
        text.lines()
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    } else {
        interesting
            .into_iter()
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    };
    pick.join("\n")
}

/// Is our intent model reachable? Drives the "engine ready" indicator.
#[tauri::command]
fn model_status() -> bool {
    model_reachable()
}

/// Start the local intent model if needed (called on app startup, and defensively before a request).
/// The UI polls `model_status` afterward to flip to "engine ready" once it finishes loading.
#[tauri::command]
fn start_model() -> Value {
    let r = ensure_model();
    eprintln!("[amc-desktop] ensure_model: {r}");
    r
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_projects,
            init_project,
            list_items,
            request,
            undo,
            export,
            play,
            model_status,
            start_model,
            minecraft_info,
            loader_info,
            install_neoforge,
            install_fabric,
            install_loader,
            launch_minecraft,
            pick_mods_folder
        ])
        .build(tauri::generate_context!())
        .expect("error while running AI-mod creator")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                stop_model();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_download_dir_is_fresh_and_owner_only() {
        let a = private_download_dir().unwrap();
        let b = private_download_dir().unwrap();
        assert_ne!(a, b);
        assert!(a.is_dir() && b.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&a).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700);
        }
        let _ = std::fs::remove_dir_all(a);
        let _ = std::fs::remove_dir_all(b);
    }
}
