//! Coverage tool integration - parse and use coverage data from Istanbul/c8

mod istanbul;

pub use istanbul::{CoverageData, CoverageReport, FileCoverage, parse_istanbul_json};

use std::path::Path;

/// Load coverage data from a file (auto-detects format)
pub fn load_coverage(path: &Path) -> anyhow::Result<CoverageReport> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read coverage file: {}", e))?;
    
    // Try Istanbul/c8 JSON format
    if path.extension().map_or(false, |ext| ext == "json") {
        return parse_istanbul_json(&content);
    }
    
    // Default to Istanbul JSON
    parse_istanbul_json(&content)
}
