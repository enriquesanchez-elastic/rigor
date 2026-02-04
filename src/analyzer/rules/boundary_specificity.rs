//! Boundary specificity rule
//!
//! Flags cases where boundary conditions are tested with nearby values but the assertion
//! doesn't verify the exact boundary behavior.

use super::AnalysisRule;
use crate::{Issue, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule that checks tests use exact boundary values, not just "near" values.
pub struct BoundarySpecificityRule;

impl BoundarySpecificityRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BoundarySpecificityRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for BoundarySpecificityRule {
    fn name(&self) -> &'static str {
        "boundary-specificity"
    }

    fn analyze(&self, tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        // Heuristic: if test name mentions "boundary" or "edge" but assertions only use
        // toBeTruthy / toBeDefined, the boundary value might not be asserted exactly.
        for test in tests {
            let name_lower = test.name.to_lowercase();
            let mentions_boundary = name_lower.contains("boundary")
                || name_lower.contains("edge")
                || name_lower.contains("limit")
                || name_lower.contains("min")
                || name_lower.contains("max");

            if !mentions_boundary {
                continue;
            }

            let has_exact_assertion = test.assertions.iter().any(|a| {
                let raw = a.raw.as_str();
                raw.contains(".toBe(")
                    || raw.contains(".toEqual(")
                    || raw.contains(".toStrictEqual(")
                    || raw.contains(".toHaveLength(")
            });

            if !has_exact_assertion && !test.assertions.is_empty() {
                let loc = test.location.clone();
                issues.push(Issue {
                    rule: Rule::BoundarySpecificity,
                    severity: Severity::Info,
                    message: format!(
                        "Test '{}' mentions boundary/edge but doesn't assert exact value",
                        test.name
                    ),
                    location: loc,
                    suggestion: Some(
                        "Assert exact boundary: e.g. expect(fn(17)).toBe(false); expect(fn(18)).toBe(true)".to_string(),
                    ),
                });
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], _issues: &[Issue]) -> u8 {
        25
    }
}
