//! Coverage tool integration - parse and use coverage data from Istanbul/c8

mod istanbul;

pub use istanbul::{parse_istanbul_json, CoverageData, CoverageReport, FileCoverage};

use std::path::Path;

/// Load coverage data from a file (auto-detects format)
pub fn load_coverage(path: &Path) -> anyhow::Result<CoverageReport> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read coverage file: {}", e))?;

    // Try Istanbul/c8 JSON format
    if path.extension().is_some_and(|ext| ext == "json") {
        return parse_istanbul_json(&content);
    }

    // Default to Istanbul JSON
    parse_istanbul_json(&content)
}
