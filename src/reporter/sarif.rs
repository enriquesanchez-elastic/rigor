//! SARIF 2.1 reporter for GitHub Code Scanning / VS Code SARIF viewer

use crate::analyzer::engine::AggregateStats;
use crate::{AnalysisResult, Severity};
use serde::Serialize;
use std::path::Path;

/// SARIF 2.1.0 minimal structure for one run
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifLog {
    #[serde(rename = "$schema")]
    schema: String,
    version: String,
    runs: Vec<SarifRun>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifDriver {
    name: String,
    version: String,
    information_uri: String,
    rules: Vec<SarifRule>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRule {
    id: String,
    short_description: SarifMessage,
    full_description: Option<SarifMessage>,
    default_configuration: SarifDefaultConfig,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifMessage {
    text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifDefaultConfig {
    level: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifResult {
    rule_id: String,
    level: String,
    message: SarifMessage,
    locations: Vec<SarifLocation>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifLocation {
    physical_location: SarifPhysicalLocation,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifPhysicalLocation {
    artifact_location: SarifArtifactLocation,
    region: SarifRegion,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifArtifactLocation {
    uri: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRegion {
    start_line: usize,
    start_column: Option<usize>,
    end_line: Option<usize>,
    end_column: Option<usize>,
}

fn severity_to_level(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

/// Convert a path to a URI (file://)
fn path_to_uri(p: &Path) -> String {
    let path = p.to_string_lossy();
    let path = path.replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{}", path)
    } else {
        format!("file:///{}", path)
    }
}

/// SARIF reporter for GitHub Code Scanning integration
pub struct SarifReporter;

impl SarifReporter {
    pub fn new() -> Self {
        Self
    }

    /// Produce SARIF 2.1 JSON for one or more analysis results
    pub fn report(&self, results: &[AnalysisResult], _stats: Option<&AggregateStats>) -> String {
        let mut rule_ids = std::collections::HashSet::new();
        for r in results {
            for i in &r.issues {
                rule_ids.insert(i.rule.to_string());
            }
        }

        let rules: Vec<SarifRule> = rule_ids
            .iter()
            .map(|id| SarifRule {
                id: id.clone(),
                short_description: SarifMessage {
                    text: id.replace('-', " ").to_string(),
                },
                full_description: None,
                default_configuration: SarifDefaultConfig {
                    level: "warning".to_string(),
                },
            })
            .collect();

        let mut sarif_results = Vec::new();
        for result in results {
            let file_uri = path_to_uri(&result.file_path);
            for issue in &result.issues {
                sarif_results.push(SarifResult {
                    rule_id: issue.rule.to_string(),
                    level: severity_to_level(issue.severity).to_string(),
                    message: SarifMessage {
                        text: issue.message.clone(),
                    },
                    locations: vec![SarifLocation {
                        physical_location: SarifPhysicalLocation {
                            artifact_location: SarifArtifactLocation {
                                uri: file_uri.clone(),
                            },
                            region: SarifRegion {
                                start_line: issue.location.line,
                                start_column: Some(issue.location.column),
                                end_line: issue.location.end_line,
                                end_column: issue.location.end_column,
                            },
                        },
                    }],
                });
            }
        }

        let run = SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "Rigor".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    information_uri: "https://github.com/rigor-dev/rigor".to_string(),
                    rules,
                },
            },
            results: sarif_results,
        };

        let log = SarifLog {
            schema: "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json".to_string(),
            version: "2.1.0".to_string(),
            runs: vec![run],
        };

        serde_json::to_string_pretty(&log).unwrap_or_else(|_| "{}".to_string())
    }
}

impl Default for SarifReporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AnalysisResult, Issue, Location, Rule, Score, ScoreBreakdown, Severity, TestFramework,
        TestStats, TestType,
    };
    use std::path::PathBuf;

    fn make_result_with_issues(issues: Vec<Issue>) -> AnalysisResult {
        AnalysisResult {
            file_path: PathBuf::from("/src/tests/auth.test.ts"),
            score: Score::new(75),
            breakdown: ScoreBreakdown {
                assertion_quality: 20,
                error_coverage: 15,
                boundary_conditions: 10,
                test_isolation: 20,
                input_variety: 15,
            },
            issues,
            stats: TestStats {
                total_tests: 5,
                ..TestStats::default()
            },
            framework: TestFramework::Jest,
            test_type: TestType::Unit,
            source_file: None,
        }
    }

    #[test]
    fn sarif_output_is_valid_json() {
        let reporter = SarifReporter::new();
        let result = make_result_with_issues(vec![]);
        let output = reporter.report(&[result], None);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.is_object());
    }

    #[test]
    fn sarif_has_correct_schema_and_version() {
        let reporter = SarifReporter::new();
        let result = make_result_with_issues(vec![]);
        let output = reporter.report(&[result], None);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["version"], "2.1.0");
        assert!(
            parsed["$schema"]
                .as_str()
                .unwrap()
                .contains("sarif-schema-2.1.0"),
            "expected SARIF 2.1.0 schema URL"
        );
    }

    #[test]
    fn sarif_has_single_run_with_tool_driver() {
        let reporter = SarifReporter::new();
        let result = make_result_with_issues(vec![]);
        let output = reporter.report(&[result], None);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let runs = parsed["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 1);

        let driver = &runs[0]["tool"]["driver"];
        assert_eq!(driver["name"], "Rigor");
        assert!(driver["version"].is_string());
        assert!(driver["informationUri"]
            .as_str()
            .unwrap()
            .starts_with("https://"));
        assert!(driver["rules"].is_array());
    }

    #[test]
    fn sarif_results_have_correct_structure() {
        let issues = vec![
            Issue {
                rule: Rule::WeakAssertion,
                severity: Severity::Warning,
                message: "Using toBeTruthy instead of toBe".to_string(),
                location: Location::new(10, 5).with_end(10, 30),
                suggestion: Some("Use toBe(true) instead".to_string()),
            },
            Issue {
                rule: Rule::NoAssertions,
                severity: Severity::Error,
                message: "Test has no assertions".to_string(),
                location: Location::new(25, 3),
                suggestion: None,
            },
        ];

        let reporter = SarifReporter::new();
        let result = make_result_with_issues(issues);
        let output = reporter.report(&[result], None);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let results = parsed["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);

        // Check first result structure
        let r0 = &results[0];
        assert!(r0["ruleId"].is_string());
        assert!(r0["level"].is_string());
        assert!(r0["message"]["text"].is_string());

        // Check location structure
        let locations = r0["locations"].as_array().unwrap();
        assert_eq!(locations.len(), 1);
        let phys = &locations[0]["physicalLocation"];
        assert!(phys["artifactLocation"]["uri"]
            .as_str()
            .unwrap()
            .starts_with("file://"));
        assert!(phys["region"]["startLine"].is_number());
    }

    #[test]
    fn sarif_severity_mapping() {
        let issues = vec![
            Issue {
                rule: Rule::NoAssertions,
                severity: Severity::Error,
                message: "err".to_string(),
                location: Location::new(1, 1),
                suggestion: None,
            },
            Issue {
                rule: Rule::WeakAssertion,
                severity: Severity::Warning,
                message: "warn".to_string(),
                location: Location::new(2, 1),
                suggestion: None,
            },
            Issue {
                rule: Rule::HardcodedValues,
                severity: Severity::Info,
                message: "info".to_string(),
                location: Location::new(3, 1),
                suggestion: None,
            },
        ];

        let reporter = SarifReporter::new();
        let result = make_result_with_issues(issues);
        let output = reporter.report(&[result], None);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let results = parsed["runs"][0]["results"].as_array().unwrap();
        let levels: Vec<&str> = results
            .iter()
            .map(|r| r["level"].as_str().unwrap())
            .collect();

        assert!(levels.contains(&"error"));
        assert!(levels.contains(&"warning"));
        assert!(levels.contains(&"note"));
    }

    #[test]
    fn sarif_driver_rules_match_issues() {
        let issues = vec![
            Issue {
                rule: Rule::WeakAssertion,
                severity: Severity::Warning,
                message: "weak".to_string(),
                location: Location::new(1, 1),
                suggestion: None,
            },
            Issue {
                rule: Rule::MissingErrorTest,
                severity: Severity::Warning,
                message: "missing err".to_string(),
                location: Location::new(2, 1),
                suggestion: None,
            },
        ];

        let reporter = SarifReporter::new();
        let result = make_result_with_issues(issues);
        let output = reporter.report(&[result], None);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let driver_rules = parsed["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap();
        let rule_ids: Vec<&str> = driver_rules
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect();

        assert!(rule_ids.contains(&"weak-assertion"));
        assert!(rule_ids.contains(&"missing-error-test"));
    }

    #[test]
    fn sarif_physical_location_has_file_line_info() {
        let issue = Issue {
            rule: Rule::DebugCode,
            severity: Severity::Warning,
            message: "console.log found".to_string(),
            location: Location::new(42, 8).with_end(42, 25),
            suggestion: None,
        };

        let reporter = SarifReporter::new();
        let result = make_result_with_issues(vec![issue]);
        let output = reporter.report(&[result], None);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let loc = &parsed["runs"][0]["results"][0]["locations"][0]["physicalLocation"];
        assert_eq!(loc["region"]["startLine"], 42);
        assert_eq!(loc["region"]["startColumn"], 8);
        assert_eq!(loc["region"]["endLine"], 42);
        assert_eq!(loc["region"]["endColumn"], 25);

        let uri = loc["artifactLocation"]["uri"].as_str().unwrap();
        assert!(
            uri.contains("auth.test.ts"),
            "URI should contain the file name, got: {}",
            uri
        );
    }

    #[test]
    fn sarif_empty_results_produces_valid_output() {
        let reporter = SarifReporter::new();
        let output = reporter.report(&[], None);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["version"], "2.1.0");
        let results = parsed["runs"][0]["results"].as_array().unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn sarif_multiple_files() {
        let r1 = make_result_with_issues(vec![Issue {
            rule: Rule::WeakAssertion,
            severity: Severity::Warning,
            message: "w1".to_string(),
            location: Location::new(1, 1),
            suggestion: None,
        }]);
        let mut r2 = make_result_with_issues(vec![Issue {
            rule: Rule::NoAssertions,
            severity: Severity::Error,
            message: "e1".to_string(),
            location: Location::new(5, 1),
            suggestion: None,
        }]);
        r2.file_path = PathBuf::from("/src/tests/cart.test.ts");

        let reporter = SarifReporter::new();
        let output = reporter.report(&[r1, r2], None);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let results = parsed["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);

        // Results should reference different files
        let uris: Vec<&str> = results
            .iter()
            .map(|r| {
                r["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                    .as_str()
                    .unwrap()
            })
            .collect();
        assert!(uris.iter().any(|u| u.contains("auth.test.ts")));
        assert!(uris.iter().any(|u| u.contains("cart.test.ts")));
    }

    #[test]
    fn path_to_uri_unix() {
        assert_eq!(
            path_to_uri(Path::new("/home/user/test.ts")),
            "file:///home/user/test.ts"
        );
    }

    #[test]
    fn path_to_uri_relative() {
        assert_eq!(path_to_uri(Path::new("src/test.ts")), "file:///src/test.ts");
    }
}
