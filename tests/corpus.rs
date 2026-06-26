//! Real-world score-drift gate.
//!
//! Runs Rigor over a vendored corpus of real OSS TypeScript test files
//! (see `test-repos/corpus/SOURCES.md`) and compares per-file `{score, issues,
//! per-rule counts}` against a committed baseline.
//!
//! Unlike `tests/regression.rs` (synthetic `fake-project`), this protects the
//! score's *credibility on real code*: any rule/scoring edit that shifts
//! real-world output shows up here as a baseline diff that must be reviewed and
//! regenerated intentionally.
//!
//! Regenerate after an intentional change:
//!     UPDATE_CORPUS_BASELINE=1 cargo test --test corpus
//!
//! The gate also fails if the corpus is empty (vendoring regressed) or if any
//! file errors during analysis (a real input that panics/errors is a bug).

use rigor::analyzer::AnalysisEngine;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const CORPUS_ROOT: &str = "test-repos/corpus";
const BASELINE: &str = "test-repos/corpus/corpus-baseline.json";

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct FileBaseline {
    score: u8,
    issues: usize,
    /// rule-id -> count, for catching shifts that leave the score unchanged.
    rules: BTreeMap<String, usize>,
}

type Baseline = BTreeMap<String, FileBaseline>;

/// Recursively collect `*.test.ts` / `*.test.tsx` paths under `dir`, sorted.
fn collect_test_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(dir, &mut out);
    out.sort();
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.ends_with(".test.ts") || name.ends_with(".test.tsx") {
                out.push(path);
            }
        }
    }
}

/// Compute the current baseline for the whole corpus.
fn compute() -> Baseline {
    let root = PathBuf::from(CORPUS_ROOT);
    let files = collect_test_files(&root);
    assert!(
        !files.is_empty(),
        "corpus is empty under {CORPUS_ROOT} — vendoring regressed (see SOURCES.md)"
    );

    let engine = AnalysisEngine::new().with_project_root(root.clone());
    let mut baseline = Baseline::new();

    for path in files {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/"); // stable across platforms

        let result = engine.analyze(&path, None).unwrap_or_else(|e| {
            panic!("analyze({rel}) errored on real-world input: {e}");
        });

        let mut rules: BTreeMap<String, usize> = BTreeMap::new();
        for issue in &result.issues {
            *rules.entry(issue.rule.to_string()).or_insert(0) += 1;
        }

        baseline.insert(
            rel,
            FileBaseline {
                score: result.score.value,
                issues: result.issues.len(),
                rules,
            },
        );
    }
    baseline
}

#[test]
fn corpus_scores_match_baseline() {
    let current = compute();

    if std::env::var_os("UPDATE_CORPUS_BASELINE").is_some() {
        let json = serde_json::to_string_pretty(&current).expect("serialize baseline");
        std::fs::write(BASELINE, json + "\n").expect("write baseline");
        eprintln!(
            "corpus baseline regenerated: {BASELINE} ({} files)",
            current.len()
        );
        return;
    }

    let raw = std::fs::read_to_string(BASELINE).unwrap_or_else(|_| {
        panic!(
            "missing {BASELINE} — generate with: UPDATE_CORPUS_BASELINE=1 cargo test --test corpus"
        )
    });
    let expected: Baseline = serde_json::from_str(&raw).expect("parse baseline json");

    // Build a precise, reviewable diff rather than a raw struct dump.
    let mut diffs = Vec::new();
    for (file, cur) in &current {
        match expected.get(file) {
            None => diffs.push(format!("  + {file}: new file not in baseline")),
            Some(exp) if exp != cur => {
                if exp.score != cur.score {
                    diffs.push(format!("  ~ {file}: score {} -> {}", exp.score, cur.score));
                }
                if exp.issues != cur.issues {
                    diffs.push(format!(
                        "  ~ {file}: issues {} -> {}",
                        exp.issues, cur.issues
                    ));
                }
                if exp.rules != cur.rules {
                    diffs.push(format!(
                        "  ~ {file}: rules {:?} -> {:?}",
                        exp.rules, cur.rules
                    ));
                }
            }
            Some(_) => {}
        }
    }
    for file in expected.keys() {
        if !current.contains_key(file) {
            diffs.push(format!("  - {file}: present in baseline, missing now"));
        }
    }

    assert!(
        diffs.is_empty(),
        "real-world score drift detected ({} change(s)):\n{}\n\nIf intentional, regenerate:\n  UPDATE_CORPUS_BASELINE=1 cargo test --test corpus",
        diffs.len(),
        diffs.join("\n")
    );
}
