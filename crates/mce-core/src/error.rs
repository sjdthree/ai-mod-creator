//! Common error type and the failure-ownership taxonomy.

use thiserror::Error;

/// The pipeline components, used to assign a single primary owner to every failure
/// (`SPEC-005 §17`). Kept as a small closed enum so failure ownership is always explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    Cli,
    IntentAdapter,
    Validator,
    StateManager,
    Generator,
    BuildManager,
    VerificationHarness,
    Launcher,
    Benchmark,
    Schema,
    Core,
}

impl Component {
    /// Stable machine-readable tag matching the `SPEC-005 §17` taxonomy where applicable.
    pub fn tag(self) -> &'static str {
        match self {
            Component::Cli => "CLI",
            Component::IntentAdapter => "MODEL_INTERPRETATION",
            Component::Validator => "SCHEMA_VALIDATION",
            Component::StateManager => "STATE_TRANSITION",
            Component::Generator => "GENERATOR",
            Component::BuildManager => "BUILD_ENVIRONMENT",
            Component::VerificationHarness => "GAMETEST",
            Component::Launcher => "CLIENT_LAUNCH",
            Component::Benchmark => "BENCHMARK_HARNESS",
            Component::Schema => "SCHEMA_VALIDATION",
            Component::Core => "CORE",
        }
    }
}

impl std::fmt::Display for Component {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.tag())
    }
}

/// The shared error type for MCE.
#[derive(Debug, Error)]
pub enum MceError {
    /// A capability that is defined but not yet implemented in the current phase. Carries the
    /// owning component so nothing can silently look like success (`SPEC-005 §18`).
    #[error("[{component}] not implemented: {what}")]
    Unimplemented { component: Component, what: String },

    /// A document or operation failed schema/structural validation.
    #[error("[{component}] validation failed: {message}")]
    Validation {
        component: Component,
        message: String,
    },

    /// An I/O failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A JSON (de)serialization failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl MceError {
    /// Construct an [`MceError::Unimplemented`] for the given component.
    pub fn unimplemented(component: Component, what: impl Into<String>) -> Self {
        MceError::Unimplemented {
            component,
            what: what.into(),
        }
    }

    /// Construct an [`MceError::Validation`] for the given component.
    pub fn validation(component: Component, message: impl Into<String>) -> Self {
        MceError::Validation {
            component,
            message: message.into(),
        }
    }

    /// The component that owns this failure, for reporting (`SPEC-005 §17`).
    pub fn owner(&self) -> Option<Component> {
        match self {
            MceError::Unimplemented { component, .. } => Some(*component),
            MceError::Validation { component, .. } => Some(*component),
            _ => None,
        }
    }
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, MceError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unimplemented_carries_owner() {
        let e = MceError::unimplemented(Component::Generator, "weapon generation");
        assert_eq!(e.owner(), Some(Component::Generator));
        assert!(e.to_string().contains("GENERATOR"));
    }
}
