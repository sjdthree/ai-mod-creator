//! Cross-platform helpers for spawning toolchain subprocesses.
//!
//! Two Windows papercuts these smooth over:
//! - The Gradle wrapper is `gradlew.bat` on Windows (the extensionless `gradlew` is a POSIX shell
//!   script Windows can't execute — running it yields `%1 is not a valid Win32 application`).
//! - A GUI app (the Tauri desktop shell) that spawns a console subprocess makes Windows pop up a
//!   console window for it — and for each of *that* process's own console children. `hide_console`
//!   suppresses those with `CREATE_NO_WINDOW`.
//!
//! Both are no-ops / trivial on macOS and Linux.

use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Run a command to completion but with a hard **timeout**: on timeout the child is killed and a
/// `TimedOut` error is returned, so a wedged toolchain (a hung Gradle build/verify) can never freeze
/// the app forever. stdout/stderr are drained on background threads throughout — Gradle emits far more
/// than a pipe buffer holds, so without continuous draining the child would block on write and a
/// perfectly healthy build would look "stuck".
pub fn output_with_timeout(mut cmd: Command, timeout: Duration) -> std::io::Result<Output> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let mut out = child.stdout.take();
    let mut err = child.stderr.take();
    let h_out = std::thread::spawn(move || {
        let mut b = Vec::new();
        if let Some(s) = out.as_mut() {
            let _ = s.read_to_end(&mut b);
        }
        b
    });
    let h_err = std::thread::spawn(move || {
        let mut b = Vec::new();
        if let Some(s) = err.as_mut() {
            let _ = s.read_to_end(&mut b);
        }
        b
    });
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            let stdout = h_out.join().unwrap_or_default();
            let stderr = h_err.join().unwrap_or_default();
            return Ok(Output {
                status,
                stdout,
                stderr,
            });
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("timed out after {}s", timeout.as_secs()),
            ));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// The Gradle wrapper script name for the host OS: `gradlew.bat` on Windows, else `gradlew`.
pub fn gradle_wrapper() -> &'static str {
    if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    }
}

/// Suppress the console window Windows would otherwise allocate when a windowless (GUI) parent
/// spawns a console subprocess. Applied to every toolchain spawn so Quick Test / build / verify
/// don't flash a terminal. No-op off Windows. Returns `cmd` for chaining.
pub fn hide_console(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW — run without allocating a console window.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}
