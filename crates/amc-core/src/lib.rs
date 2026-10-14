//! Shared primitives for the AI-mod creator.
//!
//! This crate is dependency-light and is used by every other crate. It provides:
//! - [`Component`]: the pipeline component taxonomy used for failure ownership (`SPEC-005 §17`).
//! - [`AmcError`]: a common error type, including the [`AmcError::Unimplemented`] variant used by
//!   Phase 0 stubs so that no unimplemented path can be mistaken for success (`SPEC-005 §18`).
//! - [`new_id`]: stable, sortable ID generation (`SPEC-004 §5.1`).
//! - [`canonical_hash`]: deterministic hashing that excludes variable metadata (`SPEC-005 §22`).
//! - [`init_logging`]: `tracing`-based structured logging.

pub mod error;
pub mod hash;
pub mod id;
pub mod log;
pub mod proc;

pub use error::{AmcError, Component, Result};
pub use hash::canonical_hash;
pub use id::new_id;
pub use log::init_logging;
pub use proc::{
    download_retry_delay, gradle_wrapper, hide_console, is_transient_download_failure,
    output_with_timeout, DOWNLOAD_ATTEMPTS, DOWNLOAD_GAVE_UP_MARKER, DOWNLOAD_RETRY_MARKER,
};
