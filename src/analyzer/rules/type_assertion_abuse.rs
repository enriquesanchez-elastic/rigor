//! Type assertion abuse: overuse of type assertions (as Type) instead of real checks.

use super::AnalysisRule;
use crate::parser::{global_query_cache, is_inside_comment_range, QueryId, TypeScriptParser};
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::Tree;

const TYPE_ASSERTION_WARN_THRESHOLD: usize = 5;

/// Rule for detecting type assertion overuse in tests
pub struct TypeAssertionAbuseRule;

impl TypeAssertionAbuseRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TypeAssertionAbuseRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for TypeAssertionAbuseRule {
    fn name(&self) -> &'static str {
        "type-assertion-abuse"
    }

    fn analyze(&self, _tests: &[TestCase], source: &str, tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();
        let lang = TypeScriptParser::language();
        let cache = global_query_cache();

        let mut as_any_count = 0usize;

        if let Ok(matches) = cache.run_cached_query(source, tree, &lang, QueryId::AsTypeAssertion) {
            for caps in matches {
                let as_cap = match caps.iter().find(|c| c.name == "as_expr") {
                    Some(c) => c,
                    None => continue,
                };
                if is_inside_comment_range(as_cap.start_byte, as_cap.end_byte, source) {
                    continue;
                }
                let slice = source.get(as_cap.start_byte..as_cap.end_byte).unwrap_or("");
                if slice.contains(" as any") || slice.contains(" as unknown") {
                    as_any_count += 1;
                    if as_any_count == TYPE_ASSERTION_WARN_THRESHOLD {
                        let (line, col) = as_cap.start_point;
                        issues.push(Issue {
                            rule: Rule::TypeAssertionAbuse,
                            severity: Severity::Info,
                            message: "Multiple type assertions (as any / as unknown) - consider proper typing or runtime checks".to_string(),
                            location: Location::new(line, col),
                            suggestion: Some(
                                "Prefer type guards or runtime validation instead of casting".to_string(),
                            ),
                            fix: None,
                        });
                    }
                }
            }
        }

        for (i, line) in source.lines().enumerate() {
            if line.contains("@ts-ignore") || line.contains("@ts-expect-error") {
                let line_no = i + 1;
                if line.trim_start().starts_with("//") {
                    issues.push(Issue {
                        rule: Rule::TypeAssertionAbuse,
                        severity: Severity::Info,
                        message: "@ts-ignore or @ts-expect-error in test - consider fixing types"
                            .to_string(),
                        location: Location::new(line_no, 1),
                        suggestion: Some("Fix the type error or use a type guard".to_string()),
                        fix: None,
                    });
                }
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == Rule::TypeAssertionAbuse)
            .count();
        (25i32 - (n as i32 * 2).min(10)).max(0) as u8
    }
}
