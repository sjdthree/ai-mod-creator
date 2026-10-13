//! The model manifest (`amc-model.json`) — the compatibility contract between this engine and a
//! separately-distributed AMC Model.
//!
//! The engine and the model live in different repositories and ship on different schedules, so
//! "they were built from the same checkout" can no longer guarantee they agree. The manifest makes
//! that agreement explicit and checkable: it records the sha256 of the exact CONTRACT (system
//! prompt) and grammar the model was trained/served against, plus the sha256 of the weights file.
//! [`ModelManifest::verify`] refuses a model whose contract or grammar differs from the bytes this
//! engine embeds — the train/serve-skew failure that silently wrecks a 270M model's output.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{CONTRACT, GRAMMAR};

/// `model.lock` at the repository root: which released model this engine ships with.
const MODEL_LOCK: &str = include_str!("../../../model.lock");

/// The model pinned in `model.lock`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockedModel {
    /// Model name — the model directory name and the provenance recorded in transactions.
    pub name: String,
    /// Hugging Face repository, e.g. `sjdthree/ai-mod-create-270smol`.
    pub repo: String,
    /// Pinned revision (tag) of that repository.
    pub revision: String,
    /// sha256 of that revision's `amc-model.json`.
    pub manifest_sha256: String,
}

/// Parse `model.lock` (`key=value` lines, `#` comments).
pub fn locked_model() -> LockedModel {
    let get = |key: &str| {
        MODEL_LOCK
            .lines()
            .map(str::trim)
            .filter(|l| !l.starts_with('#'))
            .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
            .unwrap_or_else(|| panic!("model.lock is missing `{key}`"))
            .trim()
            .to_string()
    };
    LockedModel {
        name: get("name"),
        repo: get("repo"),
        revision: get("revision"),
        manifest_sha256: get("manifest_sha256"),
    }
}

/// File name of the manifest inside a model directory.
pub const MANIFEST_FILE: &str = "amc-model.json";

/// Manifest format version this engine reads and writes.
pub const MANIFEST_FORMAT: u32 = 1;

/// Hex sha256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Hex sha256 of the CONTRACT embedded in this engine.
pub fn contract_sha256() -> String {
    sha256_hex(CONTRACT.as_bytes())
}

/// Hex sha256 of the grammar embedded in this engine.
pub fn grammar_sha256() -> String {
    sha256_hex(GRAMMAR.as_bytes())
}

/// Streaming sha256 of a (possibly large) file.
pub fn file_sha256(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// `amc-model.json`: identity, provenance, and compatibility pins for one promoted model build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelManifest {
    /// Manifest format version ([`MANIFEST_FORMAT`]).
    pub format: u32,
    /// Model name, e.g. `ai-mod-create-270smol` — recorded as provenance in every transaction.
    pub name: String,
    /// Model release version, e.g. `1.0.0`.
    pub version: String,
    /// Upstream base model the weights derive from, e.g. `google/functiongemma-270m-it`.
    pub base_model: String,
    /// License identifier governing the weights, e.g. `gemma`.
    pub license: String,
    /// Weights file name, relative to the manifest's directory.
    pub gguf: String,
    /// sha256 of the weights file.
    pub gguf_sha256: String,
    /// sha256 of the CONTRACT the model was trained against.
    pub contract_sha256: String,
    /// sha256 of the grammar the model is served with.
    pub grammar_sha256: String,
}

impl ModelManifest {
    /// Build a manifest for a weights file, pinning it to this engine's CONTRACT and grammar.
    pub fn for_gguf(
        gguf: &Path,
        name: &str,
        version: &str,
        base_model: &str,
        license: &str,
    ) -> std::io::Result<Self> {
        let file_name = gguf
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| std::io::Error::other("gguf path has no file name"))?;
        Ok(ModelManifest {
            format: MANIFEST_FORMAT,
            name: name.to_string(),
            version: version.to_string(),
            base_model: base_model.to_string(),
            license: license.to_string(),
            gguf: file_name,
            gguf_sha256: file_sha256(gguf)?,
            contract_sha256: contract_sha256(),
            grammar_sha256: grammar_sha256(),
        })
    }

    /// Read `<dir>/amc-model.json`.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join(MANIFEST_FILE);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("can't read {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("invalid {}: {e}", path.display()))
    }

    /// Check this manifest against the engine and the files in `dir`. Returns every problem found
    /// (empty = compatible). `check_weights` re-hashes the weights file — worth doing when staging a
    /// release; skippable at app startup where the bundle was already verified.
    pub fn verify(&self, dir: &Path, check_weights: bool) -> Vec<String> {
        let mut problems = Vec::new();
        if self.format != MANIFEST_FORMAT {
            problems.push(format!(
                "manifest format {} is not supported (this engine reads format {MANIFEST_FORMAT})",
                self.format
            ));
        }
        if self.contract_sha256 != contract_sha256() {
            problems.push(format!(
                "model was trained against a different contract ({}) than this engine embeds ({})",
                self.contract_sha256,
                contract_sha256()
            ));
        }
        if self.grammar_sha256 != grammar_sha256() {
            problems.push(format!(
                "model expects a different grammar ({}) than this engine embeds ({})",
                self.grammar_sha256,
                grammar_sha256()
            ));
        }
        let weights = dir.join(&self.gguf);
        if !weights.exists() {
            problems.push(format!("weights file {} is missing", weights.display()));
        } else if check_weights {
            match file_sha256(&weights) {
                Ok(h) if h == self.gguf_sha256 => {}
                Ok(h) => problems.push(format!(
                    "weights file {} has sha256 {h}, manifest pins {}",
                    weights.display(),
                    self.gguf_sha256
                )),
                Err(e) => problems.push(format!("can't hash {}: {e}", weights.display())),
            }
        }
        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("amc-manifest-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fixture(tag: &str) -> (std::path::PathBuf, ModelManifest) {
        let dir = temp_dir(tag);
        let gguf = dir.join("m.gguf");
        std::fs::write(&gguf, b"weights").unwrap();
        let m = ModelManifest::for_gguf(&gguf, "amc-model-v1", "1.0.0", "base", "gemma").unwrap();
        (dir, m)
    }

    #[test]
    fn model_lock_parses() {
        let m = locked_model();
        assert_eq!(m.repo.split('/').nth(1), Some(m.name.as_str()));
        assert!(!m.revision.is_empty());
        assert_eq!(m.manifest_sha256.len(), 64);
    }

    #[test]
    fn fresh_manifest_verifies() {
        let (dir, m) = fixture("ok");
        assert_eq!(m.gguf, "m.gguf");
        assert_eq!(m.gguf_sha256, sha256_hex(b"weights"));
        assert!(m.verify(&dir, true).is_empty());
    }

    #[test]
    fn round_trips_through_json() {
        let (dir, m) = fixture("json");
        std::fs::write(
            dir.join(MANIFEST_FILE),
            serde_json::to_string_pretty(&m).unwrap(),
        )
        .unwrap();
        assert_eq!(ModelManifest::load(&dir).unwrap(), m);
    }

    #[test]
    fn rejects_contract_skew() {
        let (dir, mut m) = fixture("skew");
        m.contract_sha256 = "0".repeat(64);
        let problems = m.verify(&dir, false);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("different contract"));
    }

    #[test]
    fn rejects_tampered_weights_only_when_hashing() {
        let (dir, m) = fixture("tamper");
        std::fs::write(dir.join("m.gguf"), b"other").unwrap();
        assert!(m.verify(&dir, false).is_empty());
        assert!(m.verify(&dir, true)[0].contains("sha256"));
    }

    #[test]
    fn rejects_missing_weights() {
        let (dir, m) = fixture("missing");
        std::fs::remove_file(dir.join("m.gguf")).unwrap();
        assert!(m.verify(&dir, false)[0].contains("missing"));
    }
}
