//! Missing cleanup: afterEach or reset mocks not used when needed.

use super::AnalysisRule;
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule for detecting missing test cleanup
pub struct MissingCleanupRule;

impl MissingCleanupRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MissingCleanupRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for MissingCleanupRule {
    fn name(&self) -> &'static str {
        "missing-cleanup"
    }

    fn analyze(&self, _tests: &[TestCase], source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        let has_fake_timers = source.contains("useFakeTimers");
        let has_use_real =
            source.contains("useRealTimers") || source.contains("runOnlyPendingTimers");
        let has_after_each = source.contains("afterEach");

        if has_fake_timers && !has_use_real && !has_after_each {
            if let Some((line_no, _)) = source
                .lines()
                .enumerate()
                .find(|(_, line)| line.contains("useFakeTimers"))
            {
                issues.push(Issue {
                    rule: Rule::MissingCleanup,
                    severity: Severity::Info,
                    message:
                        "useFakeTimers() without cleanup - consider afterEach with useRealTimers()"
                            .to_string(),
                    location: Location::new(line_no + 1, 1),
                    suggestion: Some(
                        "Add afterEach(() => { jest.useRealTimers(); }) or vi.useRealTimers()"
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
            .filter(|i| i.rule == Rule::MissingCleanup)
            .count();
        (25i32 - (n as i32 * 2).min(10)).max(0) as u8
    }
}
