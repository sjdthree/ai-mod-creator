//! On-disk project store — carries canonical state across `amc` invocations.
//!
//! A project directory holds:
//!   `.amc/state.json`  — the persisted canonical state + transaction bookkeeping (this module);
//!   `mod/`             — the generated, buildable NeoForge project (written by the generator).
//!
//! Canonical state is the source of truth; the `mod/` project is a pure function of it and is
//! regenerated on every `request`. (History is not yet persisted, so cross-session `revert` is a
//! follow-on; everything else works from the restored canonical state.)

use std::path::{Path, PathBuf};

use amc_core::{AmcError, Component, Result};
use amc_state::{CanonicalState, ProjectStateManager};
use serde::{Deserialize, Serialize};

const STORE_DIR: &str = ".amc";
/// The store's pre-rename name; moved to [`STORE_DIR`] the first time a project is opened.
const LEGACY_STORE_DIR: &str = ".mce";
const STATE_FILE: &str = "state.json";
/// Subdirectory holding the generated NeoForge project (the Gradle build root).
pub const MOD_SUBDIR: &str = "mod";

/// The serialized project store.
#[derive(Debug, Serialize, Deserialize)]
struct PersistedState {
    canonical: CanonicalState,
    tx_counter: u64,
    head: Option<String>,
    /// `(transaction_id, snapshot)` of every committed transaction, so `amc revert` works across
    /// sessions (FR-006). Defaulted for stores written before history was persisted.
    #[serde(default)]
    history: Vec<HistoryEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HistoryEntry {
    transaction_id: String,
    snapshot: CanonicalState,
}

/// `.amc/` directory inside a project (a pre-rename `.mce/` is moved there on first use; if the move
/// fails, it keeps being used where it is).
pub fn store_dir(project: &Path) -> PathBuf {
    let dir = project.join(STORE_DIR);
    let legacy = project.join(LEGACY_STORE_DIR);
    if !dir.exists() && legacy.is_dir() && std::fs::rename(&legacy, &dir).is_err() {
        return legacy;
    }
    dir
}

/// The generated NeoForge project directory (`<project>/mod`).
pub fn mod_dir(project: &Path) -> PathBuf {
    project.join(MOD_SUBDIR)
}

/// Whether `project` has been initialized as an AMC project.
pub fn is_initialized(project: &Path) -> bool {
    store_dir(project).join(STATE_FILE).exists()
}

/// Derive a mod id / namespace from a human project name: `Ruby World` -> `amc_ruby_world`.
pub fn namespace_for(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let slug = slug.trim_matches('_').replace("__", "_");
    format!("amc_{slug}")
}

/// Initialize a new project: create the dir + `.amc/` and persist an empty canonical state.
pub fn init(project: &Path, name: &str) -> Result<()> {
    if is_initialized(project) {
        return Err(AmcError::validation(
            Component::StateManager,
            format!("{} is already an AMC project", project.display()),
        ));
    }
    std::fs::create_dir_all(store_dir(project))?;
    let sm = ProjectStateManager::new(name, namespace_for(name));
    save(project, &sm)?;
    Ok(())
}

/// Load a project's state manager from `.amc/state.json`.
pub fn load(project: &Path) -> Result<ProjectStateManager> {
    let path = store_dir(project).join(STATE_FILE);
    let bytes = std::fs::read(&path).map_err(|e| {
        AmcError::validation(
            Component::StateManager,
            format!(
                "{} is not an AMC project ({e}); run `amc init` there first",
                project.display()
            ),
        )
    })?;
    let persisted: PersistedState = serde_json::from_slice(&bytes)?;
    let history = persisted
        .history
        .into_iter()
        .map(|e| (e.transaction_id, e.snapshot))
        .collect();
    Ok(ProjectStateManager::restore(
        persisted.canonical,
        persisted.tx_counter,
        persisted.head,
        history,
    ))
}

/// Persist a state manager's canonical state + bookkeeping to `.amc/state.json` (atomically).
pub fn save(project: &Path, sm: &ProjectStateManager) -> Result<()> {
    std::fs::create_dir_all(store_dir(project))?;
    let persisted = PersistedState {
        canonical: sm.canonical().clone(),
        tx_counter: sm.tx_counter(),
        head: sm.head().map(str::to_owned),
        history: sm
            .committed_history()
            .into_iter()
            .map(|(transaction_id, snapshot)| HistoryEntry {
                transaction_id,
                snapshot,
            })
            .collect(),
    };
    let path = store_dir(project).join(STATE_FILE);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&persisted)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_slug() {
        assert_eq!(namespace_for("Ruby World"), "amc_ruby_world");
        assert_eq!(namespace_for("my-cool_mod!"), "amc_my_cool_mod");
    }

    #[test]
    fn init_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("amc-proj-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!is_initialized(&dir));
        init(&dir, "Test Project").unwrap();
        assert!(is_initialized(&dir));
        // Second init refuses.
        assert!(init(&dir, "Test Project").is_err());
        let sm = load(&dir).unwrap();
        assert_eq!(sm.canonical().namespace, "amc_test_project");
        std::fs::remove_dir_all(&dir).ok();
    }
}
