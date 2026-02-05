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
