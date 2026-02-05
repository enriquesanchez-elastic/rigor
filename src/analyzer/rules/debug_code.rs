//! Debug code and leftover development artifacts in tests

use super::AnalysisRule;
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule for detecting debug code left in test files
pub struct DebugCodeRule;

impl DebugCodeRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DebugCodeRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for DebugCodeRule {
    fn name(&self) -> &'static str {
        "debug-code"
    }

    fn analyze(&self, tests: &[TestCase], source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        for (zero_indexed, line) in source.lines().enumerate() {
            let line_no = zero_indexed + 1;
            let trimmed = line.trim();

            // Skip comment-only lines (we don't flag comments that contain console.log as debug)
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
                // But do check for commented-out tests
                if (trimmed.contains("it(") || trimmed.contains("test("))
                    && (trimmed.contains("// it(") || trimmed.contains("// test("))
                    && !trimmed.contains("rigor-ignore")
                {
                    issues.push(Issue {
                        rule: Rule::DebugCode,
                        severity: Severity::Info,
                        message: "Commented-out test code - remove or restore the test".to_string(),
                        location: Location::new(line_no, 1),
                        suggestion: Some(
                            "Delete commented code or uncomment to run the test".to_string(),
                        ),
                    });
                }
                continue;
            }

            // console.log, console.debug, console.warn (not in string literals - simple check)
            if !trimmed.contains('"') && !trimmed.contains('\'') {
                if trimmed.contains("console.log(") {
                    issues.push(Issue {
                        rule: Rule::DebugCode,
                        severity: Severity::Info,
                        message: "Test contains console.log - remove debugging code".to_string(),
                        location: Location::new(line_no, 1),
                        suggestion: Some(
                            "Remove console.log or use a proper logging mock".to_string(),
                        ),
                    });
                } else if trimmed.contains("console.debug(") {
                    issues.push(Issue {
                        rule: Rule::DebugCode,
                        severity: Severity::Info,
                        message: "Test contains console.debug - remove debugging code".to_string(),
                        location: Location::new(line_no, 1),
                        suggestion: Some("Remove console.debug from tests".to_string()),
                    });
                } else if trimmed.contains("console.warn(") && !trimmed.starts_with("expect") {
                    issues.push(Issue {
                        rule: Rule::DebugCode,
                        severity: Severity::Info,
                        message: "Test contains console.warn - remove debugging code".to_string(),
                        location: Location::new(line_no, 1),
                        suggestion: Some("Remove console.warn from tests".to_string()),
                    });
                }
            }

            // debugger statement
            if trimmed.contains("debugger") {
                let col = line.find("debugger").unwrap_or(0) + 1;
                issues.push(Issue {
                    rule: Rule::DebugCode,
                    severity: Severity::Warning,
                    message: "debugger statement left in test".to_string(),
                    location: Location::new(line_no, col),
                    suggestion: Some("Remove debugger statement before committing".to_string()),
                });
            }
        }

        // Focused tests: it.only, test.only, fit, ftest (run only that test, often left by mistake)
        for test in tests {
            if test.is_skipped {
                continue;
            }
            let name_lower = test.name.to_lowercase();
            if name_lower.contains("only") {
                // Could be "should only allow..." - check test body via source around location
                // Simpler: we don't have "is_focused" on TestCase. So we need to detect it.only in parser.
                // The test_file parser already distinguishes it.skip vs it.only. We don't currently set is_focused.
                // So add a separate check: scan source for "it.only" or "test.only" or " fit(" or " ftest("
                // and report. Let's do that in the same loop above by checking each line for .only
            }
        }

        // Scan again for it.only / test.only / fit / ftest
        for (zero_indexed, line) in source.lines().enumerate() {
            let line_no = zero_indexed + 1;
            let trimmed = line.trim();
            if trimmed.contains("it.only(")
                || trimmed.contains("test.only(")
                || trimmed.contains("fit(")
                || trimmed.contains("ftest(")
            {
                issues.push(Issue {
                    rule: Rule::FocusedTest,
                    severity: Severity::Warning,
                    message: "Focused test (.only) - will skip other tests when run".to_string(),
                    location: Location::new(line_no, 1),
                    suggestion: Some("Remove .only to run the full test suite".to_string()),
                });
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let mut score: i32 = 25;

        let debug_count = issues.iter().filter(|i| i.rule == Rule::DebugCode).count();
        let focused_count = issues
            .iter()
            .filter(|i| i.rule == Rule::FocusedTest)
            .count();

        score -= (debug_count as i32 * 2).min(8);
        score -= (focused_count as i32 * 5).min(15);

        score.clamp(0, 25) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Issue, Location, Severity, TestCase};

    fn make_empty_tests() -> Vec<TestCase> {
        vec![TestCase {
            name: "test".to_string(),
            location: Location::new(1, 1),
            is_async: false,
            is_skipped: false,
            assertions: vec![],
            describe_block: None,
        }]
    }

    #[test]
    fn positive_detects_console_log() {
        let rule = DebugCodeRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let source = "  console.log(debug);";
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::DebugCode));
    }

    #[test]
    fn positive_detects_it_only() {
        let rule = DebugCodeRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let source = "it.only('test', () => { expect(1).toBe(1); });";
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::FocusedTest));
    }

    #[test]
    fn negative_clean_source_no_issues() {
        let rule = DebugCodeRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let source = "it('adds numbers', () => { expect(1 + 1).toBe(2); });";
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(issues.is_empty());
    }

    #[test]
    fn score_decreases_with_issues() {
        let rule = DebugCodeRule::new();
        let tests = make_empty_tests();
        let zero_issues: Vec<Issue> = vec![];
        let one_debug = vec![Issue {
            rule: Rule::DebugCode,
            severity: Severity::Info,
            message: "test".to_string(),
            location: Location::new(1, 1),
            suggestion: None,
        }];
        assert_eq!(rule.calculate_score(&tests, &zero_issues), 25);
        assert_eq!(rule.calculate_score(&tests, &one_debug), 23);
    }
}
