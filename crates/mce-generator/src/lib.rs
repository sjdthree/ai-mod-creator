//! Deterministic Generator (`SPEC-003 §9`) — Phase 2.
//!
//! Converts canonical state into a buildable Minecraft mod project: Gradle files, mod metadata, Java
//! registration, client resources, and placeholder textures. The same state + generator version +
//! loader produce byte-identical output (`SPEC-004 §4`, `SPEC-005 §22`), proven by the generation
//! manifest's `output_hash`.
//!
//! **Multi-loader (ADR-0007).** The loader-agnostic half of generation — the vanilla data-pack
//! resources, recipes, textures, GameTest structure, and the Gradle wrapper — lives in [`shared`].
//! Each loader is a thin module over that seam:
//! - [`NeoForgeGenerator`] (`neoforge`) — the default, ADR-0003 target. Output is unchanged from the
//!   original single-loader generator.
//! - [`FabricGenerator`] (`fabric`) — a Fabric Loom / fabric-api target (scaffold; see ADR-0007).
//!
//! Pick a generator with [`generator_for`]; select a [`Loader`] by name via `Loader::from_str`.
//!
//! Prohibited (`SPEC-003 §9`): must not call the language model, interpret English, mutate
//! canonical state, or repair itself through free-form model output.
//!
//! Scope: `item`, `weapon`, `tool` (pickaxe/axe/shovel/hoe), `food`, and `block` concepts all
//! generate real, type-appropriate Java + resources, and every item is placed in its **vanilla
//! creative tab** so a player can find it in-game. Recipes (shaped/shapeless/smelting/blasting)
//! generate as data-pack JSON.

pub mod capability;
pub mod fabric;
pub mod manifest;
pub mod neoforge;
pub mod shared;
pub mod structure;
pub mod texture;

use std::path::{Path, PathBuf};
use std::str::FromStr;

use mce_core::{Component, MceError, Result};
use mce_state::CanonicalState;

use crate::manifest::GenerationManifest;

pub use crate::fabric::FabricGenerator;
pub use crate::neoforge::NeoForgeGenerator;

/// The generator version, recorded in the transaction and manifest (`SPEC-004 §31`).
pub const GENERATOR_VERSION: &str = "0.1.0";

/// The result of a generation run.
#[derive(Debug, Clone)]
pub struct Generated {
    pub output_root: PathBuf,
    pub manifest: GenerationManifest,
}

impl Generated {
    /// The stable output hash proving reproducibility (`SPEC-005 §22`).
    pub fn output_hash(&self) -> &str {
        &self.manifest.output_hash
    }
}

/// Converts canonical state into a buildable Minecraft project for one mod loader.
pub trait Generator {
    /// The generator version (`SPEC-004 §31`).
    fn version(&self) -> &str;
    /// The concrete target string recorded in the manifest, e.g. `neoforge-1.21.1`.
    fn target(&self) -> &str;
    /// Materialize the project under `output_root`.
    fn generate(&self, state: &CanonicalState, output_root: &Path) -> Result<Generated>;
}

/// The mod loader a generation targets (ADR-0007). NeoForge is the default (ADR-0003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Loader {
    /// NeoForge 1.21.1 (ADR-0003) — the default, fully verified target.
    #[default]
    NeoForge,
    /// Fabric Loom / fabric-api 1.21.1 (ADR-0007) — scaffold, not yet CI-proven.
    Fabric,
}

impl Loader {
    /// The lower-case identifier used on the CLI and in the manifest (`neoforge` / `fabric`).
    pub fn as_str(self) -> &'static str {
        match self {
            Loader::NeoForge => "neoforge",
            Loader::Fabric => "fabric",
        }
    }

    /// The concrete manifest target string for this loader.
    pub fn target(self) -> &'static str {
        match self {
            Loader::NeoForge => neoforge::MINECRAFT_TARGET,
            Loader::Fabric => fabric::MINECRAFT_TARGET,
        }
    }

    /// The Gradle task that runs this loader's GameTests headlessly (`mce-verify` uses this).
    /// NeoForge exposes a dedicated `runGameTestServer`; Fabric declares a `runGametest` run config
    /// in its generated `build.gradle`.
    pub fn gametest_task(self) -> &'static str {
        match self {
            Loader::NeoForge => "runGameTestServer",
            Loader::Fabric => "runGametest",
        }
    }

    /// Every supported loader, for CLI help / iteration.
    pub fn all() -> &'static [Loader] {
        &[Loader::NeoForge, Loader::Fabric]
    }
}

impl FromStr for Loader {
    type Err = MceError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "neoforge" | "neo" => Ok(Loader::NeoForge),
            "fabric" => Ok(Loader::Fabric),
            other => Err(MceError::validation(
                Component::Generator,
                format!("unknown loader '{other}' (expected 'neoforge' or 'fabric')"),
            )),
        }
    }
}

impl std::fmt::Display for Loader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The generator for a loader.
pub fn generator_for(loader: Loader) -> Box<dyn Generator> {
    match loader {
        Loader::NeoForge => Box::new(NeoForgeGenerator),
        Loader::Fabric => Box::new(FabricGenerator),
    }
}

/// Guard against a caller passing a path that does not exist yet.
pub fn ensure_output_root(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).map_err(MceError::from)?;
    if !path.is_dir() {
        return Err(MceError::validation(
            Component::Generator,
            "output root is not a directory",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_round_trips_through_str() {
        for loader in Loader::all() {
            assert_eq!(Loader::from_str(loader.as_str()).unwrap(), *loader);
        }
        assert_eq!(Loader::from_str("NeoForge").unwrap(), Loader::NeoForge);
        assert_eq!(Loader::from_str(" fabric ").unwrap(), Loader::Fabric);
        assert!(Loader::from_str("forge").is_err());
    }

    #[test]
    fn default_loader_is_neoforge() {
        assert_eq!(Loader::default(), Loader::NeoForge);
        assert_eq!(generator_for(Loader::default()).target(), "neoforge-1.21.1");
    }

    #[test]
    fn factory_returns_matching_target_and_task() {
        assert_eq!(generator_for(Loader::NeoForge).target(), "neoforge-1.21.1");
        assert_eq!(generator_for(Loader::Fabric).target(), "fabric-1.21.1");
        assert_eq!(Loader::NeoForge.gametest_task(), "runGameTestServer");
        assert_eq!(Loader::Fabric.gametest_task(), "runGametest");
    }
}
