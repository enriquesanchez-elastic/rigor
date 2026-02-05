//! Trend tracking - persist scores to .rigor-history.json

use crate::AnalysisResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const HISTORY_FILENAME: &str = ".rigor-history.json";
const MAX_RUNS: usize = 50;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct HistoryFile {
    pub runs: Vec<HistoryRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRun {
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    pub files: HashMap<String, FileScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileScore {
    pub score: u8,
    pub issues: usize,
}

/// Find project root (directory containing .rigor-history.json or first dir with package.json / .git)
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_file() {
        start.parent()?
    } else {
        start
    };

    loop {
        if dir.join(HISTORY_FILENAME).exists() {
            return Some(dir.to_path_buf());
        }
        if dir.join("package.json").exists() || dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

/// Load history from project root (or create empty)
pub fn load_history(project_root: &Path) -> HistoryFile {
    let path = project_root.join(HISTORY_FILENAME);
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(history) = serde_json::from_str::<HistoryFile>(&content) {
            return history;
        }
    }
    HistoryFile::default()
}

/// Save history to project root
pub fn save_history(project_root: &Path, history: &HistoryFile) -> std::io::Result<()> {
    let path = project_root.join(HISTORY_FILENAME);
    let content = serde_json::to_string_pretty(history).unwrap_or_else(|_| "{}".to_string());
    fs::write(path, content)
}

/// Get the previous score for a file from the latest run
pub fn previous_score(history: &HistoryFile, file_path: &Path) -> Option<u8> {
    let run = history.runs.last()?;
    let key = file_path.to_string_lossy().to_string();
    run.files.get(&key).map(|f| f.score)
}

/// Build a new run from analysis results and append to history
pub fn append_run(history: &mut HistoryFile, results: &[AnalysisResult], commit: Option<String>) {
    let timestamp = chrono::Utc::now().to_rfc3339();
    let mut files = HashMap::new();
    for r in results {
        let key = r.file_path.to_string_lossy().to_string();
        files.insert(
            key,
            FileScore {
                score: r.score.value,
                issues: r.issues.len(),
            },
        );
    }
    history.runs.push(HistoryRun {
        timestamp,
        commit,
        files,
    });
    if history.runs.len() > MAX_RUNS {
        history.runs.drain(0..history.runs.len() - MAX_RUNS);
    }
}

/// Format delta for console: "[was 82, down 4]" or "[was 82, up 2]" or ""
pub fn format_delta(previous: Option<u8>, current: u8) -> String {
    let Some(prev) = previous else {
        return String::new();
    };
    if prev == current {
        return format!(" [unchanged at {}]", current);
    }
    let diff = current as i16 - prev as i16;
    if diff > 0 {
        format!(" [was {}, up {}]", prev, diff)
    } else {
        format!(" [was {}, down {}]", prev, -diff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AnalysisResult, Score, ScoreBreakdown, TestFramework, TestStats, TestType};
    use std::path::PathBuf;

    fn make_result(path: &str, score: u8, issue_count: usize) -> AnalysisResult {
        AnalysisResult {
            file_path: PathBuf::from(path),
            score: Score::new(score),
            breakdown: ScoreBreakdown {
                assertion_quality: 20,
                error_coverage: 18,
                boundary_conditions: 15,
                test_isolation: 17,
                input_variety: 15,
            },
            issues: (0..issue_count)
                .map(|_| crate::Issue {
                    rule: crate::Rule::WeakAssertion,
                    severity: crate::Severity::Warning,
                    message: "test".to_string(),
                    location: crate::Location::new(1, 1),
                    suggestion: None,
                })
                .collect(),
            stats: TestStats::default(),
            framework: TestFramework::Jest,
            test_type: TestType::Unit,
            source_file: None,
        }
    }

    // --- format_delta ---

    #[test]
    fn format_delta_no_previous_returns_empty() {
        assert_eq!(format_delta(None, 85), "");
    }

    #[test]
    fn format_delta_score_increased() {
        assert_eq!(format_delta(Some(80), 83), " [was 80, up 3]");
    }

    #[test]
    fn format_delta_score_decreased() {
        assert_eq!(format_delta(Some(90), 86), " [was 90, down 4]");
    }

    #[test]
    fn format_delta_score_unchanged() {
        assert_eq!(format_delta(Some(75), 75), " [unchanged at 75]");
    }

    #[test]
    fn format_delta_extreme_values() {
        assert_eq!(format_delta(Some(0), 100), " [was 0, up 100]");
        assert_eq!(format_delta(Some(100), 0), " [was 100, down 100]");
    }

    // --- previous_score ---

    #[test]
    fn previous_score_empty_history_returns_none() {
        let history = HistoryFile::default();
        assert_eq!(previous_score(&history, Path::new("auth.test.ts")), None);
    }

    #[test]
    fn previous_score_returns_latest_run_score() {
        let mut files = HashMap::new();
        files.insert(
            "auth.test.ts".to_string(),
            FileScore {
                score: 88,
                issues: 2,
            },
        );

        let history = HistoryFile {
            runs: vec![HistoryRun {
                timestamp: "2025-01-01T00:00:00Z".to_string(),
                commit: None,
                files,
            }],
        };

        assert_eq!(
            previous_score(&history, Path::new("auth.test.ts")),
            Some(88)
        );
        assert_eq!(previous_score(&history, Path::new("other.test.ts")), None);
    }

    #[test]
    fn previous_score_uses_last_run_not_first() {
        let mut files1 = HashMap::new();
        files1.insert(
            "auth.test.ts".to_string(),
            FileScore {
                score: 70,
                issues: 5,
            },
        );
        let mut files2 = HashMap::new();
        files2.insert(
            "auth.test.ts".to_string(),
            FileScore {
                score: 90,
                issues: 1,
            },
        );

        let history = HistoryFile {
            runs: vec![
                HistoryRun {
                    timestamp: "2025-01-01T00:00:00Z".to_string(),
                    commit: None,
                    files: files1,
                },
                HistoryRun {
                    timestamp: "2025-01-02T00:00:00Z".to_string(),
                    commit: None,
                    files: files2,
                },
            ],
        };

        assert_eq!(
            previous_score(&history, Path::new("auth.test.ts")),
            Some(90)
        );
    }

    // --- append_run ---

    #[test]
    fn append_run_adds_entry_with_correct_scores() {
        let mut history = HistoryFile::default();
        let results = vec![
            make_result("a.test.ts", 85, 2),
            make_result("b.test.ts", 60, 5),
        ];

        append_run(&mut history, &results, Some("abc123".to_string()));

        assert_eq!(history.runs.len(), 1);
        let run = &history.runs[0];
        assert_eq!(run.commit, Some("abc123".to_string()));
        assert_eq!(run.files.len(), 2);
        assert_eq!(run.files["a.test.ts"].score, 85);
        assert_eq!(run.files["a.test.ts"].issues, 2);
        assert_eq!(run.files["b.test.ts"].score, 60);
        assert_eq!(run.files["b.test.ts"].issues, 5);
    }

    #[test]
    fn append_run_truncates_to_max_runs() {
        let mut history = HistoryFile::default();
        let results = vec![make_result("a.test.ts", 80, 1)];

        // Add 55 runs (more than MAX_RUNS = 50)
        for _ in 0..55 {
            append_run(&mut history, &results, None);
        }

        assert_eq!(history.runs.len(), MAX_RUNS);
    }

    // --- load_history / save_history roundtrip ---

    #[test]
    fn save_and_load_history_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut history = HistoryFile::default();
        let results = vec![make_result("x.test.ts", 92, 0)];
        append_run(&mut history, &results, Some("deadbeef".to_string()));

        save_history(dir.path(), &history).unwrap();
        let loaded = load_history(dir.path());

        assert_eq!(loaded.runs.len(), 1);
        assert_eq!(loaded.runs[0].commit, Some("deadbeef".to_string()));
        assert_eq!(loaded.runs[0].files["x.test.ts"].score, 92);
        assert_eq!(loaded.runs[0].files["x.test.ts"].issues, 0);
    }

    #[test]
    fn load_history_returns_empty_for_nonexistent_file() {
        let dir = tempfile::tempdir().unwrap();
        let history = load_history(dir.path());
        assert!(history.runs.is_empty());
    }

    #[test]
    fn load_history_returns_empty_for_corrupt_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(HISTORY_FILENAME), "not valid json {{{").unwrap();
        let history = load_history(dir.path());
        assert!(history.runs.is_empty());
    }

    // --- find_project_root ---

    #[test]
    fn find_project_root_with_history_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(HISTORY_FILENAME), "{}").unwrap();
        let root = find_project_root(dir.path());
        assert_eq!(root.unwrap(), dir.path());
    }

    #[test]
    fn find_project_root_with_package_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        let sub = dir.path().join("src");
        std::fs::create_dir(&sub).unwrap();
        let root = find_project_root(&sub);
        assert_eq!(root.unwrap(), dir.path());
    }
}
