//! Generation manifest & output hashing (`SPEC-003 §9`, `SPEC-005 §22`) — Phase 2 work item
//! `0006`.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};

/// One generated file and the hash of its bytes.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FileEntry {
    pub path: String,
    pub sha256: String,
}

/// The manifest describing a generation run and proving reproducibility (`SPEC-003 §9`).
#[derive(Debug, Clone, Serialize)]
pub struct GenerationManifest {
    pub generator_version: String,
    pub minecraft_target: String,
    pub files: Vec<FileEntry>,
    /// Canonical hash over the sorted `(path, sha256)` list — stable for identical inputs.
    pub output_hash: String,
}

/// Hash of a single file's bytes as `sha256:...`.
pub fn file_hash(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

impl GenerationManifest {
    /// Build a manifest from the generated file map (path → bytes).
    pub fn build(
        generator_version: &str,
        minecraft_target: &str,
        files: &BTreeMap<String, Vec<u8>>,
    ) -> Self {
        let entries: Vec<FileEntry> = files
            .iter()
            .map(|(path, bytes)| FileEntry {
                path: path.clone(),
                sha256: file_hash(bytes),
            })
            .collect();

        // The output hash covers only paths + content hashes — no timestamps or absolute paths.
        let hashable = json!(entries
            .iter()
            .map(|e| json!([e.path, e.sha256]))
            .collect::<Vec<_>>());

        GenerationManifest {
            generator_version: generator_version.to_string(),
            minecraft_target: minecraft_target.to_string(),
            files: entries,
            output_hash: amc_core::canonical_hash(&hashable),
        }
    }
}
