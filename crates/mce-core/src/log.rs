//! Structured logging setup.

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initialize `tracing` logging once. Reads the `MCE_LOG` (falling back to `RUST_LOG`)
/// environment variable for the filter; defaults to `info`.
///
/// Safe to call multiple times — subsequent calls are ignored.
pub fn init_logging() {
    let filter = EnvFilter::try_from_env("MCE_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // `try_init` returns Err if a global subscriber is already set; that is fine.
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .try_init();
}
