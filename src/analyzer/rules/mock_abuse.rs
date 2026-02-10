//! Mock abuse detection - excessive or inappropriate mocking.
//! Uses tree-sitter query to find jest.mock/vi.mock and extract module path from AST.

use super::AnalysisRule;
use crate::parser::{global_query_cache, QueryId, TypeScriptParser};
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::Tree;

const MOCK_COUNT_WARNING_THRESHOLD: usize = 5;

/// Standard library / built-in modules that are suspicious to mock
const STD_MOCKS: &[&str] = &[
    "Array",
    "Object",
    "Promise",
    "Map",
    "Set",
    "Date",
    "Math",
    "fetch",
    "globalThis",
    "process",
    "require",
];

/// Rule for detecting mock abuse
pub struct MockAbuseRule;

impl MockAbuseRule {
    pub fn new() -> Self {
        Self
    }

    /// Extract first string literal argument from a call node (e.g. jest.mock('foo') -> "foo").
    fn first_string_arg(
        tree: &Tree,
        source: &str,
        call_start: usize,
        call_end: usize,
    ) -> Option<String> {
        let root = tree.root_node();
        let call_node = root.descendant_for_byte_range(call_start, call_end)?;
        let args = call_node.child_by_field_name("arguments")?;
        let mut cursor = args.walk();
        let first_arg = args.named_children(&mut cursor).next()?;
        let text = first_arg.utf8_text(source.as_bytes()).ok()?;
        let s = text.trim();
        if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
            Some(s[1..s.len() - 1].to_string())
        } else {
            None
        }
    }
}

impl Default for MockAbuseRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for MockAbuseRule {
    fn name(&self) -> &'static str {
        "mock-abuse"
    }

    fn analyze(&self, _tests: &[TestCase], source: &str, tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();
        let lang = TypeScriptParser::language();
        let cache = global_query_cache();

        let mut mock_calls: Vec<(usize, usize, String)> = Vec::new(); // (line, col, module)

        if let Ok(matches) = cache.run_cached_query(source, tree, &lang, QueryId::MockCall) {
            for caps in matches {
                let obj = caps
                    .iter()
                    .find(|c| c.name == "obj")
                    .map(|c| c.text.as_str());
                let prop = caps
                    .iter()
                    .find(|c| c.name == "prop")
                    .map(|c| c.text.as_str());
                if obj != Some("jest") && obj != Some("vi") {
                    continue;
                }
                if prop != Some("mock") {
                    continue;
                }
                let call_cap = match caps.iter().find(|c| c.name == "call") {
                    Some(c) => c,
                    None => continue,
                };
                let (line, col) = call_cap.start_point;
                let start_byte = call_cap.start_byte;
                let end_byte = call_cap.end_byte;
                if let Some(module) = Self::first_string_arg(tree, source, start_byte, end_byte) {
                    mock_calls.push((line, col, module));
                }
            }
        }

        let count = mock_calls.len();
        if count > MOCK_COUNT_WARNING_THRESHOLD {
            issues.push(Issue {
                rule: Rule::MockAbuse,
                severity: Severity::Warning,
                message: format!(
                    "File has {} mock calls - consider if this should be an integration test",
                    count
                ),
                location: Location::new(1, 1),
                suggestion: Some(
                    "Many mocks often indicate testing implementation; prefer integration tests or fewer mocks".to_string(),
                ),
                fix: None,
            });
        }

        for (line_no, _col, module) in &mock_calls {
            let mod_trimmed = module.trim_matches(|c| c == '\'' || c == '"');
            let final_segment = mod_trimmed
                .rsplit('/')
                .next()
                .unwrap_or(mod_trimmed)
                .trim_end_matches(".ts")
                .trim_end_matches(".tsx")
                .trim_end_matches(".js")
                .trim_end_matches(".jsx");

            for std_name in STD_MOCKS {
                if final_segment == *std_name {
                    issues.push(Issue {
                        rule: Rule::MockAbuse,
                        severity: Severity::Warning,
                        message: format!(
                            "Mocking standard library '{}' can make tests brittle",
                            mod_trimmed
                        ),
                        location: Location::new(*line_no, 1),
                        suggestion: Some(
                            "Prefer dependency injection or wrapping built-ins instead of mocking them".to_string(),
                        ),
                        fix: None,
                    });
                    break;
                }
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let count = issues.iter().filter(|i| i.rule == Rule::MockAbuse).count();
        let mut score: i32 = 25;
        score -= (count as i32 * 4).min(16);
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
    fn positive_detects_excessive_mocks() {
        let rule = MockAbuseRule::new();
        let source = (0..6)
            .map(|_| "jest.mock('foo');")
            .collect::<Vec<_>>()
            .join("\n");
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(&source)
            .unwrap();
        let issues = rule.analyze(&make_empty_tests(), &source, &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::MockAbuse));
    }

    #[test]
    fn positive_detects_std_lib_mock() {
        let rule = MockAbuseRule::new();
        let source = "jest.mock('Math');";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::MockAbuse));
    }

    #[test]
    fn negative_user_map_does_not_match_map() {
        let rule = MockAbuseRule::new();
        let source = "jest.mock('../services/UserMap');";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(
            issues.is_empty(),
            "UserMap should not trigger std-lib mock warning for 'Map'"
        );
    }

    #[test]
    fn negative_map_service_does_not_match_map() {
        let rule = MockAbuseRule::new();
        let source = "jest.mock('./MapService');";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(
            issues.is_empty(),
            "MapService should not trigger std-lib mock warning for 'Map'"
        );
    }

    #[test]
    fn negative_few_mocks_no_issue() {
        let rule = MockAbuseRule::new();
        let source = "jest.mock('my-module');\nit('works', () => {});";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(issues.is_empty());
    }

    #[test]
    fn score_decreases_with_issues() {
        let rule = MockAbuseRule::new();
        let tests = make_empty_tests();
        let zero_issues: Vec<Issue> = vec![];
        let one_issue = vec![Issue {
            rule: Rule::MockAbuse,
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
