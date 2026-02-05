//! Istanbul/c8 coverage format parser
//!
//! Parses coverage JSON files produced by Istanbul, NYC, c8, or Jest.
//! Format reference: https://github.com/gotwarlost/istanbul/blob/master/coverage.json.md

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Complete coverage report (may contain multiple files)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoverageReport {
    /// Coverage data per file
    pub files: HashMap<PathBuf, FileCoverage>,
    /// Overall summary statistics
    pub summary: CoverageSummary,
}

/// Coverage data for a single file
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileCoverage {
    /// Path to the source file
    pub path: PathBuf,
    /// Line coverage: line number -> hit count
    pub lines: HashMap<u32, u32>,
    /// Branch coverage: branch id -> [taken, not taken]
    pub branches: HashMap<String, BranchCoverage>,
    /// Function coverage: function name -> hit count
    pub functions: HashMap<String, u32>,
    /// Statement coverage: statement id -> hit count
    pub statements: HashMap<String, u32>,
    /// Summary for this file
    pub summary: CoverageSummary,
}

/// Branch coverage data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BranchCoverage {
    /// Line where the branch is
    pub line: u32,
    /// Coverage for each branch path (e.g., [true_count, false_count])
    pub coverage: Vec<u32>,
}

/// Coverage summary statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoverageSummary {
    /// Line coverage percentage (0-100)
    pub lines_pct: f32,
    /// Lines covered
    pub lines_covered: u32,
    /// Total lines
    pub lines_total: u32,
    /// Branch coverage percentage (0-100)
    pub branches_pct: f32,
    /// Branches covered
    pub branches_covered: u32,
    /// Total branches
    pub branches_total: u32,
    /// Function coverage percentage (0-100)
    pub functions_pct: f32,
    /// Functions covered
    pub functions_covered: u32,
    /// Total functions
    pub functions_total: u32,
    /// Statement coverage percentage (0-100)
    pub statements_pct: f32,
    /// Statements covered
    pub statements_covered: u32,
    /// Total statements
    pub statements_total: u32,
}

/// Raw coverage data as stored in Istanbul JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageData {
    /// Path to source file
    pub path: String,
    /// Statement map: statement id -> location
    #[serde(default, rename = "statementMap")]
    pub statement_map: HashMap<String, LocationMap>,
    /// Function map: function id -> function info
    #[serde(default, rename = "fnMap")]
    pub fn_map: HashMap<String, FunctionInfo>,
    /// Branch map: branch id -> branch info
    #[serde(default, rename = "branchMap")]
    pub branch_map: HashMap<String, BranchInfo>,
    /// Statement hit counts: statement id -> count
    #[serde(default, rename = "s")]
    pub s: HashMap<String, u32>,
    /// Function hit counts: function id -> count
    #[serde(default, rename = "f")]
    pub f: HashMap<String, u32>,
    /// Branch hit counts: branch id -> [path counts]
    #[serde(default, rename = "b")]
    pub b: HashMap<String, Vec<u32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationMap {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
    #[serde(default)]
    pub decl: Option<LocationMap>,
    pub loc: LocationMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    #[serde(rename = "type")]
    pub branch_type: String,
    pub loc: LocationMap,
    pub locations: Vec<LocationMap>,
}

/// Parse Istanbul/c8 JSON coverage format
pub fn parse_istanbul_json(content: &str) -> anyhow::Result<CoverageReport> {
    // Istanbul JSON can be either:
    // 1. A map of file paths to coverage data: { "/path/to/file.ts": { ... }, ... }
    // 2. A c8/nyc summary format with "total" key
    
    let raw: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| anyhow::anyhow!("Failed to parse coverage JSON: {}", e))?;
    
    let mut report = CoverageReport::default();
    
    // Handle different formats
    if let Some(obj) = raw.as_object() {
        for (key, value) in obj {
            // Skip metadata keys
            if key == "total" || key == "coverageMap" {
                continue;
            }
            
            // Try to parse as coverage data
            if let Ok(coverage_data) = serde_json::from_value::<CoverageData>(value.clone()) {
                let file_coverage = process_coverage_data(&coverage_data);
                report.files.insert(PathBuf::from(&coverage_data.path), file_coverage);
            }
        }
    }
    
    // Calculate overall summary
    report.summary = calculate_summary(&report.files);
    
    Ok(report)
}

/// Process raw coverage data into our FileCoverage format
fn process_coverage_data(data: &CoverageData) -> FileCoverage {
    let mut coverage = FileCoverage {
        path: PathBuf::from(&data.path),
        ..Default::default()
    };
    
    // Process line coverage from statements
    for (stmt_id, &count) in &data.s {
        if let Some(loc) = data.statement_map.get(stmt_id) {
            let line = loc.start.line;
            let entry = coverage.lines.entry(line).or_insert(0);
            *entry = (*entry).max(count);
        }
    }
    
    // Process function coverage
    for (fn_id, &count) in &data.f {
        if let Some(fn_info) = data.fn_map.get(fn_id) {
            coverage.functions.insert(fn_info.name.clone(), count);
        }
    }
    
    // Process branch coverage
    for (branch_id, counts) in &data.b {
        if let Some(branch_info) = data.branch_map.get(branch_id) {
            coverage.branches.insert(branch_id.clone(), BranchCoverage {
                line: branch_info.loc.start.line,
                coverage: counts.clone(),
            });
        }
    }
    
    // Copy statement coverage
    coverage.statements = data.s.clone();
    
    // Calculate summary for this file
    coverage.summary = calculate_file_summary(&coverage, data);
    
    coverage
}

/// Calculate summary statistics for a single file
fn calculate_file_summary(coverage: &FileCoverage, data: &CoverageData) -> CoverageSummary {
    let lines_total = coverage.lines.len() as u32;
    let lines_covered = coverage.lines.values().filter(|&&c| c > 0).count() as u32;
    
    let stmts_total = data.s.len() as u32;
    let stmts_covered = data.s.values().filter(|&&c| c > 0).count() as u32;
    
    let fns_total = data.f.len() as u32;
    let fns_covered = data.f.values().filter(|&&c| c > 0).count() as u32;
    
    let mut branches_total = 0u32;
    let mut branches_covered = 0u32;
    for counts in data.b.values() {
        branches_total += counts.len() as u32;
        branches_covered += counts.iter().filter(|&&c| c > 0).count() as u32;
    }
    
    CoverageSummary {
        lines_pct: if lines_total > 0 { (lines_covered as f32 / lines_total as f32) * 100.0 } else { 100.0 },
        lines_covered,
        lines_total,
        branches_pct: if branches_total > 0 { (branches_covered as f32 / branches_total as f32) * 100.0 } else { 100.0 },
        branches_covered,
        branches_total,
        functions_pct: if fns_total > 0 { (fns_covered as f32 / fns_total as f32) * 100.0 } else { 100.0 },
        functions_covered: fns_covered,
        functions_total: fns_total,
        statements_pct: if stmts_total > 0 { (stmts_covered as f32 / stmts_total as f32) * 100.0 } else { 100.0 },
        statements_covered: stmts_covered,
        statements_total: stmts_total,
    }
}

/// Calculate overall summary from all file coverages
fn calculate_summary(files: &HashMap<PathBuf, FileCoverage>) -> CoverageSummary {
    let mut total = CoverageSummary::default();
    
    for fc in files.values() {
        total.lines_total += fc.summary.lines_total;
        total.lines_covered += fc.summary.lines_covered;
        total.branches_total += fc.summary.branches_total;
        total.branches_covered += fc.summary.branches_covered;
        total.functions_total += fc.summary.functions_total;
        total.functions_covered += fc.summary.functions_covered;
        total.statements_total += fc.summary.statements_total;
        total.statements_covered += fc.summary.statements_covered;
    }
    
    total.lines_pct = if total.lines_total > 0 { (total.lines_covered as f32 / total.lines_total as f32) * 100.0 } else { 100.0 };
    total.branches_pct = if total.branches_total > 0 { (total.branches_covered as f32 / total.branches_total as f32) * 100.0 } else { 100.0 };
    total.functions_pct = if total.functions_total > 0 { (total.functions_covered as f32 / total.functions_total as f32) * 100.0 } else { 100.0 };
    total.statements_pct = if total.statements_total > 0 { (total.statements_covered as f32 / total.statements_total as f32) * 100.0 } else { 100.0 };
    
    total
}

impl FileCoverage {
    /// Check if a specific line is covered
    pub fn is_line_covered(&self, line: u32) -> bool {
        self.lines.get(&line).map_or(false, |&c| c > 0)
    }
    
    /// Get uncovered lines
    pub fn uncovered_lines(&self) -> Vec<u32> {
        self.lines
            .iter()
            .filter(|(_, &c)| c == 0)
            .map(|(&line, _)| line)
            .collect()
    }
    
    /// Get uncovered functions
    pub fn uncovered_functions(&self) -> Vec<&str> {
        self.functions
            .iter()
            .filter(|(_, &c)| c == 0)
            .map(|(name, _)| name.as_str())
            .collect()
    }
}

impl CoverageReport {
    /// Get coverage for a specific file path
    pub fn get_file_coverage(&self, path: &Path) -> Option<&FileCoverage> {
        // Try exact match first
        if let Some(fc) = self.files.get(path) {
            return Some(fc);
        }
        
        // Try matching by file name
        let file_name = path.file_name()?;
        for (p, fc) in &self.files {
            if p.file_name() == Some(file_name) {
                return Some(fc);
            }
        }
        
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_istanbul_json() {
        let json = r#"{
            "/src/utils.ts": {
                "path": "/src/utils.ts",
                "statementMap": {
                    "0": { "start": { "line": 1, "column": 0 }, "end": { "line": 1, "column": 20 } },
                    "1": { "start": { "line": 2, "column": 0 }, "end": { "line": 2, "column": 15 } }
                },
                "fnMap": {
                    "0": { "name": "add", "loc": { "start": { "line": 1, "column": 0 }, "end": { "line": 3, "column": 1 } } }
                },
                "branchMap": {},
                "s": { "0": 5, "1": 3 },
                "f": { "0": 5 },
                "b": {}
            }
        }"#;
        
        let report = parse_istanbul_json(json).unwrap();
        assert_eq!(report.files.len(), 1);
        
        let file = report.files.get(&PathBuf::from("/src/utils.ts")).unwrap();
        assert_eq!(file.summary.functions_covered, 1);
        assert_eq!(file.summary.functions_total, 1);
    }
    
    #[test]
    fn test_uncovered_detection() {
        let mut coverage = FileCoverage::default();
        coverage.lines.insert(1, 5);
        coverage.lines.insert(2, 0);
        coverage.lines.insert(3, 3);
        coverage.lines.insert(4, 0);
        
        let uncovered = coverage.uncovered_lines();
        assert!(uncovered.contains(&2));
        assert!(uncovered.contains(&4));
        assert!(!uncovered.contains(&1));
        assert!(!uncovered.contains(&3));
    }
}
