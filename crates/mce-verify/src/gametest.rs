//! GameTest runtime level (`SPEC-005 §15` L4) — the mechanism proven by the Phase 3 spike
//! (ADR-0006). Splits into a **pure** log parser (unit-tested here) and an **effectful** runner
//! that shells out to `gradle runGameTestServer` (needs the toolchain; use the dev container).
//!
//! `runGameTestServer` exits 0 when all required tests pass, and non-zero (the failed-test count)
//! otherwise — exactly the pass/fail signal the harness needs (`SPEC-005 §18`).

use std::path::Path;
use std::process::Command;

use mce_core::{gradle_wrapper, hide_console, Component, MceError, Result};

/// Parsed result of a GameTest run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameTestOutcome {
    pub passed: bool,
    pub total: usize,
}

/// Parse a `runGameTestServer` log into an outcome. Pure.
///
/// Recognizes the framework's summary lines, e.g.
/// `========= 2 GAME TESTS COMPLETE ...` and `All 2 required tests passed :)`.
pub fn parse_gametest_log(log: &str) -> GameTestOutcome {
    let total = log
        .lines()
        .find_map(|l| {
            let l = l.trim_start_matches(|c: char| !c.is_ascii_digit());
            if l.contains("GAME TESTS COMPLETE") {
                l.split_whitespace()
                    .next()
                    .and_then(|n| n.parse::<usize>().ok())
            } else {
                None
            }
        })
        .unwrap_or(0);
    let passed = log.contains("required tests passed")
        && !log.contains("required tests failed")
        && !log.contains("Missing test structure");
    GameTestOutcome { passed, total }
}

/// Run the generated project's GameTests headlessly and return the outcome.
///
/// `task` is the loader's headless GameTest Gradle task (`runGameTestServer` for NeoForge,
/// `runGametest` for Fabric — see `mce_generator::Loader::gametest_task`). Uses the project's
/// `./gradlew` if present, else the system `gradle`. Requires JDK 21 + network (first run downloads
/// the loader). Returns a clear owned error rather than any false success.
pub fn run_game_tests(project_root: &Path, task: &str) -> Result<GameTestOutcome> {
    // Absolute root so the wrapper path (in the *program*) isn't re-resolved against `current_dir`.
    let root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let project_root = root.as_path();
    // Use the OS-correct wrapper: `gradlew.bat` on Windows (running the extensionless POSIX `gradlew`
    // there fails with `%1 is not a valid Win32 application`, which surfaced as verify failing every
    // time). Fall back to a system `gradle` only when the project carries no wrapper.
    let wrapper = project_root.join(gradle_wrapper());
    let (program, mut args): (String, Vec<String>) = if wrapper.exists() {
        (wrapper.display().to_string(), vec![])
    } else {
        ("gradle".to_string(), vec![])
    };
    args.extend([
        "--no-daemon".into(),
        "--console=plain".into(),
        task.to_string(),
    ]);

    let mut command = Command::new(&program);
    command.args(&args).current_dir(project_root);
    hide_console(&mut command);
    // Hard 4-minute cap: a wedged in-game test can never hang the app forever (the "shows all the
    // steps, then nothing" freeze). A timeout returns a clear owned error, not a false success.
    let output = mce_core::output_with_timeout(command, std::time::Duration::from_secs(240))
        .map_err(|e| {
            let msg = if e.kind() == std::io::ErrorKind::TimedOut {
                "in-game verification timed out (over 4 minutes) — the test may be stuck; try again"
                    .to_string()
            } else {
                format!("failed to launch `{program} {task}`: {e}")
            };
            MceError::validation(Component::VerificationHarness, msg)
        })?;

    let mut log = String::from_utf8_lossy(&output.stdout).into_owned();
    log.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok(parse_gametest_log(&log))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_passed() {
        let log = "\
[Server thread/INFO]: ========= 2 GAME TESTS COMPLETE IN 182.8 ms ==============\n\
[Server thread/INFO]: All 2 required tests passed :)";
        let o = parse_gametest_log(log);
        assert!(o.passed);
        assert_eq!(o.total, 2);
    }

    #[test]
    fn parses_failure() {
        let log = "========= 3 GAME TESTS COMPLETE ...\n1 required tests failed :(";
        let o = parse_gametest_log(log);
        assert!(!o.passed);
        assert_eq!(o.total, 3);
    }

    #[test]
    fn missing_structure_is_not_a_pass() {
        let log = "java.lang.IllegalStateException: Missing test structure: mce_ruby_world:empty";
        assert!(!parse_gametest_log(log).passed);
    }
}
