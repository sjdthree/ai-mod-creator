//! Minecraft Launcher (`SPEC-003 §12`).
//!
//! Launches the pinned NeoForge development client via the generated project's `gradlew runClient`,
//! but only after the required verification gates pass (`SPEC-005 §18` — never launch unverified work).
//!
//! Two modes: an **interactive** run (foreground, a visible window on a machine with a display) and a
//! bounded **headless smoke** (`xvfb-run`, watch the client log for a readiness marker, then stop it)
//! so "the client boots and loads the mod" is automatable in the dev container.
//!
//! Prohibited (`SPEC-003 §12`): must not validate language-model output, generate source, or modify
//! canonical state.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use amc_core::{
    download_retry_delay, hide_console, is_transient_download_failure, AmcError, Component, Result,
    DOWNLOAD_ATTEMPTS, DOWNLOAD_GAVE_UP_MARKER, DOWNLOAD_RETRY_MARKER,
};

/// Result of a launch attempt.
#[derive(Debug, Clone)]
pub struct LaunchResult {
    pub started: bool,
    pub client_log_path: Option<String>,
}

/// Launches the pinned client.
pub trait Launcher {
    /// Launch only if `verification_passed` is true; otherwise this must refuse.
    fn launch(&self, project_root: &str, verification_passed: bool) -> Result<LaunchResult>;
}

/// The Gradle wrapper script for the host OS. On Windows the executable wrapper is `gradlew.bat`
/// (the extensionless `gradlew` is a POSIX shell script Windows can't run); everywhere else it's
/// `gradlew`. Mirrors `amc-build`'s wrapper selection so build and launch agree per platform.
fn gradle_wrapper() -> &'static str {
    if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    }
}

/// Log substrings that mean the NeoForge client has finished loading (mods registered, render/sound
/// backends up, main menu imminent). Any one is a sufficient readiness signal for the smoke.
const READY_MARKERS: &[&str] = &[
    "Sound engine started",
    "Created: 1024x512x4 minecraft:textures/atlas/blocks.png",
    "Backend library: LWJGL",
    "[Render thread/INFO] [minecraft/LoadingOverlay]",
];

/// Real launcher backed by the generated project's Gradle `runClient` task (declared in the
/// generated `build.gradle`). Requires JDK 21 (the dev container).
#[derive(Debug, Clone)]
pub struct GradleLauncher {
    /// Wrap the run in `xvfb-run` for a headless display (dev container / CI).
    pub headless: bool,
    /// When set, run as a bounded smoke: stop the client once it reports ready (or this elapses),
    /// and treat "ready before timeout" as success. When `None`, run interactively to exit.
    pub smoke_timeout: Option<Duration>,
}

impl Default for GradleLauncher {
    /// A headless, bounded client smoke — the automatable default.
    fn default() -> Self {
        GradleLauncher {
            headless: true,
            smoke_timeout: Some(Duration::from_secs(300)),
        }
    }
}

impl GradleLauncher {
    /// Interactive, visible run (foreground until the window closes). For a host with a display.
    pub fn interactive() -> Self {
        GradleLauncher {
            headless: false,
            smoke_timeout: None,
        }
    }

    fn command(&self, project_root: &Path, log_path: &Path) -> Command {
        let gradlew = project_root.join(gradle_wrapper());
        let (program, pre_args): (String, Vec<String>) = if self.headless {
            (
                "xvfb-run".into(),
                vec!["-a".into(), gradlew.display().to_string()],
            )
        } else {
            (gradlew.display().to_string(), vec![])
        };
        let mut cmd = Command::new(program);
        cmd.args(pre_args)
            .args(["--no-daemon", "--console=plain", "runClient"])
            .current_dir(project_root);
        // No console window when the desktop app (a GUI process) drives the launch — the Minecraft
        // window itself is a GL window, unaffected. The client's stdout/stderr are teed into the log
        // so the smoke can watch for readiness.
        hide_console(&mut cmd);
        let _ = log_path; // interactive mode inherits the terminal; smoke pipes (set below).
        cmd
    }
}

impl Launcher for GradleLauncher {
    fn launch(&self, project_root: &str, verification_passed: bool) -> Result<LaunchResult> {
        if !verification_passed {
            // The launcher must refuse unverified work (SPEC-005 §18).
            return Err(AmcError::validation(
                Component::Launcher,
                "refusing to launch: verification did not pass",
            ));
        }
        // Absolute root so the wrapper path (in the *program*) isn't re-resolved against `current_dir`.
        let root = PathBuf::from(project_root);
        let root = root.canonicalize().unwrap_or(root);
        if !root.join(gradle_wrapper()).exists() {
            return Err(AmcError::validation(
                Component::Launcher,
                format!(
                    "no Gradle wrapper ({}) at {}; build the project first (`amc request`/`amc test`)",
                    gradle_wrapper(),
                    root.display()
                ),
            ));
        }
        let log_path = root.join("build").join("amc-client.log");

        match self.smoke_timeout {
            None => self.run_interactive(&root, &log_path),
            Some(timeout) => self.run_smoke(&root, &log_path, timeout),
        }
    }
}

impl GradleLauncher {
    /// Foreground run: block until the client exits, passing its output through. If it fails before
    /// the client comes up because a download hit a network hiccup (the first run fetches Minecraft
    /// and the loader), print [`DOWNLOAD_RETRY_MARKER`] and run it again, up to [`DOWNLOAD_ATTEMPTS`].
    fn run_interactive(&self, root: &Path, log_path: &Path) -> Result<LaunchResult> {
        for attempt in 1..=DOWNLOAD_ATTEMPTS {
            if attempt > 1 {
                println!("{DOWNLOAD_RETRY_MARKER} (attempt {attempt} of {DOWNLOAD_ATTEMPTS})");
                let _ = std::io::stdout().flush();
                std::thread::sleep(download_retry_delay(attempt));
            }
            let (success, early_log) = self.run_teed(root, log_path)?;
            // Once the client is up, a failure is the game's, not a download's: never rerun it.
            let came_up = READY_MARKERS.iter().any(|m| early_log.contains(m));
            if success || came_up || !is_transient_download_failure(&early_log) {
                return Ok(LaunchResult {
                    started: success,
                    client_log_path: None,
                });
            }
            if attempt == DOWNLOAD_ATTEMPTS {
                println!("{DOWNLOAD_GAVE_UP_MARKER}");
            }
        }
        Ok(LaunchResult {
            started: false,
            client_log_path: None,
        })
    }

    /// Run the client once, copying its stdout/stderr through to ours line by line while keeping the
    /// start of it (up to [`EARLY_LOG_LIMIT`] bytes — the Gradle setup, where downloads happen) for
    /// [`is_transient_download_failure`]. Returns whether it exited successfully, and that log.
    fn run_teed(&self, root: &Path, log_path: &Path) -> Result<(bool, String)> {
        let mut child = self
            .command(root, log_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| launch_err(format!("failed to launch client: {e}")))?;
        let early = Arc::new(Mutex::new(String::new()));
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let h_out = tee(stdout, std::io::stdout(), Arc::clone(&early));
        let h_err = tee(stderr, std::io::stderr(), Arc::clone(&early));
        let status = child
            .wait()
            .map_err(|e| launch_err(format!("the client stopped unexpectedly: {e}")))?;
        let _ = h_out.join();
        let _ = h_err.join();
        let log = early.lock().map(|l| l.clone()).unwrap_or_default();
        Ok((status.success(), log))
    }

    /// Bounded smoke: pipe the client's output, watch for a readiness marker, then terminate. The
    /// client never exits on its own (it sits at the menu), so a clean smoke *must* stop it.
    fn run_smoke(&self, root: &Path, log_path: &Path, timeout: Duration) -> Result<LaunchResult> {
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let mut child = self
            .command(root, log_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| launch_err(format!("failed to launch client: {e}")))?;

        let stdout = child.stdout.take().expect("piped stdout");
        let deadline = Instant::now() + timeout;
        let mut log = String::new();
        let mut ready = false;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        // Read line-by-line until a readiness marker, EOF, or the deadline.
        while Instant::now() < deadline {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break, // client exited on its own (usually a crash during load)
                Ok(_) => {
                    log.push_str(&line);
                    if READY_MARKERS.iter().any(|m| line.contains(m)) {
                        ready = true;
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        // Stop the client regardless of outcome — smoke never leaves a process running.
        let _ = child.kill();
        let _ = child.wait();
        std::fs::write(log_path, &log).ok();

        Ok(LaunchResult {
            started: ready,
            client_log_path: Some(log_path.display().to_string()),
        })
    }
}

/// How much of the start of an interactive run's output is kept for the download-failure check.
/// Gradle's setup and any download errors come first; the game's own endless log is not needed.
const EARLY_LOG_LIMIT: usize = 512 * 1024;

/// Copy `from` to `to` line by line on a background thread, appending to `early` until it is full.
fn tee<R, W>(from: R, mut to: W, early: Arc<Mutex<String>>) -> std::thread::JoinHandle<()>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    std::thread::spawn(move || {
        let mut reader = BufReader::new(from);
        let mut line = Vec::new();
        while matches!(reader.read_until(b'\n', &mut line), Ok(n) if n > 0) {
            let _ = to.write_all(&line);
            let _ = to.flush();
            if let Ok(mut log) = early.lock() {
                if log.len() < EARLY_LOG_LIMIT {
                    log.push_str(&String::from_utf8_lossy(&line));
                }
            }
            line.clear();
        }
    })
}

fn launch_err(msg: impl Into<String>) -> AmcError {
    AmcError::validation(Component::Launcher, msg)
}

/// Phase 0 stub, kept for callers that must not launch.
#[derive(Debug, Default)]
pub struct UnimplementedLauncher;

impl Launcher for UnimplementedLauncher {
    fn launch(&self, _project_root: &str, _verification_passed: bool) -> Result<LaunchResult> {
        Err(AmcError::unimplemented(Component::Launcher, "launch"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_reports_unimplemented_owned_by_launcher() {
        let l = UnimplementedLauncher;
        let err = l.launch("project", true).unwrap_err();
        assert_eq!(err.owner(), Some(Component::Launcher));
    }

    #[test]
    fn real_launcher_refuses_unverified() {
        let l = GradleLauncher::default();
        let err = l.launch("project", false).unwrap_err();
        assert_eq!(err.owner(), Some(Component::Launcher));
        assert!(err.to_string().contains("refusing to launch"));
    }

    #[test]
    fn real_launcher_needs_a_wrapper() {
        // Verified but no gradlew present → a clear owned error, not a panic.
        let l = GradleLauncher::default();
        let err = l.launch("/nonexistent/project/path", true).unwrap_err();
        assert_eq!(err.owner(), Some(Component::Launcher));
    }

    #[test]
    fn wrapper_matches_host_os() {
        // The launcher must invoke the OS-correct Gradle wrapper: `gradlew.bat` on Windows
        // (the extensionless `gradlew` is a shell script Windows can't execute), else `gradlew`.
        let w = gradle_wrapper();
        if cfg!(windows) {
            assert_eq!(w, "gradlew.bat");
        } else {
            assert_eq!(w, "gradlew");
        }
    }

    #[test]
    fn missing_wrapper_error_names_the_platform_wrapper() {
        // A verified project without a wrapper yields an owned error that mentions the exact
        // wrapper we looked for — so a Windows user sees `gradlew.bat`, not a Unix hint.
        let l = GradleLauncher::interactive();
        let err = l.launch("/nonexistent/project/path", true).unwrap_err();
        assert_eq!(err.owner(), Some(Component::Launcher));
        assert!(err.to_string().contains(gradle_wrapper()));
    }

    #[test]
    fn default_is_a_bounded_headless_smoke() {
        let l = GradleLauncher::default();
        assert!(l.headless && l.smoke_timeout.is_some());
        let i = GradleLauncher::interactive();
        assert!(!i.headless && i.smoke_timeout.is_none());
    }

    /// A project whose `gradlew` runs `body` (a POSIX shell snippet) and counts its runs in `runs`.
    #[cfg(unix)]
    fn fake_project(name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("amc-launch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = format!("#!/bin/sh\necho run >> \"$(dirname \"$0\")/runs\"\n{body}\n");
        let gradlew = dir.join("gradlew");
        std::fs::write(&gradlew, script).unwrap();
        std::fs::set_permissions(&gradlew, std::fs::Permissions::from_mode(0o755)).unwrap();
        dir
    }

    #[cfg(unix)]
    fn runs(dir: &Path) -> usize {
        std::fs::read_to_string(dir.join("runs"))
            .unwrap_or_default()
            .lines()
            .count()
    }

    #[cfg(unix)]
    #[test]
    fn interactive_run_retries_a_download_hiccup() {
        // Fails with a dropped connection the first time, then works.
        let dir = fake_project(
            "retry",
            "if [ \"$(wc -l < \"$(dirname \"$0\")/runs\")\" -lt 2 ]; then\n\
             echo \"> Could not GET 'https://maven.fabricmc.net/x.pom'. Connection reset\"; exit 1\nfi\nexit 0",
        );
        let res = GradleLauncher::interactive()
            .launch(dir.to_str().unwrap(), true)
            .unwrap();
        assert!(res.started);
        assert_eq!(runs(&dir), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn interactive_run_does_not_retry_a_real_failure() {
        let dir = fake_project(
            "noretry",
            "echo 'Foo.java:3: error: cannot find symbol'; echo '> Task :compileJava FAILED'; exit 1",
        );
        let res = GradleLauncher::interactive()
            .launch(dir.to_str().unwrap(), true)
            .unwrap();
        assert!(!res.started);
        assert_eq!(runs(&dir), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
