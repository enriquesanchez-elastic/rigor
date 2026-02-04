//! JSON reporter for machine-readable output

use crate::analyzer::engine::AggregateStats;
use crate::AnalysisResult;
use serde::Serialize;

/// Reporter for JSON output
pub struct JsonReporter {
    /// Whether to pretty-print JSON
    pretty: bool,
}

impl JsonReporter {
    /// Create a new JSON reporter
    pub fn new() -> Self {
        Self { pretty: false }
    }

    /// Enable pretty-printing
    pub fn pretty(mut self) -> Self {
        self.pretty = true;
        self
    }

    /// Report a single analysis result as JSON
    pub fn report(&self, result: &AnalysisResult) -> String {
        if self.pretty {
            serde_json::to_string_pretty(result).unwrap_or_else(|_| "{}".to_string())
        } else {
            serde_json::to_string(result).unwrap_or_else(|_| "{}".to_string())
        }
    }

    /// Report multiple results as JSON array
    pub fn report_many(&self, results: &[AnalysisResult]) -> String {
        if self.pretty {
            serde_json::to_string_pretty(results).unwrap_or_else(|_| "[]".to_string())
        } else {
            serde_json::to_string(results).unwrap_or_else(|_| "[]".to_string())
        }
    }

    /// Report with summary
    pub fn report_with_summary(&self, results: &[AnalysisResult], stats: &AggregateStats) -> String {
        let output = JsonOutput {
            results,
            summary: JsonSummary {
                files_analyzed: stats.files_analyzed,
                average_score: stats.average_score.value,
                average_grade: stats.average_score.grade.to_string(),
                total_tests: stats.total_tests,
                total_issues: stats.total_issues,
            },
        };

        if self.pretty {
            serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
        } else {
            serde_json::to_string(&output).unwrap_or_else(|_| "{}".to_string())
        }
    }
}

impl Default for JsonReporter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonOutput<'a> {
    results: &'a [AnalysisResult],
    summary: JsonSummary,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonSummary {
    files_analyzed: usize,
    average_score: u8,
    average_grade: String,
    total_tests: usize,
    total_issues: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Score, ScoreBreakdown, TestFramework, TestStats};
    use std::path::PathBuf;

    #[test]
    fn test_json_output() {
        let result = AnalysisResult {
            file_path: PathBuf::from("test.ts"),
            score: Score::new(85),
            breakdown: ScoreBreakdown {
                assertion_quality: 20,
                error_coverage: 18,
                boundary_conditions: 15,
                test_isolation: 17,
                input_variety: 15,
            },
            issues: vec![],
            stats: TestStats::default(),
            framework: TestFramework::Jest,
            source_file: None,
        };

        let reporter = JsonReporter::new();
        let json = reporter.report(&result);

        assert!(json.contains("\"filePath\""));
        assert!(json.contains("\"score\""));
        assert!(json.contains("85"));
    }
}
