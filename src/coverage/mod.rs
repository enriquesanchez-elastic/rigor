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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_coverage_json() {
        let mut file = NamedTempFile::with_suffix(".json").unwrap();
        write!(
            file,
            r#"{{
                "/src/utils.ts": {{
                    "path": "/src/utils.ts",
                    "statementMap": {{
                        "0": {{ "start": {{ "line": 1, "column": 0 }}, "end": {{ "line": 1, "column": 20 }} }},
                        "1": {{ "start": {{ "line": 2, "column": 0 }}, "end": {{ "line": 2, "column": 15 }} }}
                    }},
                    "fnMap": {{
                        "0": {{ "name": "add", "loc": {{ "start": {{ "line": 1, "column": 0 }}, "end": {{ "line": 3, "column": 1 }} }} }}
                    }},
                    "branchMap": {{}},
                    "s": {{ "0": 5, "1": 0 }},
                    "f": {{ "0": 5 }},
                    "b": {{}}
                }}
            }}"#
        )
        .unwrap();
        file.flush().unwrap();

        let report = load_coverage(file.path()).unwrap();
        assert_eq!(report.files.len(), 1);
        assert!(report.summary.lines_total > 0);

        let fc = report
            .files
            .get(&std::path::PathBuf::from("/src/utils.ts"))
            .unwrap();
        assert_eq!(fc.summary.functions_covered, 1);
        assert_eq!(fc.summary.functions_total, 1);
        assert_eq!(fc.summary.statements_total, 2);
        assert_eq!(fc.summary.statements_covered, 1); // only stmt "0" has count > 0
    }

    #[test]
    fn test_load_coverage_non_json_extension() {
        // Non-json extension should still try Istanbul parse (default branch)
        let mut file = NamedTempFile::with_suffix(".coverage").unwrap();
        write!(
            file,
            r#"{{
                "/src/app.ts": {{
                    "path": "/src/app.ts",
                    "statementMap": {{}},
                    "fnMap": {{}},
                    "branchMap": {{}},
                    "s": {{}},
                    "f": {{}},
                    "b": {{}}
                }}
            }}"#
        )
        .unwrap();
        file.flush().unwrap();

        let report = load_coverage(file.path()).unwrap();
        assert_eq!(report.files.len(), 1);
    }

    #[test]
    fn test_load_coverage_file_not_found() {
        let result = load_coverage(std::path::Path::new("/nonexistent/coverage.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_coverage_invalid_json() {
        let mut file = NamedTempFile::with_suffix(".json").unwrap();
        write!(file, "not valid json").unwrap();
        file.flush().unwrap();

        let result = load_coverage(file.path());
        assert!(result.is_err());
    }
}
