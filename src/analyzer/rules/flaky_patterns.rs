//! Flaky test pattern detection - non-deterministic code without mocks

use super::AnalysisRule;
use crate::{Issue, Location, Rule, Severity, TestCase, TestFramework};
use tree_sitter::Tree;

enum TimerSuggestionKind {
    Date,
    Advance,
}

/// Rule for detecting flaky patterns (Date.now, Math.random, timers, etc.)
pub struct FlakyPatternsRule {
    framework: Option<TestFramework>,
}

impl FlakyPatternsRule {
    pub fn new() -> Self {
        Self { framework: None }
    }

    pub fn with_framework(mut self, framework: TestFramework) -> Self {
        self.framework = Some(framework);
        self
    }

    fn timer_suggestion(&self, kind: TimerSuggestionKind) -> String {
        match self.framework {
            Some(TestFramework::Vitest) => match kind {
                TimerSuggestionKind::Date => {
                    "Use vi.useFakeTimers() and vi.setSystemTime() for deterministic dates"
                        .to_string()
                }
                TimerSuggestionKind::Advance => {
                    "Use vi.useFakeTimers() and vi.advanceTimersByTime()".to_string()
                }
            },
            Some(TestFramework::Playwright) => match kind {
                TimerSuggestionKind::Date => {
                    "Use page.clock or test.use(clock) for deterministic time in Playwright"
                        .to_string()
                }
                TimerSuggestionKind::Advance => {
                    "Use page.clock and advance time with clock.tick() in Playwright".to_string()
                }
            },
            Some(TestFramework::Cypress) => match kind {
                TimerSuggestionKind::Date => {
                    "Use cy.clock() and cy.tick() for deterministic dates in Cypress".to_string()
                }
                TimerSuggestionKind::Advance => {
                    "Use cy.clock() and cy.tick() to control timers in Cypress".to_string()
                }
            },
            _ => match kind {
                TimerSuggestionKind::Date => {
                    "Use jest.useFakeTimers() and jest.setSystemTime() for deterministic dates"
                        .to_string()
                }
                TimerSuggestionKind::Advance => {
                    "Use jest.useFakeTimers() and jest.advanceTimersByTime()".to_string()
                }
            },
        }
    }

    fn random_mock_suggestion(&self) -> String {
        match self.framework {
            Some(TestFramework::Vitest) => {
                "Use vi.spyOn(Math, 'random').mockReturnValue(0.5) or similar".to_string()
            }
            Some(TestFramework::Playwright) | Some(TestFramework::Cypress) => {
                "Mock Math.random for deterministic results in this test".to_string()
            }
            _ => "Use jest.spyOn(Math, 'random').mockReturnValue(0.5) or similar".to_string(),
        }
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
                    message: "Date.now() is non-deterministic - use fake timers or mock it"
                        .to_string(),
                    location: Location::new(line_no, col),
                    suggestion: Some(self.timer_suggestion(TimerSuggestionKind::Date)),
                    fix: None,
                });
            }

            // new Date() without freeze
            if (trimmed.contains("new Date()") || trimmed.contains("new Date ("))
                && !has_fake_timers
            {
                let col = line.find("new Date").unwrap_or(0) + 1;
                issues.push(Issue {
                    rule: Rule::FlakyPattern,
                    severity: Severity::Info,
                    message: "new Date() is non-deterministic - consider using fake timers"
                        .to_string(),
                    location: Location::new(line_no, col),
                    suggestion: Some(self.timer_suggestion(TimerSuggestionKind::Date)),
                    fix: None,
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
                    suggestion: Some(self.random_mock_suggestion()),
                    fix: None,
                });
            }

            // setTimeout/setInterval without fake timers — timing-dependent code is flaky.
            // Only flag when there's a numeric delay argument (regex-free: look for
            // a comma followed by digits, e.g. `setTimeout(fn, 1000)`).
            if (trimmed.contains("setTimeout(") || trimmed.contains("setInterval("))
                && !has_fake_timers
            {
                // Check for an actual numeric delay: comma, optional whitespace, digits
                let call_start = trimmed
                    .find("setTimeout(")
                    .or_else(|| trimmed.find("setInterval("));
                let has_numeric_delay = call_start
                    .and_then(|start| {
                        // Scan the argument list after the opening paren
                        let args = &trimmed[start..];
                        let comma_pos = args.find(',')?;
                        let after_comma = args[comma_pos + 1..].trim_start();
                        // Check if the next non-whitespace chars are digits
                        Some(after_comma.starts_with(|c: char| c.is_ascii_digit()))
                    })
                    .unwrap_or(false);

                if has_numeric_delay {
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
                        suggestion: Some(self.timer_suggestion(TimerSuggestionKind::Advance)),
                        fix: None,
                    });
                }
            }

            // fetch() or axios without mock (in unit test context - simple heuristic)
            if (trimmed.contains("fetch(")
                || trimmed.contains("axios.")
                || trimmed.contains("axios("))
                && !trimmed.contains("mock")
                && !has_fetch_mock
            {
                // Avoid double-reporting per line
                if !issues
                    .iter()
                    .any(|i| i.location.line == line_no && i.rule == Rule::FlakyPattern)
                {
                    let col = line
                        .find("fetch(")
                        .or_else(|| line.find("axios"))
                        .unwrap_or(0)
                        + 1;
                    issues.push(Issue {
                        rule: Rule::FlakyPattern,
                        severity: Severity::Warning,
                        message:
                            "Network call (fetch/axios) without mock - test may be flaky or slow"
                                .to_string(),
                        location: Location::new(line_no, col),
                        suggestion: Some(
                            "Mock fetch/axios with jest.mock() or MSW for unit tests".to_string(),
                        ),
                        fix: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Issue, Location, Severity, TestCase};

    fn make_empty_tests() -> Vec<TestCase> {
        vec![]
    }

    #[test]
    fn positive_detects_date_now_without_fake_timers() {
        let rule = FlakyPatternsRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let source = r#"
        it('uses time', () => {
            const t = Date.now();
            expect(t).toBeGreaterThan(0);
        });
        "#;
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(
            !issues.is_empty(),
            "should detect Date.now() without useFakeTimers"
        );
        assert!(issues.iter().any(|i| i.rule == Rule::FlakyPattern));
    }

    #[test]
    fn positive_detects_math_random_without_mock() {
        let rule = FlakyPatternsRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let source = "const x = Math.random();";
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::FlakyPattern));
    }

    #[test]
    fn negative_no_issues_with_fake_timers() {
        let rule = FlakyPatternsRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let source = r#"
        beforeEach(() => { jest.useFakeTimers(); });
        it('uses time', () => {
            const t = Date.now();
            expect(t).toBeGreaterThan(0);
        });
        "#;
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(
            issues.is_empty(),
            "useFakeTimers should suppress Date.now() issue"
        );
    }

    #[test]
    fn negative_clean_source_no_issues() {
        let rule = FlakyPatternsRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let source = "it('adds numbers', () => { expect(1 + 1).toBe(2); });";
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(issues.is_empty());
    }

    #[test]
    fn score_decreases_with_more_issues() {
        let rule = FlakyPatternsRule::new();
        let tests = make_empty_tests();
        let zero_issues: Vec<Issue> = vec![];
        let one_issue = vec![Issue {
            rule: Rule::FlakyPattern,
            severity: Severity::Warning,
            message: "test".to_string(),
            location: Location::new(1, 1),
            suggestion: None,
            fix: None,
        }];
        assert_eq!(rule.calculate_score(&tests, &zero_issues), 25);
        assert_eq!(rule.calculate_score(&tests, &one_issue), 21);
    }
}
