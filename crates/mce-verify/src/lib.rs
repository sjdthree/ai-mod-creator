//! Verification Harness core (`SPEC-003 §11`) — Phase 3 work item `0007`.
//!
//! The environment-independent core: model the verification levels (`SPEC-005 §15`), plan the
//! checks a transaction's verification requirements imply, and aggregate per-level results into a
//! structured report. Runtime-dependent levels (server boot, GameTest, client smoke) are supplied
//! by a [`LevelRunner`] implementation (Phase 3 `0002`–`0005`, environment-gated); the core here
//! stays fully testable with a fake runner.
//!
//! The defining rule (`SPEC-003 §11`, `SPEC-005 §18`): a report passes only when **all required**
//! checks pass. Compilation alone is never success.
//!
//! Prohibited (`SPEC-003 §11`): must not alter requested behavior, commit state, or treat build
//! success as gameplay success.

pub mod gametest;

use serde::Serialize;

use mce_state::VerificationRequirement;

/// The level at which a check runs (`SPEC-005 §15`). Finer-grained than the coarse level stored on
/// a requirement, because it is derived from the specific assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationLevel {
    Static,
    Build,
    ServerBoot,
    Registry,
    GameTest,
    Persistence,
    ClientSmoke,
}

impl VerificationLevel {
    /// The failure owner for this level (`SPEC-005 §17`).
    pub fn owner(self) -> &'static str {
        match self {
            VerificationLevel::Static => "SEMANTIC_VALIDATION",
            VerificationLevel::Build => "BUILD_ENVIRONMENT",
            VerificationLevel::ServerBoot => "MINECRAFT_ADAPTER",
            VerificationLevel::Registry | VerificationLevel::GameTest => "GAMETEST",
            VerificationLevel::Persistence => "GAMETEST",
            VerificationLevel::ClientSmoke => "CLIENT_LAUNCH",
        }
    }

    /// Map an MVP assertion type (`SPEC-004 §32`) to the level that checks it.
    pub fn for_assertion(assertion: &str) -> VerificationLevel {
        match assertion {
            "registry_exists" | "registry_absent" => VerificationLevel::Registry,
            "client_loads_mod" => VerificationLevel::ClientSmoke,
            "server_reaches_ready" => VerificationLevel::ServerBoot,
            "save_reload_preserves" => VerificationLevel::Persistence,
            // property_equals, recipe_produces, block_places/drops, weapon_damages, behaviour, ...
            _ => VerificationLevel::GameTest,
        }
    }
}

/// A single check the harness must run to satisfy a requirement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlannedCheck {
    pub level: VerificationLevel,
    pub assertion: String,
    pub target: Option<String>,
}

/// Plan the checks implied by a set of verification requirements.
pub fn plan(requirements: &[VerificationRequirement]) -> Vec<PlannedCheck> {
    requirements
        .iter()
        .map(|r| PlannedCheck {
            level: VerificationLevel::for_assertion(&r.assertion),
            assertion: r.assertion.clone(),
            target: r.target.clone(),
        })
        .collect()
}

/// The result of running one level's checks.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LevelResult {
    pub level: VerificationLevel,
    pub passed: bool,
    pub detail: String,
}

/// The structured verification report (`SPEC-003 §11` outputs).
#[derive(Debug, Clone, Serialize)]
pub struct VerificationReport {
    pub passed: bool,
    pub level_results: Vec<LevelResult>,
    /// Assertions of checks that failed.
    pub failed_assertions: Vec<String>,
    /// The owning component of the first failure (`SPEC-005 §17`).
    pub failure_owner: Option<String>,
}

/// Executes the runtime levels. Implemented by the environment-gated Phase 3 items; the core is
/// tested with a fake runner.
pub trait LevelRunner {
    /// Run all checks for one level, returning whether they passed and a human detail string.
    fn run_level(&self, level: VerificationLevel, checks: &[PlannedCheck]) -> LevelResult;
}

/// Run the full verification for a transaction.
///
/// `build_passed` is the `mce-build` outcome for level 2. Runtime levels only run if the build
/// passed; the report never claims success on a failed or skipped build.
pub fn verify(
    requirements: &[VerificationRequirement],
    build_passed: bool,
    runner: &dyn LevelRunner,
) -> VerificationReport {
    let mut level_results = Vec::new();
    let mut failed_assertions = Vec::new();
    let mut failure_owner: Option<String> = None;

    // Level 1: static validation is guaranteed upstream (schema/validator) by the time we get here.
    level_results.push(LevelResult {
        level: VerificationLevel::Static,
        passed: true,
        detail: "static validation passed upstream".into(),
    });

    // Level 2: build.
    level_results.push(LevelResult {
        level: VerificationLevel::Build,
        passed: build_passed,
        detail: if build_passed {
            "build succeeded".into()
        } else {
            "build failed".into()
        },
    });
    if !build_passed {
        // No runtime levels run; the report is a failure owned by the build environment.
        return VerificationReport {
            passed: false,
            level_results,
            failed_assertions,
            failure_owner: Some(VerificationLevel::Build.owner().to_string()),
        };
    }

    // Runtime levels: group planned checks by level and run each group in a stable order.
    let checks = plan(requirements);
    for level in [
        VerificationLevel::ServerBoot,
        VerificationLevel::Registry,
        VerificationLevel::GameTest,
        VerificationLevel::Persistence,
        VerificationLevel::ClientSmoke,
    ] {
        let group: Vec<PlannedCheck> = checks
            .iter()
            .filter(|c| c.level == level)
            .cloned()
            .collect();
        if group.is_empty() {
            continue;
        }
        let result = runner.run_level(level, &group);
        if !result.passed {
            for c in &group {
                failed_assertions.push(c.assertion.clone());
            }
            if failure_owner.is_none() {
                failure_owner = Some(level.owner().to_string());
            }
        }
        level_results.push(result);
    }

    let passed = failure_owner.is_none();
    VerificationReport {
        passed,
        level_results,
        failed_assertions,
        failure_owner,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mce_state::{requirements_for, ProjectStateManager};
    use serde_json::json;

    fn weapon_requirements() -> Vec<VerificationRequirement> {
        let mut sm = ProjectStateManager::new("Ruby World", "mce_ruby_world");
        let op = json!({
            "operation": "create_concept",
            "payload": {
                "concept_type": "weapon", "subtype": "sword", "display_name": "Ruby Blade",
                "properties": { "attack_damage": 9.0, "durability": 1200 }
            }
        });
        let tx = sm.open_candidate(std::slice::from_ref(&op)).unwrap();
        sm.commit(&tx.transaction_id, true).unwrap();
        requirements_for(&op, sm.canonical())
    }

    struct AlwaysPass;
    impl LevelRunner for AlwaysPass {
        fn run_level(&self, level: VerificationLevel, _checks: &[PlannedCheck]) -> LevelResult {
            LevelResult {
                level,
                passed: true,
                detail: "ok".into(),
            }
        }
    }

    struct FailLevel(VerificationLevel);
    impl LevelRunner for FailLevel {
        fn run_level(&self, level: VerificationLevel, _checks: &[PlannedCheck]) -> LevelResult {
            LevelResult {
                level,
                passed: level != self.0,
                detail: String::new(),
            }
        }
    }

    #[test]
    fn plan_maps_weapon_requirements_to_registry_and_gametest() {
        let checks = plan(&weapon_requirements());
        assert!(checks
            .iter()
            .any(|c| c.level == VerificationLevel::Registry));
        assert!(checks
            .iter()
            .any(|c| c.level == VerificationLevel::GameTest));
    }

    #[test]
    fn all_pass_reports_success() {
        let report = verify(&weapon_requirements(), true, &AlwaysPass);
        assert!(report.passed);
        assert!(report.failure_owner.is_none());
    }

    #[test]
    fn failed_build_never_reports_success_and_skips_runtime() {
        let report = verify(&weapon_requirements(), false, &AlwaysPass);
        assert!(!report.passed);
        assert_eq!(report.failure_owner.as_deref(), Some("BUILD_ENVIRONMENT"));
        // Only static + build levels recorded; no runtime levels ran.
        assert_eq!(report.level_results.len(), 2);
    }

    #[test]
    fn a_failing_gametest_fails_the_report_even_if_build_passed() {
        // "It compiled" is not "it works" (SPEC-003 §11).
        let report = verify(
            &weapon_requirements(),
            true,
            &FailLevel(VerificationLevel::GameTest),
        );
        assert!(!report.passed);
        assert_eq!(report.failure_owner.as_deref(), Some("GAMETEST"));
        assert!(!report.failed_assertions.is_empty());
    }
}
