//! Mutation-resistant assertion detection
//!
//! Flags assertions that might let mutants survive (e.g. toBeGreaterThan(0) instead of toBe(3)).

use super::AnalysisRule;
use crate::{AssertionKind, Issue, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule that flags assertions which are not specific enough to kill common mutants.
pub struct MutationResistantRule;

impl MutationResistantRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MutationResistantRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for MutationResistantRule {
    fn name(&self) -> &'static str {
        "mutation-resistant"
    }

    fn analyze(&self, tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        for test in tests {
            for assertion in &test.assertions {
                let (flag, suggestion) = match &assertion.kind {
                    AssertionKind::ToBeGreaterThan | AssertionKind::ToBeLessThan => {
                        if assertion.raw.contains("> 0") || assertion.raw.contains("< 1")
                            || assertion.raw.contains(">= 0")
                        {
                            (
                            true,
                            "Replace with exact value: expect(result).toBe(3) so mutations (e.g. off-by-one) are caught".to_string(),
                        )
                        } else {
                            (false, String::new())
                        }
                    }
                    _ => {
                        if assertion.raw.contains("toHaveLength(0)")
                            || assertion.raw.contains("toHaveLength(1)")
                        {
                            (
                                true,
                                "Consider asserting exact length: expect(arr).toHaveLength(3) to catch mutation-resistant bugs".to_string(),
                            )
                        } else {
                            (false, String::new())
                        }
                    }
                };
                if flag {
                    issues.push(Issue {
                        rule: Rule::MutationResistant,
                        severity: Severity::Info,
                        message: format!(
                            "Assertion in '{}' may let mutants survive: {}",
                            test.name,
                            if assertion.raw.len() > 45 {
                                format!("{}...", &assertion.raw[..42])
                            } else {
                                assertion.raw.clone()
                            }
                        ),
                        location: assertion.location.clone(),
                        suggestion: Some(suggestion),
                    });
                }
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], _issues: &[Issue]) -> u8 {
        25
    }
}
