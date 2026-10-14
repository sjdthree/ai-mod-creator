//! Build Manager (`SPEC-003 §10`) — Phase 3 work item `0001`.
//!
//! Executes the pinned Gradle build (Minecraft 1.21.1 / NeoForge 21.1.x / Java 21, ADR-0003) and
//! classifies the result. The build is split into **pure logic** (command construction, log
//! classification) that is fully unit-tested here, and an **effectful** invocation that shells out
//! to the Gradle wrapper and therefore needs a real toolchain + network.
//!
//! Prohibited (`SPEC-003 §10`): must not edit generated files, reinterpret user intent, or report
//! gameplay success. "It compiled" is never "it works" — that is the verification harness's call.

use std::path::Path;
use std::process::Command;

use amc_core::{
    download_retry_delay, hide_console, is_transient_download_failure, AmcError, Component, Result,
    DOWNLOAD_ATTEMPTS, DOWNLOAD_GAVE_UP_MARKER, DOWNLOAD_RETRY_MARKER,
};

/// A classified build result category (`SPEC-003 §10`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildOutcome {
    Success,
    /// Generated Java failed to compile — owned by the generator.
    CompileError,
    /// JDK/toolchain mismatch (e.g. Java 17 vs 21) — owned by the build environment.
    ToolchainMismatch,
    /// A dependency could not be resolved/downloaded — owned by the build environment.
    MissingDependency,
    /// A generic Gradle failure not otherwise classified.
    GradleFailure,
    /// Unrecognized output.
    Unknown,
}

impl BuildOutcome {
    /// Whether this outcome represents a successful build.
    pub fn is_success(self) -> bool {
        self == BuildOutcome::Success
    }

    /// The failure owner (`SPEC-005 §17`), or `None` on success.
    pub fn owner(self) -> Option<&'static str> {
        match self {
            BuildOutcome::Success => None,
            BuildOutcome::CompileError => Some("GENERATOR"),
            BuildOutcome::ToolchainMismatch
            | BuildOutcome::MissingDependency
            | BuildOutcome::GradleFailure
            | BuildOutcome::Unknown => Some("BUILD_ENVIRONMENT"),
        }
    }
}

/// The result of a build attempt (`SPEC-003 §10` outputs).
#[derive(Debug, Clone)]
pub struct BuildResult {
    pub outcome: BuildOutcome,
    pub jar_path: Option<String>,
    pub artifact_checksum: Option<String>,
    pub log: String,
}

/// Gradle distribution the wrapper is pinned to (matches the generated `gradle-wrapper.properties`
/// and the dev container, ADR-0003).
pub const GRADLE_VERSION: &str = "8.10.2";

/// Materialize the executable Gradle wrapper (`gradlew` + `gradle-wrapper.jar`) in a generated
/// project if it is missing. The generator emits `gradle-wrapper.properties` but not the jar/script
/// (they are binary/bootstrap artifacts), so we run the system `gradle wrapper` once — which needs
/// the toolchain (JDK 21 + a system Gradle, i.e. the dev container). No-op if `gradlew` exists.
pub fn ensure_wrapper(project_root: &Path) -> Result<()> {
    let wrapper = if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    };
    if project_root.join(wrapper).exists() {
        return Ok(());
    }
    let output = hide_console(
        Command::new("gradle")
            .args(["wrapper", "--gradle-version", GRADLE_VERSION])
            .current_dir(project_root),
    )
    .output()
    .map_err(|e| {
        AmcError::validation(
            Component::BuildManager,
            format!(
                "no Gradle wrapper and `gradle` is unavailable to create one ({e}); run inside \
                     the dev container (JDK 21 + Gradle, ADR-0003)"
            ),
        )
    })?;
    if !output.status.success() || !project_root.join(wrapper).exists() {
        let mut log = String::from_utf8_lossy(&output.stdout).into_owned();
        log.push_str(&String::from_utf8_lossy(&output.stderr));
        return Err(AmcError::validation(
            Component::BuildManager,
            format!(
                "`gradle wrapper` failed to materialize the wrapper: {}",
                log.trim()
            ),
        ));
    }
    Ok(())
}

/// Locate the built mod JAR under `build/libs/` and return `(path, sha256)`. NeoForge's ModDevGradle
/// writes the packaged mod there; we ignore `-sources`/`-javadoc` siblings.
pub fn discover_jar(project_root: &Path) -> Option<(String, String)> {
    use sha2::{Digest, Sha256};
    let libs = project_root.join("build").join("libs");
    let mut jars: Vec<std::path::PathBuf> = std::fs::read_dir(&libs)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|x| x == "jar")
                && !p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.ends_with("-sources") || s.ends_with("-javadoc"))
        })
        .collect();
    jars.sort();
    let jar = jars.into_iter().next()?;
    let bytes = std::fs::read(&jar).ok()?;
    let checksum = format!("sha256:{:x}", Sha256::digest(&bytes));
    Some((jar.display().to_string(), checksum))
}

/// The Gradle wrapper invocation for a project, as `(program, args)`. Deterministic and pure.
pub fn build_command(project_root: &Path) -> (String, Vec<String>) {
    let wrapper = if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    };
    let program = project_root.join(wrapper).display().to_string();
    // `--no-daemon` keeps CI deterministic; `build` compiles, tests, and packages the JAR.
    (program, vec!["--no-daemon".into(), "build".into()])
}

/// Classify a Gradle/`javac` build log into a [`BuildOutcome`]. Pure; the heart of the manager.
///
/// Specific signatures are checked before the generic `BUILD FAILED`, so a compile error or a
/// toolchain mismatch is never misattributed to the environment (or vice versa).
pub fn classify(log: &str) -> BuildOutcome {
    if log.contains("BUILD SUCCESSFUL") {
        return BuildOutcome::Success;
    }
    // Toolchain / JDK mismatch.
    if log.contains("has been compiled by a more recent version of the Java Runtime")
        || log.contains("invalid target release")
        || log.contains("No compatible toolchains")
        || log.contains("Could not find a Java installation")
    {
        return BuildOutcome::ToolchainMismatch;
    }
    // Dependency resolution.
    if log.contains("Could not resolve")
        || log.contains("Could not find")
        || log.contains("Could not download")
        || log.contains("Could not GET")
    {
        return BuildOutcome::MissingDependency;
    }
    // Java compilation failure of the generated sources.
    if log.contains("compileJava FAILED")
        || log.contains("Compilation failed")
        || (log.contains("error:") && log.contains(".java:"))
    {
        return BuildOutcome::CompileError;
    }
    if log.contains("BUILD FAILED") {
        return BuildOutcome::GradleFailure;
    }
    BuildOutcome::Unknown
}

/// Runs the pinned build.
pub trait BuildManager {
    fn build(&self, project_root: &Path) -> Result<BuildResult>;
}

/// The real Gradle-backed build manager. `build` shells out to the wrapper and therefore needs a
/// JDK 21 toolchain and network access on first run.
#[derive(Debug, Default)]
pub struct GradleBuildManager;

impl BuildManager for GradleBuildManager {
    fn build(&self, project_root: &Path) -> Result<BuildResult> {
        // Resolve to an absolute path: `build_command` puts the wrapper path in the *program* while
        // we also set `current_dir(root)`, so a relative root would be applied twice by the OS.
        let root = project_root
            .canonicalize()
            .unwrap_or_else(|_| project_root.to_path_buf());
        let project_root = root.as_path();

        // Bootstrap the wrapper if the generated project doesn't carry one yet.
        ensure_wrapper(project_root)?;

        let (program, args) = build_command(project_root);
        // A first build downloads Minecraft and the loader; a dropped connection there would otherwise
        // fail the whole create. Retry only that kind of failure, keeping each attempt's log.
        let mut log = String::new();
        let mut outcome = BuildOutcome::Unknown;
        for attempt in 1..=DOWNLOAD_ATTEMPTS {
            if attempt > 1 {
                let note =
                    format!("{DOWNLOAD_RETRY_MARKER} (attempt {attempt} of {DOWNLOAD_ATTEMPTS})\n");
                eprint!("{note}");
                log.push_str(&note);
                std::thread::sleep(download_retry_delay(attempt));
            }
            let mut command = Command::new(&program);
            command.args(&args).current_dir(project_root);
            hide_console(&mut command);
            // Hard 5-minute cap so a wedged Gradle can't hang the app forever. Generous enough for the
            // one-time first-run NeoForge download; a genuine hang (normal builds are ~10s) is caught.
            let output = amc_core::output_with_timeout(command, std::time::Duration::from_secs(300))
                .map_err(|e| {
                    let msg = if e.kind() == std::io::ErrorKind::TimedOut {
                        "the mod build timed out (over 5 minutes) — the toolchain may be stuck; try again"
                            .to_string()
                    } else {
                        format!("failed to launch Gradle wrapper `{program}`: {e}")
                    };
                    AmcError::validation(Component::BuildManager, msg)
                })?;
            let mut attempt_log = String::from_utf8_lossy(&output.stdout).into_owned();
            attempt_log.push_str(&String::from_utf8_lossy(&output.stderr));
            log.push_str(&attempt_log);
            // Classify only the last attempt, so an earlier attempt's download error can't mask what
            // actually happened in the final one.
            outcome = classify(&attempt_log);
            if !is_transient_download_failure(&attempt_log) {
                break;
            }
            if attempt == DOWNLOAD_ATTEMPTS {
                log.push_str(&format!("\n{DOWNLOAD_GAVE_UP_MARKER}\n"));
            }
        }

        // Surface the produced artifact on success so the caller can record/launch it.
        let (jar_path, artifact_checksum) = if outcome.is_success() {
            match discover_jar(project_root) {
                Some((path, sum)) => (Some(path), Some(sum)),
                None => (None, None),
            }
        } else {
            (None, None)
        };

        Ok(BuildResult {
            outcome,
            jar_path,
            artifact_checksum,
            log,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_command_targets_the_wrapper() {
        let (program, args) = build_command(Path::new("/tmp/proj"));
        assert!(program.ends_with("gradlew") || program.ends_with("gradlew.bat"));
        assert!(args.contains(&"build".to_string()));
    }

    #[test]
    fn classifies_success() {
        assert_eq!(
            classify("> Task :build\nBUILD SUCCESSFUL in 12s"),
            BuildOutcome::Success
        );
    }

    #[test]
    fn classifies_toolchain_mismatch_as_environment() {
        let log = "> Task :compileJava\nerror: invalid target release: 21\nBUILD FAILED";
        assert_eq!(classify(log), BuildOutcome::ToolchainMismatch);
        assert_eq!(classify(log).owner(), Some("BUILD_ENVIRONMENT"));
    }

    #[test]
    fn classifies_compile_error_as_generator() {
        let log = "path/ModItems.java:12: error: cannot find symbol\n> Task :compileJava FAILED\nBUILD FAILED";
        assert_eq!(classify(log), BuildOutcome::CompileError);
        assert_eq!(classify(log).owner(), Some("GENERATOR"));
    }

    #[test]
    fn classifies_missing_dependency_as_environment() {
        let log = "Could not resolve net.neoforged:neoforge:21.1.100\nBUILD FAILED";
        assert_eq!(classify(log), BuildOutcome::MissingDependency);
    }

    #[test]
    fn generic_failure_and_unknown() {
        assert_eq!(classify("BUILD FAILED"), BuildOutcome::GradleFailure);
        assert_eq!(classify("something odd"), BuildOutcome::Unknown);
    }

    #[test]
    fn missing_wrapper_is_a_clear_error_not_success() {
        let dir = std::env::temp_dir().join("amc-build-nonexistent-xyz");
        let err = GradleBuildManager.build(&dir).unwrap_err();
        assert_eq!(err.owner(), Some(Component::BuildManager));
    }

    #[cfg(unix)]
    #[test]
    fn build_retries_a_download_hiccup_then_classifies_the_last_attempt() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("amc-build-retry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // First run: a mirror's 503. Second run: a real compile error, which must not be retried.
        let script = "#!/bin/sh\nR=\"$(dirname \"$0\")/runs\"; echo run >> \"$R\"\n\
            if [ \"$(wc -l < \"$R\")\" -lt 2 ]; then\n\
            echo \"Could not GET 'https://x/y.pom'. Received status code 503 from server\"; echo 'BUILD FAILED'; exit 1\nfi\n\
            echo 'A.java:1: error: oops'; echo '> Task :compileJava FAILED'; echo 'BUILD FAILED'; exit 1\n";
        let gradlew = dir.join("gradlew");
        std::fs::write(&gradlew, script).unwrap();
        std::fs::set_permissions(&gradlew, std::fs::Permissions::from_mode(0o755)).unwrap();
        let res = GradleBuildManager.build(&dir).unwrap();
        let runs = std::fs::read_to_string(dir.join("runs")).unwrap();
        assert_eq!(runs.lines().count(), 2);
        assert_eq!(res.outcome, BuildOutcome::CompileError);
        assert!(res.log.contains(DOWNLOAD_RETRY_MARKER));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
