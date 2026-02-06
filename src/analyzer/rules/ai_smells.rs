//! AI-specific test smell detection.
//!
//! Flags patterns that often appear in AI-generated tests: tautological assertions,
//! over-mocking, shallow input variety, happy-path-only, parrot assertions, boilerplate padding.
//! Tuned to keep false positives low on human-written tests (target <10% FP).

use super::AnalysisRule;
use crate::{Issue, Location, Rule, Severity, TestCase};
use regex::Regex;
use tree_sitter::Tree;

/// Rule that detects AI-typical test smells
pub struct AiSmellsRule;

impl AiSmellsRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AiSmellsRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for AiSmellsRule {
    fn name(&self) -> &'static str {
        "ai-smells"
    }

    fn analyze(&self, tests: &[TestCase], source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        // Tautological assertion: expect(x).toBe(x) — capture both sides and compare
        let tautological = Regex::new(
            r"expect\s*\(\s*(\w+)\s*\)\s*\.\s*to(Be|Equal|StrictEqual)\s*\(\s*(\w+)\s*\)",
        )
        .ok();
        for (zero_indexed, line) in source.lines().enumerate() {
            let line_no = zero_indexed + 1;
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.is_empty() {
                continue;
            }
            if let Some(ref re) = tautological {
                if let Some(caps) = re.captures(trimmed) {
                    let left = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    let right = caps.get(3).map(|m| m.as_str()).unwrap_or("");
                    if left == right {
                        issues.push(Issue {
                            rule: Rule::TautologicalAssertion,
                            severity: Severity::Info,
                            message: "Tautological assertion: same value on both sides (common in AI-generated tests)".to_string(),
                            location: Location::new(line_no, 1),
                            suggestion: Some("Assert the actual outcome of the code under test, not the same value twice".to_string()),
                            fix: None,
                        });
                    }
                }
            }
        }

        // Over-mocking: many jest.mock/vi.mock in one file (heuristic)
        let mock_count = source.matches("jest.mock(").count() + source.matches("vi.mock(").count();
        if mock_count >= 5 && tests.len() <= 3 {
            issues.push(Issue {
                rule: Rule::OverMocking,
                severity: Severity::Info,
                message: format!(
                    "Many mocks ({}) for few tests ({}) — may be testing implementation",
                    mock_count,
                    tests.len()
                ),
                location: Location::new(1, 1),
                suggestion: Some(
                    "Prefer testing behavior with fewer mocks or use integration tests".to_string(),
                ),
                fix: None,
            });
        }

        // Shallow variety: all tests use similar inputs (e.g. only one numeric literal type)
        let has_numbers = source.contains("expect(")
            && (source.contains(".toBe(0)") || source.contains(".toBe(1)"));
        let number_variety = source.matches(".toBe(").count();
        if tests.len() >= 3 && number_variety >= 2 && has_numbers {
            let re = Regex::new(r"\.toBe\s*\(\s*(\d+)\s*\)").unwrap();
            let unique_numbers = source
                .lines()
                .flat_map(|l| {
                    re.captures_iter(l)
                        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                        .collect::<Vec<_>>()
                })
                .collect::<std::collections::HashSet<_>>();
            if unique_numbers.len() <= 1 && tests.len() >= 3 {
                issues.push(Issue {
                    rule: Rule::ShallowVariety,
                    severity: Severity::Info,
                    message: "Tests use very limited input variety (same or few values)"
                        .to_string(),
                    location: Location::new(1, 1),
                    suggestion: Some(
                        "Add tests with 0, negative, and larger values to improve coverage"
                            .to_string(),
                    ),
                    fix: None,
                });
            }
        }

        // Happy-path-only: no error/reject/throw in test names or assertions
        let has_error_tests = tests.iter().any(|t| {
            let n = t.name.to_lowercase();
            n.contains("throw") || n.contains("error") || n.contains("reject") || n.contains("fail")
        });
        let has_to_throw = source.contains("toThrow") || source.contains("rejects");
        if tests.len() >= 4 && !has_error_tests && !has_to_throw {
            issues.push(Issue {
                rule: Rule::HappyPathOnly,
                severity: Severity::Info,
                message: "No tests for errors or failure paths (happy-path-only)".to_string(),
                location: Location::new(1, 1),
                suggestion: Some("Add tests that expect errors: expect(() => fn(bad)).toThrow() or expect(promise).rejects".to_string()),
                fix: None,
            });
        }

        // Parrot assertion: test name repeats generic wording like "works" or "returns value"
        let parrot_names = [
            "works",
            "returns value",
            "returns result",
            "is correct",
            "succeeds",
        ];
        for test in tests {
            let name_lower = test.name.to_lowercase();
            if parrot_names
                .iter()
                .any(|p| name_lower == *p || name_lower.trim() == *p)
            {
                issues.push(Issue {
                    rule: Rule::ParrotAssertion,
                    severity: Severity::Info,
                    message: format!("Vague test name '{}' — describe the scenario and expected outcome", test.name),
                    location: test.location.clone(),
                    suggestion: Some("Use a name that describes input and expected result, e.g. 'returns 404 when user not found'".to_string()),
                    fix: None,
                });
            }
        }

        // Boilerplate padding: many beforeEach/setup lines vs few assertions
        let setup_lines = source
            .lines()
            .filter(|l| {
                let t = l.trim();
                t.starts_with("beforeEach(")
                    || t.starts_with("beforeAll(")
                    || t.contains("mockReturnValue")
                    || t.contains("mockResolvedValue")
            })
            .count();
        let assertion_count: usize = tests.iter().map(|t| t.assertions.len()).sum();
        if setup_lines >= 5 && assertion_count <= tests.len() * 2 && tests.len() >= 2 {
            issues.push(Issue {
                rule: Rule::BoilerplatePadding,
                severity: Severity::Info,
                message: "Heavy setup with relatively few assertions (possible boilerplate)"
                    .to_string(),
                location: Location::new(1, 1),
                suggestion: Some(
                    "Consider simplifying setup or adding more meaningful assertions".to_string(),
                ),
                fix: None,
            });
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let ai_count = issues
            .iter()
            .filter(|i| {
                matches!(
                    i.rule,
                    Rule::TautologicalAssertion
                        | Rule::OverMocking
                        | Rule::ShallowVariety
                        | Rule::HappyPathOnly
                        | Rule::ParrotAssertion
                        | Rule::BoilerplatePadding
                )
            })
            .count();
        (25i32 - (ai_count as i32 * 4).min(25)).max(0) as u8
    }
}
