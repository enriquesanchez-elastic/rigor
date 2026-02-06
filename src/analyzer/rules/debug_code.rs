//! Debug code and leftover development artifacts in tests.
//! Uses tree-sitter queries (see parser::queries) for console.*, debugger, and .only when possible.

use super::AnalysisRule;
use crate::parser::{global_query_cache, QueryId, TypeScriptParser};
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

    fn analyze(&self, _tests: &[TestCase], source: &str, tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();
        let lang = TypeScriptParser::language();
        let cache = global_query_cache();

        // Tree-sitter queries for console.*, debugger, and .only (AST-based)
        let mut used_query_console = false;
        let mut used_query_debugger = false;
        let mut used_query_only = false;

        if let Ok(console_matches) =
            cache.run_cached_query(source, tree, &lang, QueryId::ConsoleCall)
        {
            used_query_console = true;
            for caps in console_matches {
                let obj = caps
                    .iter()
                    .find(|c| c.name == "obj")
                    .map(|c| c.text.as_str())
                    .unwrap_or("");
                let prop = caps
                    .iter()
                    .find(|c| c.name == "prop")
                    .map(|c| c.text.as_str())
                    .unwrap_or("");
                if obj == "console" && matches!(prop, "log" | "debug" | "warn" | "error") {
                    let (line, col) = caps.first().map(|c| c.start_point).unwrap_or((1, 1));
                    let msg = match prop {
                        "log" => "Test contains console.log - remove debugging code",
                        "debug" => "Test contains console.debug - remove debugging code",
                        "warn" => "Test contains console.warn - remove debugging code",
                        _ => "Test contains console.* - remove debugging code",
                    };
                    issues.push(Issue {
                        rule: Rule::DebugCode,
                        severity: Severity::Info,
                        message: msg.to_string(),
                        location: Location::new(line, col),
                        suggestion: Some(
                            "Remove console.* or use a proper logging mock".to_string(),
                        ),
                        fix: None,
                    });
                }
            }
        }

        if let Ok(debugger_matches) =
            cache.run_cached_query(source, tree, &lang, QueryId::DebuggerStatement)
        {
            used_query_debugger = true;
            for caps in debugger_matches {
                let (line, col) = caps.first().map(|c| c.start_point).unwrap_or((1, 1));
                issues.push(Issue {
                    rule: Rule::DebugCode,
                    severity: Severity::Warning,
                    message: "debugger statement left in test".to_string(),
                    location: Location::new(line, col),
                    suggestion: Some("Remove debugger statement before committing".to_string()),
                    fix: None,
                });
            }
        }

        if let Ok(only_matches) =
            cache.run_cached_query(source, tree, &lang, QueryId::FocusedTestOnly)
        {
            used_query_only = true;
            for caps in only_matches {
                let obj = caps
                    .iter()
                    .find(|c| c.name == "obj")
                    .map(|c| c.text.as_str())
                    .unwrap_or("");
                let prop = caps
                    .iter()
                    .find(|c| c.name == "prop")
                    .map(|c| c.text.as_str())
                    .unwrap_or("");
                if prop == "only" && matches!(obj, "it" | "test" | "describe") {
                    let (line, col) = caps.first().map(|c| c.start_point).unwrap_or((1, 1));
                    issues.push(Issue {
                        rule: Rule::FocusedTest,
                        severity: Severity::Warning,
                        message: "Focused test (.only) - will skip other tests when run"
                            .to_string(),
                        location: Location::new(line, col),
                        suggestion: Some("Remove .only to run the full test suite".to_string()),
                        fix: None,
                    });
                }
            }
        }

        // Fallback: line-based when queries fail or grammar doesn't support (e.g. parse error)
        if !used_query_console || !used_query_debugger || !used_query_only {
            for (zero_indexed, line) in source.lines().enumerate() {
                let line_no = zero_indexed + 1;
                let trimmed = line.trim();
                if trimmed.starts_with("//")
                    || trimmed.starts_with("/*")
                    || trimmed.starts_with('*')
                {
                    continue;
                }
                if !used_query_console && !trimmed.contains('"') && !trimmed.contains('\'') {
                    if trimmed.contains("console.log(") {
                        issues.push(Issue {
                            rule: Rule::DebugCode,
                            severity: Severity::Info,
                            message: "Test contains console.log - remove debugging code"
                                .to_string(),
                            location: Location::new(line_no, 1),
                            suggestion: Some(
                                "Remove console.log or use a proper logging mock".to_string(),
                            ),
                            fix: None,
                        });
                    } else if trimmed.contains("console.debug(") {
                        issues.push(Issue {
                            rule: Rule::DebugCode,
                            severity: Severity::Info,
                            message: "Test contains console.debug - remove debugging code"
                                .to_string(),
                            location: Location::new(line_no, 1),
                            suggestion: Some("Remove console.debug from tests".to_string()),
                            fix: None,
                        });
                    } else if trimmed.contains("console.warn(") && !trimmed.starts_with("expect") {
                        issues.push(Issue {
                            rule: Rule::DebugCode,
                            severity: Severity::Info,
                            message: "Test contains console.warn - remove debugging code"
                                .to_string(),
                            location: Location::new(line_no, 1),
                            suggestion: Some("Remove console.warn from tests".to_string()),
                            fix: None,
                        });
                    }
                }
                if !used_query_debugger && trimmed.contains("debugger") {
                    let col = line.find("debugger").unwrap_or(0) + 1;
                    issues.push(Issue {
                        rule: Rule::DebugCode,
                        severity: Severity::Warning,
                        message: "debugger statement left in test".to_string(),
                        location: Location::new(line_no, col),
                        suggestion: Some("Remove debugger statement before committing".to_string()),
                        fix: None,
                    });
                }
                if !used_query_only
                    && (trimmed.contains("it.only(")
                        || trimmed.contains("test.only(")
                        || trimmed.contains("describe.only(")
                        || trimmed.contains("fit(")
                        || trimmed.contains("ftest("))
                {
                    issues.push(Issue {
                        rule: Rule::FocusedTest,
                        severity: Severity::Warning,
                        message: "Focused test (.only) - will skip other tests when run"
                            .to_string(),
                        location: Location::new(line_no, 1),
                        suggestion: Some("Remove .only to run the full test suite".to_string()),
                        fix: None,
                    });
                }
            }
        }

        // Line-based: commented-out test code (not covered by AST queries)
        for (zero_indexed, line) in source.lines().enumerate() {
            let line_no = zero_indexed + 1;
            let trimmed = line.trim();
            if (trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*'))
                && (trimmed.contains("it(") || trimmed.contains("test("))
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
                    fix: None,
                });
            }
        }

        // Fallback: fit( / ftest( are not member_expression (no .only), so scan lines
        for (zero_indexed, line) in source.lines().enumerate() {
            let line_no = zero_indexed + 1;
            let trimmed = line.trim();
            if trimmed.contains("fit(") || trimmed.contains("ftest(") {
                issues.push(Issue {
                    rule: Rule::FocusedTest,
                    severity: Severity::Warning,
                    message: "Focused test (.only) - will skip other tests when run".to_string(),
                    location: Location::new(line_no, 1),
                    suggestion: Some("Remove .only to run the full test suite".to_string()),
                    fix: None,
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
        let source = "  console.log(debug);";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::DebugCode));
    }

    #[test]
    fn positive_detects_it_only() {
        let rule = DebugCodeRule::new();
        let source = "it.only('test', () => { expect(1).toBe(1); });";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
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
            fix: None,
        }];
        assert_eq!(rule.calculate_score(&tests, &zero_issues), 25);
        assert_eq!(rule.calculate_score(&tests, &one_debug), 23);
    }
}
