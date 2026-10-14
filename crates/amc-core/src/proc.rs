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
//!
//! It also holds the shared rule for which Gradle failures are a passing network hiccup worth
//! retrying ([`is_transient_download_failure`]), used by both the build and the Quick Test launch.

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

/// How many times a Gradle run that failed on a transient download problem is tried in total.
pub const DOWNLOAD_ATTEMPTS: u32 = 3;

/// Printed (on its own line, to stdout) before each retry of a download-failed Gradle run. The desktop
/// app watches the Quick Test log for it to tell the player what's happening; keep the text stable.
pub const DOWNLOAD_RETRY_MARKER: &str = "[amc] download problem, retrying";

/// Printed when a Gradle run still failed on a download problem after the last attempt.
pub const DOWNLOAD_GAVE_UP_MARKER: &str = "[amc] download problem persisted";

/// The wait before retry `attempt` (2 = the first retry): a short backoff that gives a flaky
/// connection or a busy Maven mirror a moment to recover.
pub fn download_retry_delay(attempt: u32) -> Duration {
    Duration::from_secs(if attempt <= 2 { 5 } else { 15 })
}

/// Did this Gradle log fail because a download hit a passing network problem (a dropped connection,
/// a timeout, a mirror's 5xx), so running it again will likely work? Pure.
///
/// Deliberately narrow: a real compile error or a dependency that doesn't exist (a 404, "Could not
/// find") fails the same way every time, so neither is retried — even if the log also mentions a
/// network word, a compile error wins.
pub fn is_transient_download_failure(log: &str) -> bool {
    const PERMANENT: &[&str] = &[
        "compileJava FAILED",
        "Compilation failed",
        "invalid target release",
        "has been compiled by a more recent version of the Java Runtime",
    ];
    if log.contains("BUILD SUCCESSFUL") || PERMANENT.iter().any(|m| log.contains(m)) {
        return false;
    }
    const TRANSIENT: &[&str] = &[
        "Could not GET",
        "Could not HEAD",
        "Could not download",
        "Failed to download",
        "Connection reset",
        "Connection refused",
        "Connection timed out",
        "Read timed out",
        "connect timed out",
        "Timeout waiting for connection",
        "Remote host terminated the handshake",
        "Remote host closed connection",
        "Premature end of Content-Length",
        "UnknownHostException",
        "SocketException",
        "SocketTimeoutException",
        "SSLHandshakeException",
        "HttpTimeoutException",
        "No route to host",
        "Network is unreachable",
        "Temporary failure in name resolution",
    ];
    // Gradle's "Received status code 503 from server: Service Unavailable" — only server-side (5xx)
    // and rate-limit (429) codes; a 404 means the file really isn't there.
    let retryable_status = log.split("Received status code ").skip(1).any(|rest| {
        let code: String = rest.chars().take(3).collect();
        code == "429" || (code.starts_with('5') && code.parse::<u16>().is_ok())
    });
    let not_found = log
        .split("Received status code ")
        .skip(1)
        .any(|rest| rest.starts_with("404"));
    if not_found && !retryable_status {
        return false;
    }
    retryable_status || TRANSIENT.iter().any(|m| log.contains(m))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_hiccups_are_transient() {
        for log in [
            "> Could not resolve net.fabricmc:fabric-loader:0.16.5.\n   > Could not GET 'https://maven.fabricmc.net/x.pom'.\n      > Connection reset",
            "Failed to download file from https://piston-data.mojang.com/v1/objects/abc/client.jar",
            "java.net.SocketTimeoutException: Read timed out\nBUILD FAILED",
            "Could not HEAD 'https://repo/x.jar'. Received status code 503 from server: Service Unavailable",
            "Received status code 429 from server: Too Many Requests",
            "java.net.UnknownHostException: maven.fabricmc.net",
        ] {
            assert!(is_transient_download_failure(log), "{log}");
        }
    }

    #[test]
    fn real_failures_are_not_retried() {
        for log in [
            "ModItems.java:12: error: cannot find symbol\n> Task :compileJava FAILED\nBUILD FAILED",
            "error: invalid target release: 21\nBUILD FAILED",
            "> Could not find net.fabricmc:fabric-api:9.9.9.\nBUILD FAILED",
            "> Could not GET 'https://repo/x.pom'. Received status code 404 from server: Not Found",
            "BUILD FAILED",
            "",
            "Could not GET 'https://x'. Connection reset\nBUILD SUCCESSFUL in 40s",
            // A compile error wins even when the log also mentions a network word.
            "Connection reset\n> Task :compileJava FAILED",
        ] {
            assert!(!is_transient_download_failure(log), "{log}");
        }
    }

    #[test]
    fn retry_delay_backs_off() {
        assert!(download_retry_delay(3) > download_retry_delay(2));
    }
}
