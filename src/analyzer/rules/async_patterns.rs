//! Async test patterns - missing await on promises

use super::AnalysisRule;
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule for detecting missing await in async tests
pub struct AsyncPatternsRule;

impl AsyncPatternsRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AsyncPatternsRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for AsyncPatternsRule {
    fn name(&self) -> &'static str {
        "async-patterns"
    }

    fn analyze(&self, tests: &[TestCase], source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        for (zero_indexed, line) in source.lines().enumerate() {
            let line_no = zero_indexed + 1;
            let trimmed = line.trim();

            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }

            // expect(...).resolves or expect(...).rejects without await expect
            let has_resolves = trimmed.contains(".resolves.");
            let has_rejects = trimmed.contains(".rejects.");
            let has_await_expect = trimmed.contains("await expect(") || trimmed.contains("await expect (");

            if (has_resolves || has_rejects) && !has_await_expect {
                let col = line.find("expect(").unwrap_or(0) + 1;
                let kind = if has_resolves { "resolves" } else { "rejects" };
                issues.push(Issue {
                    rule: Rule::MissingAwait,
                    severity: Severity::Warning,
                    message: format!(
                        "expect().{} used without await - use 'await expect(...).{}' in async tests",
                        kind, kind
                    ),
                    location: Location::new(line_no, col),
                    suggestion: Some(
                        "Prefer: await expect(asyncFn()).resolves.toBe(value) or await expect(promise).rejects.toThrow()".to_string(),
                    ),
                });
            }
        }

        // Async tests with no await in body (heuristic: test is async but body has no "await ")
        for test in tests {
            if !test.is_async || test.is_skipped {
                continue;
            }
            // We don't have the test body text easily - we'd need to get the source range for the test.
            // Skip this check for now or do a simple scan: in the lines between test.location.line and test.location.end_line,
            // check for "await ". If async test and no await in that range, warn.
            let start = test.location.line.saturating_sub(1);
            let end_line = test
                .location
                .end_line
                .unwrap_or(test.location.line + 49);
            let line_count = end_line.saturating_sub(test.location.line) + 1;
            let test_lines: Vec<&str> = source.lines().skip(start).take(line_count).collect();
            let has_await = test_lines.iter().any(|l| {
                let t = l.trim();
                !t.starts_with("//") && t.contains("await ")
            });
            if !has_await && test_lines.len() > 1 {
                // Might be false positive if test only returns a promise
                let has_returns_promise = test_lines.iter().any(|l| {
                    l.contains("return ") && (l.contains("expect(") || l.contains("Promise"))
                });
                if !has_returns_promise {
                    issues.push(Issue {
                        rule: Rule::MissingAwait,
                        severity: Severity::Info,
                        message: format!(
                            "Async test '{}' may be missing await on async calls",
                            test.name
                        ),
                        location: test.location.clone(),
                        suggestion: Some(
                            "Ensure async operations are awaited to avoid race conditions".to_string(),
                        ),
                    });
                }
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let count = issues
            .iter()
            .filter(|i| i.rule == Rule::MissingAwait)
            .count();
        let mut score: i32 = 25;
        score -= (count as i32 * 3).min(15);
        score.clamp(0, 25) as u8
    }
}
