//! Benchmark runner (`SPEC-005 §25`, §26) — **Phase 0: empty-suite runner**.
//!
//! Discovers the benchmark corpus, runs the requested suite, and writes the mandated report
//! artifacts (`summary.json`, `cases.jsonl`, `failures.jsonl`, `metrics.json`,
//! `environment.json`, `artifacts/`). In Phase 0 no case *execution* exists yet, so a run over a
//! corpus with no cases produces a well-formed, zeroed report — the Phase 0 acceptance gate.
//!
//! Honesty rule (`SPEC-005 §18`): if cases are discovered but cannot be executed, they are
//! reported as **errored** and owned by `BENCHMARK_HARNESS` — never counted as passing.

use std::fs;
use std::path::{Path, PathBuf};

use amc_core::{AmcError, Component, Result};
use serde::Serialize;
use serde_json::json;

/// Subdirectories of the corpus that contain runnable cases (`SPEC-005 §24`).
const CASE_DIRS: &[&str] = &["intent", "integration", "e2e"];

/// Options controlling a benchmark run (subset of `SPEC-005 §25`).
#[derive(Debug, Clone)]
pub struct BenchOptions {
    /// Where report directories are written (typically `<corpus>/reports`).
    pub report_dir: PathBuf,
    /// Restrict to a single category (matched against a case's `category`).
    pub category: Option<String>,
    /// Restrict to a single case id.
    pub case: Option<String>,
    /// Stop at the first failing/errored case.
    pub fail_fast: bool,
}

impl BenchOptions {
    /// Default options writing under `<corpus>/reports`.
    pub fn for_corpus(corpus_root: &Path) -> Self {
        Self {
            report_dir: corpus_root.join("reports"),
            category: None,
            case: None,
            fail_fast: false,
        }
    }
}

/// A single case's disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    Passed,
    Failed,
    Errored,
    Skipped,
}

/// Run-level summary (`SPEC-005 §26` summary.json + §19 scoring).
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub run_id: String,
    pub suite: String,
    pub total_cases: usize,
    pub passed: usize,
    pub failed: usize,
    pub errored: usize,
    pub skipped: usize,
    /// Verified Request Completion Rate (`SPEC-005 §19`); `None` when there are no eligible cases.
    pub verified_request_completion_rate: Option<f64>,
    pub report_dir: String,
}

/// The environment record (`SPEC-005 §26`). Fields we cannot cheaply determine in Phase 0 are
/// left as `null` and filled in later phases.
#[derive(Debug, Clone, Serialize)]
pub struct Environment {
    pub source_commit: Option<String>,
    pub model_version: Option<String>,
    pub dataset_version: Option<String>,
    pub mcl_schema_version: String,
    pub generator_version: Option<String>,
    pub minecraft_adapter_version: Option<String>,
    pub os: String,
    pub arch: String,
    pub run_started_at: String,
}

impl Environment {
    fn capture() -> Self {
        Environment {
            source_commit: std::env::var("AMC_SOURCE_COMMIT")
                .ok()
                .or_else(|| std::env::var("GITHUB_SHA").ok()),
            model_version: None,
            dataset_version: None,
            mcl_schema_version: amc_schema::SCHEMA_VERSION.to_string(),
            generator_version: None,
            minecraft_adapter_version: None,
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            // Wall-clock timestamp is report metadata, not a determinism input (SPEC-005 §22).
            run_started_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// The result of a run: the summary plus the directory the report was written to.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub summary: Summary,
    pub report_path: PathBuf,
}

/// Run a benchmark suite rooted at `corpus_root`, writing a report and returning the summary.
pub fn run_suite(corpus_root: &Path, opts: &BenchOptions) -> Result<RunOutcome> {
    let env = Environment::capture();
    let run_id = make_run_id(&env.run_started_at);
    let report_path = opts.report_dir.join(&run_id);
    fs::create_dir_all(report_path.join("artifacts"))?;

    let cases = discover_cases(corpus_root)?;

    let mut summary = Summary {
        run_id: run_id.clone(),
        suite: corpus_root.display().to_string(),
        total_cases: 0,
        passed: 0,
        failed: 0,
        errored: 0,
        skipped: 0,
        verified_request_completion_rate: None,
        report_dir: report_path.display().to_string(),
    };

    let mut cases_lines = String::new();
    let mut failures_lines = String::new();
    let mut eligible = 0usize;

    for case in &cases {
        if let Some(cat) = &opts.category {
            if case.category.as_deref() != Some(cat.as_str()) {
                continue;
            }
        }
        if let Some(id) = &opts.case {
            if case.case_id.as_deref() != Some(id.as_str()) {
                continue;
            }
        }

        summary.total_cases += 1;
        eligible += 1;

        // Phase 0: no case execution exists yet. Report discovered cases as errored and owned by
        // the benchmark harness so they can never be mistaken for passes (SPEC-005 §18).
        let disposition = Disposition::Errored;
        summary.errored += 1;

        let record = json!({
            "case_id": case.case_id,
            "category": case.category,
            "path": case.path.display().to_string(),
            "disposition": disposition,
            "failure_owner": Component::Benchmark.tag(),
            "message": "case execution not implemented until Phase 5",
        });
        cases_lines.push_str(&record.to_string());
        cases_lines.push('\n');
        failures_lines.push_str(&record.to_string());
        failures_lines.push('\n');

        if opts.fail_fast {
            break;
        }
    }

    summary.verified_request_completion_rate = if eligible == 0 {
        None
    } else {
        Some(summary.passed as f64 / eligible as f64)
    };

    let metrics = json!({
        "verified_request_completion_rate": summary.verified_request_completion_rate,
        "pass_rate": if summary.total_cases == 0 { serde_json::Value::Null }
            else { json!(summary.passed as f64 / summary.total_cases as f64) },
        "counts": {
            "total": summary.total_cases,
            "passed": summary.passed,
            "failed": summary.failed,
            "errored": summary.errored,
            "skipped": summary.skipped,
        }
    });

    write_json(&report_path.join("summary.json"), &summary)?;
    write_json(&report_path.join("environment.json"), &env)?;
    write_json(&report_path.join("metrics.json"), &metrics)?;
    fs::write(report_path.join("cases.jsonl"), cases_lines)?;
    fs::write(report_path.join("failures.jsonl"), failures_lines)?;

    Ok(RunOutcome {
        summary,
        report_path,
    })
}

/// A discovered case: minimal identifying fields read from its JSON.
#[derive(Debug, Clone)]
pub struct DiscoveredCase {
    pub path: PathBuf,
    pub case_id: Option<String>,
    pub category: Option<String>,
}

/// Discover case files under the corpus `intent/`, `integration/`, and `e2e/` subtrees.
pub fn discover_cases(corpus_root: &Path) -> Result<Vec<DiscoveredCase>> {
    let mut out = Vec::new();
    for dir in CASE_DIRS {
        let root = corpus_root.join(dir);
        if root.is_dir() {
            collect_json(&root, &mut out)?;
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn collect_json(dir: &Path, out: &mut Vec<DiscoveredCase>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_json(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let (case_id, category) = read_case_meta(&path);
            out.push(DiscoveredCase {
                path,
                case_id,
                category,
            });
        }
    }
    Ok(())
}

fn read_case_meta(path: &Path) -> (Option<String>, Option<String>) {
    let Ok(text) = fs::read_to_string(path) else {
        return (None, None);
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return (None, None);
    };
    let case_id = value
        .get("case_id")
        .and_then(|v| v.as_str())
        .map(String::from);
    let category = value
        .get("category")
        .and_then(|v| v.as_str())
        .map(String::from);
    (case_id, category)
}

fn make_run_id(rfc3339: &str) -> String {
    // Compact the timestamp into a filesystem-friendly run id, e.g. 20260717T120000Z.
    let compact: String = rfc3339
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>();
    format!("run-{compact}")
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let text = serde_json::to_string_pretty(value)?;
    fs::write(path, text).map_err(AmcError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_empty_corpus() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for sub in ["intent", "integration", "e2e", "reports"] {
            fs::create_dir_all(dir.path().join(sub)).unwrap();
        }
        dir
    }

    #[test]
    fn empty_suite_produces_zeroed_report() {
        let corpus = make_empty_corpus();
        let opts = BenchOptions::for_corpus(corpus.path());
        let outcome = run_suite(corpus.path(), &opts).unwrap();

        assert_eq!(outcome.summary.total_cases, 0);
        assert_eq!(outcome.summary.verified_request_completion_rate, None);
        for f in [
            "summary.json",
            "environment.json",
            "metrics.json",
            "cases.jsonl",
        ] {
            assert!(
                outcome.report_path.join(f).exists(),
                "missing report file {f}"
            );
        }
        assert!(outcome.report_path.join("artifacts").is_dir());
    }

    #[test]
    fn discovered_case_is_errored_not_passed() {
        let corpus = make_empty_corpus();
        fs::write(
            corpus.path().join("e2e").join("E2E-DEMO-001.json"),
            r#"{"case_id":"E2E-DEMO-001","category":"create"}"#,
        )
        .unwrap();
        let opts = BenchOptions::for_corpus(corpus.path());
        let outcome = run_suite(corpus.path(), &opts).unwrap();
        assert_eq!(outcome.summary.total_cases, 1);
        assert_eq!(outcome.summary.passed, 0);
        assert_eq!(outcome.summary.errored, 1);
    }
}
