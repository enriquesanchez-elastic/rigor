//! Implementation coupling: test is tightly coupled to implementation details.

use super::AnalysisRule;
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule for detecting implementation-coupled tests
pub struct ImplementationCouplingRule;

impl ImplementationCouplingRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ImplementationCouplingRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for ImplementationCouplingRule {
    fn name(&self) -> &'static str {
        "implementation-coupling"
    }

    fn analyze(&self, _tests: &[TestCase], source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }
            if !trimmed.contains("expect(") {
                continue;
            }
            if trimmed.contains(".calls)") || trimmed.contains(".calls )") {
                let col = line.find(".calls").unwrap_or(0) + 1;
                issues.push(Issue {
                    rule: Rule::ImplementationCoupling,
                    severity: Severity::Info,
                    message: "Asserting on mock.calls couples test to implementation - prefer behavior verification".to_string(),
                    location: Location::new(i + 1, col),
                    suggestion: Some(
                        "Verify behavior (return values, side effects) instead of call order".to_string(),
                    ),
                    fix: None,
                });
            }
            if trimmed.contains(".instances)") || trimmed.contains(".instances )") {
                let col = line.find(".instances").unwrap_or(0) + 1;
                issues.push(Issue {
                    rule: Rule::ImplementationCoupling,
                    severity: Severity::Info,
                    message: "Asserting on mock.instances couples test to implementation"
                        .to_string(),
                    location: Location::new(i + 1, col),
                    suggestion: Some(
                        "Verify observable behavior instead of internal instances".to_string(),
                    ),
                    fix: None,
                });
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == Rule::ImplementationCoupling)
            .count();
        (25i32 - (n as i32 * 2).min(15)).max(0) as u8
    }
}
