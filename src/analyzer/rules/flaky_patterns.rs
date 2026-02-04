//! Flaky test pattern detection - non-deterministic code without mocks

use super::AnalysisRule;
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule for detecting flaky patterns (Date.now, Math.random, timers, etc.)
pub struct FlakyPatternsRule;

impl FlakyPatternsRule {
    pub fn new() -> Self {
        Self
    }

    /// Check if file has fake timers (jest.useFakeTimers, vi.useFakeTimers)
    fn has_fake_timers(source: &str) -> bool {
        source.contains("useFakeTimers")
    }

    /// Check if Math.random is mocked
    fn has_random_mock(source: &str) -> bool {
        source.contains("spyOn(Math")
            || source.contains("spyOn(globalThis, 'Math')")
            || source.contains("mockReturnValue")
                && (source.contains("random") || source.contains("Math"))
    }

    /// Check if fetch/network is mocked
    fn has_fetch_mock(source: &str) -> bool {
        source.contains("jest.mock(") && (source.contains("fetch") || source.contains("axios"))
            || source.contains("vi.mock(") && (source.contains("fetch") || source.contains("axios"))
            || source.contains("mockImplementation") && source.contains("fetch")
    }
}

impl Default for FlakyPatternsRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for FlakyPatternsRule {
    fn name(&self) -> &'static str {
        "flaky-patterns"
    }

    fn analyze(&self, _tests: &[TestCase], source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();
        let has_fake_timers = Self::has_fake_timers(source);
        let has_random_mock = Self::has_random_mock(source);
        let has_fetch_mock = Self::has_fetch_mock(source);

        for (zero_indexed, line) in source.lines().enumerate() {
            let line_no = zero_indexed + 1;
            let trimmed = line.trim();

            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }

            // Date.now() without mock
            if trimmed.contains("Date.now()") && !has_fake_timers {
                let col = line.find("Date.now()").unwrap_or(0) + 1;
                issues.push(Issue {
                    rule: Rule::FlakyPattern,
                    severity: Severity::Warning,
                    message: "Date.now() is non-deterministic - use jest.useFakeTimers() or mock it"
                        .to_string(),
                    location: Location::new(line_no, col),
                    suggestion: Some(
                        "Use jest.useFakeTimers() and jest.setSystemTime() for deterministic dates"
                            .to_string(),
                    ),
                });
            }

            // new Date() without freeze
            if (trimmed.contains("new Date()") || trimmed.contains("new Date (")) && !has_fake_timers
            {
                let col = line.find("new Date").unwrap_or(0) + 1;
                issues.push(Issue {
                    rule: Rule::FlakyPattern,
                    severity: Severity::Info,
                    message: "new Date() is non-deterministic - consider jest.useFakeTimers()"
                        .to_string(),
                    location: Location::new(line_no, col),
                    suggestion: Some("Use fake timers to control time in tests".to_string()),
                });
            }

            // Math.random() without mock
            if trimmed.contains("Math.random()") && !has_random_mock {
                let col = line.find("Math.random()").unwrap_or(0) + 1;
                issues.push(Issue {
                    rule: Rule::FlakyPattern,
                    severity: Severity::Warning,
                    message: "Math.random() is non-deterministic - mock it for reproducible tests"
                        .to_string(),
                    location: Location::new(line_no, col),
                    suggestion: Some(
                        "Use jest.spyOn(Math, 'random').mockReturnValue(0.5) or similar".to_string(),
                    ),
                });
            }

            // setTimeout/setInterval with literal delay (potential race)
            if (trimmed.contains("setTimeout(") || trimmed.contains("setInterval("))
                && !has_fake_timers
            {
                // Only flag if there's a numeric literal (e.g. setTimeout(..., 1000))
                let has_literal_delay = trimmed.matches(char::is_numeric).count() >= 1;
                if has_literal_delay {
                    let col = line
                    .find("setTimeout")
                    .or_else(|| line.find("setInterval"))
                    .unwrap_or(0)
                    + 1;
                    issues.push(Issue {
                        rule: Rule::FlakyPattern,
                        severity: Severity::Warning,
                        message: "setTimeout/setInterval with literal delay can cause flaky tests"
                            .to_string(),
                        location: Location::new(line_no, col),
                        suggestion: Some(
                            "Use jest.useFakeTimers() and jest.advanceTimersByTime()".to_string(),
                        ),
                    });
                }
            }

            // fetch() or axios without mock (in unit test context - simple heuristic)
            if (trimmed.contains("fetch(") || trimmed.contains("axios.") || trimmed.contains("axios("))
                && !trimmed.contains("mock")
                && !has_fetch_mock
            {
                // Avoid double-reporting per line
                if !issues.iter().any(|i| i.location.line == line_no && i.rule == Rule::FlakyPattern)
                {
                    let col = line.find("fetch(").or_else(|| line.find("axios")).unwrap_or(0) + 1;
                    issues.push(Issue {
                        rule: Rule::FlakyPattern,
                        severity: Severity::Warning,
                        message: "Network call (fetch/axios) without mock - test may be flaky or slow"
                            .to_string(),
                        location: Location::new(line_no, col),
                        suggestion: Some(
                            "Mock fetch/axios with jest.mock() or MSW for unit tests".to_string(),
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
            .filter(|i| i.rule == Rule::FlakyPattern)
            .count();
        let mut score: i32 = 25;
        score -= (count as i32 * 4).min(20);
        score.clamp(0, 25) as u8
    }
}
