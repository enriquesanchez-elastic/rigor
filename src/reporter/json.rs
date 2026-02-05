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
    pub fn report_with_summary(
        &self,
        results: &[AnalysisResult],
        stats: &AggregateStats,
    ) -> String {
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
    use crate::{
        Issue, Location, Rule, Score, ScoreBreakdown, Severity, TestFramework, TestStats, TestType,
    };
    use std::path::PathBuf;

    fn make_result(path: &str, score: u8) -> AnalysisResult {
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
            issues: vec![],
            stats: TestStats {
                total_tests: 3,
                ..TestStats::default()
            },
            framework: TestFramework::Jest,
            test_type: TestType::Unit,
            source_file: None,
        }
    }

    #[test]
    fn test_json_output() {
        let result = make_result("test.ts", 85);

        let reporter = JsonReporter::new();
        let json = reporter.report(&result);

        assert!(json.contains("\"filePath\""));
        assert!(json.contains("\"score\""));
        assert!(json.contains("85"));
    }

    #[test]
    fn test_json_single_result_has_expected_keys() {
        let mut result = make_result("auth.test.ts", 90);
        result.issues.push(Issue {
            rule: Rule::WeakAssertion,
            severity: Severity::Warning,
            message: "Weak assertion".to_string(),
            location: Location::new(5, 1),
            suggestion: Some("Use toBe()".to_string()),
        });

        let reporter = JsonReporter::new();
        let json = reporter.report(&result);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed.get("filePath").is_some());
        assert!(parsed.get("score").is_some());
        assert!(parsed.get("breakdown").is_some());
        assert!(parsed.get("issues").is_some());
        assert!(parsed.get("stats").is_some());
        assert!(parsed.get("framework").is_some());

        let issues = parsed["issues"].as_array().unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0]["rule"], "weak-assertion");
    }

    #[test]
    fn test_json_pretty_output() {
        let result = make_result("test.ts", 85);
        let reporter = JsonReporter::new().pretty();
        let json = reporter.report(&result);
        // Pretty JSON should have newlines and indentation
        assert!(json.contains('\n'), "pretty JSON should have newlines");
        assert!(json.contains("  "), "pretty JSON should have indentation");
    }

    #[test]
    fn test_json_report_many() {
        let r1 = make_result("a.test.ts", 90);
        let r2 = make_result("b.test.ts", 70);

        let reporter = JsonReporter::new();
        let json = reporter.report_many(&[r1, r2]);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["filePath"], "a.test.ts");
        assert_eq!(arr[1]["filePath"], "b.test.ts");
    }

    #[test]
    fn test_json_report_with_summary() {
        let r1 = make_result("a.test.ts", 90);
        let r2 = make_result("b.test.ts", 70);

        let stats = AggregateStats {
            files_analyzed: 2,
            average_score: Score::new(80),
            total_tests: 6,
            total_issues: 0,
        };

        let reporter = JsonReporter::new();
        let json = reporter.report_with_summary(&[r1, r2], &stats);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed.get("results").is_some());
        assert!(parsed.get("summary").is_some());

        let summary = &parsed["summary"];
        assert_eq!(summary["filesAnalyzed"], 2);
        assert_eq!(summary["averageScore"], 80);
        assert_eq!(summary["totalTests"], 6);
        assert_eq!(summary["totalIssues"], 0);

        let results = parsed["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_json_report_many_empty() {
        let reporter = JsonReporter::new();
        let json = reporter.report_many(&[]);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let arr = parsed.as_array().unwrap();
        assert!(arr.is_empty());
    }
}
