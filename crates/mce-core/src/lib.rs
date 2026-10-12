//! Shared primitives for the Minecraft Creation Engine.
//!
//! This crate is dependency-light and is used by every other crate. It provides:
//! - [`Component`]: the pipeline component taxonomy used for failure ownership (`SPEC-005 §17`).
//! - [`MceError`]: a common error type, including the [`MceError::Unimplemented`] variant used by
//!   Phase 0 stubs so that no unimplemented path can be mistaken for success (`SPEC-005 §18`).
//! - [`new_id`]: stable, sortable ID generation (`SPEC-004 §5.1`).
//! - [`canonical_hash`]: deterministic hashing that excludes variable metadata (`SPEC-005 §22`).
//! - [`init_logging`]: `tracing`-based structured logging.

pub mod error;
pub mod hash;
pub mod id;
pub mod log;
pub mod proc;

pub use error::{Component, MceError, Result};
pub use hash::canonical_hash;
pub use id::new_id;
pub use log::init_logging;
pub use proc::{gradle_wrapper, hide_console, output_with_timeout};
