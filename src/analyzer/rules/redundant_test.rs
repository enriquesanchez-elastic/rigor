//! Redundant test: test duplicates another test's coverage.

use super::AnalysisRule;
use crate::{Issue, Rule, Severity, TestCase};
use std::collections::HashMap;
use tree_sitter::Tree;

/// Rule for detecting redundant tests
pub struct RedundantTestRule;

impl RedundantTestRule {
    pub fn new() -> Self {
        Self
    }

    fn assertion_signature(assertions: &[crate::Assertion]) -> String {
        let mut parts: Vec<String> = assertions.iter().map(|a| format!("{:?}", a.kind)).collect();
        parts.sort();
        parts.join("|")
    }
}

impl Default for RedundantTestRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for RedundantTestRule {
    fn name(&self) -> &'static str {
        "redundant-test"
    }

    fn analyze(&self, tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();
        let mut sig_to_tests: HashMap<String, Vec<(usize, &TestCase)>> = HashMap::new();

        for (i, test) in tests.iter().enumerate() {
            if test.is_skipped {
                continue;
            }
            let sig = Self::assertion_signature(&test.assertions);
            if sig.is_empty() {
                continue;
            }
            sig_to_tests.entry(sig).or_default().push((i, test));
        }

        for (_sig, group) in sig_to_tests {
            if group.len() < 2 {
                continue;
            }
            for (_, test) in &group[1..] {
                issues.push(Issue {
                    rule: Rule::RedundantTest,
                    severity: Severity::Info,
                    message: format!(
                        "Test '{}' may duplicate another test (same assertion pattern)",
                        test.name
                    ),
                    location: test.location.clone(),
                    suggestion: Some(
                        "Consider merging with the similar test or testing a different scenario"
                            .to_string(),
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
            .filter(|i| i.rule == Rule::RedundantTest)
            .count();
        (25i32 - (n as i32 * 2).min(10)).max(0) as u8
    }
}
