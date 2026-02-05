//! Mock abuse detection - excessive or inappropriate mocking

use super::AnalysisRule;
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

    /// Count jest.mock() / vi.mock() calls in source
    fn count_mocks(source: &str) -> usize {
        source.matches("jest.mock(").count() + source.matches("vi.mock(").count()
    }

    /// Find mocked module paths (simplified: look for jest.mock('...') or jest.mock(\"...\"))
    fn mocked_modules(source: &str) -> Vec<(usize, String)> {
        let mut result = Vec::new();
        for (line_no, line) in source.lines().enumerate() {
            let line = line.trim();
            if line.contains("jest.mock(") || line.contains("vi.mock(") {
                // Extract first string argument - simple scan for ' or " quoted string
                if let Some(start) = line.find("mock(") {
                    let after = &line[start + 5..];
                    let rest = after.trim_start();
                    let quote = rest.chars().next();
                    if quote == Some('\'') || quote == Some('"') {
                        let end_char = quote.unwrap();
                        if let Some(end) = rest[1..].find(end_char) {
                            let module = rest[1..1 + end].to_string();
                            result.push((line_no + 1, module));
                        }
                    }
                }
            }
        }
        result
    }

    /// Check if any mocked module looks like the module under test (e.g. same file name)
    #[allow(dead_code)]
    fn mocks_module_under_test(
        _source: &str,
        mocked: &[(usize, String)],
        test_file_path: Option<&str>,
    ) -> Option<(usize, String)> {
        let under_test = test_file_path.and_then(|p| {
            let stem = std::path::Path::new(p).file_stem()?;
            let stem = stem.to_str()?;
            let stem = stem
                .trim_end_matches(".test")
                .trim_end_matches(".spec")
                .trim_end_matches("_test")
                .trim_end_matches("_spec");
            Some(stem.to_string())
        })?;
        for (line, module) in mocked {
            let mod_stem = module
                .trim_start_matches('.')
                .trim_start_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or(module)
                .trim_end_matches(".ts")
                .trim_end_matches(".tsx")
                .trim_end_matches(".js")
                .trim_end_matches(".jsx");
            if mod_stem == under_test {
                return Some((*line, module.clone()));
            }
        }
        None
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

    fn analyze(&self, _tests: &[TestCase], source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        let count = Self::count_mocks(source);
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
            });
        }

        let mocked = Self::mocked_modules(source);
        for (line_no, module) in &mocked {
            let mod_trimmed = module.trim_matches(|c| c == '\'' || c == '"');
            for std_name in STD_MOCKS {
                if mod_trimmed.contains(std_name) && mod_trimmed.len() < 30 {
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
                    });
                    break;
                }
            }
        }

        // Mocking module under test - we don't have test file path in the rule, so we skip that check
        // (engine could pass it later; for now we only do count and std mock checks)

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
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let source = (0..6)
            .map(|_| "jest.mock('foo');")
            .collect::<Vec<_>>()
            .join("\n");
        let issues = rule.analyze(&make_empty_tests(), &source, &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::MockAbuse));
    }

    #[test]
    fn positive_detects_std_lib_mock() {
        let rule = MockAbuseRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let source = "jest.mock('Math');";
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::MockAbuse));
    }

    #[test]
    fn negative_few_mocks_no_issue() {
        let rule = MockAbuseRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let source = "jest.mock('my-module');\nit('works', () => {});";
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
        }];
        assert_eq!(rule.calculate_score(&tests, &zero_issues), 25);
        assert_eq!(rule.calculate_score(&tests, &one_issue), 21);
    }
}
