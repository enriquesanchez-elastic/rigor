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
