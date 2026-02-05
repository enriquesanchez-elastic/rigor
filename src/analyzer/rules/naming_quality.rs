//! Test naming quality - vague or unhelpful test names

use super::AnalysisRule;
use crate::{Issue, Rule, Severity, TestCase};
use regex::Regex;
use tree_sitter::Tree;

/// Rule for detecting vague or poor test names
pub struct NamingQualityRule;

fn vague_patterns() -> Vec<Regex> {
    vec![
        Regex::new(r"^test\s*\d*$").unwrap(), // "test", "test1", "test 2"
        Regex::new(r"^it\s+works$").unwrap(), // "it works"
        Regex::new(r"^should\s+work$").unwrap(), // "should work"
        Regex::new(r"^handles?\s+\w+$").unwrap(), // "handles data", "handle input"
        Regex::new(r"^works\s*$").unwrap(),   // "works"
        Regex::new(r"^test\s+\d+$").unwrap(), // "test 1", "test 2"
        Regex::new(r"^test\w*\d+$").unwrap(), // "test1", "test2"
    ]
}

impl NamingQualityRule {
    pub fn new() -> Self {
        Self
    }

    fn is_vague(name: &str) -> bool {
        let name = name.trim();
        if name.len() < 4 {
            return true;
        }
        vague_patterns().iter().any(|re| re.is_match(name))
    }

    /// No verb: "user authentication" instead of "authenticates user"
    fn has_no_verb(name: &str) -> bool {
        let name_lower = name.to_lowercase();
        let verbs = [
            "should",
            "returns",
            "throws",
            "calls",
            "validates",
            "checks",
            "accepts",
            "rejects",
            "renders",
            "displays",
            "handles",
            "loads",
            "creates",
            "updates",
            "deletes",
            "fetches",
            "sends",
            "receives",
        ];
        !verbs.iter().any(|v| name_lower.contains(v))
            && name_lower.split_whitespace().count() <= 3
            && name.len() > 10
    }

    /// Copy-paste pattern: test1, test2, test3 or similar
    fn is_sequential_name(name: &str, all_names: &[String]) -> bool {
        let name_lower = name.to_lowercase();
        if !name_lower.contains("test") && !name_lower.contains("case") {
            return false;
        }
        let ends_with_digit = name_lower
            .chars()
            .last()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false);
        if !ends_with_digit {
            return false;
        }
        let similar = all_names
            .iter()
            .filter(|n| n.to_lowercase() != name_lower)
            .filter(|n| {
                let a = n.to_lowercase();
                let base_a = a.trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
                let base_b = name_lower
                    .trim_end_matches(|c: char| c.is_ascii_digit())
                    .to_string();
                base_a == base_b
            })
            .count();
        similar >= 1
    }
}

impl Default for NamingQualityRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for NamingQualityRule {
    fn name(&self) -> &'static str {
        "naming-quality"
    }

    fn analyze(&self, tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();
        let all_names: Vec<String> = tests.iter().map(|t| t.name.clone()).collect();

        for test in tests {
            if test.is_skipped {
                continue;
            }

            if Self::is_vague(&test.name) {
                issues.push(Issue {
                    rule: Rule::VagueTestName,
                    severity: Severity::Warning,
                    message: format!("Vague test name: '{}' - describe expected behavior", test.name),
                    location: test.location.clone(),
                    suggestion: Some(
                        "Use a name that describes the scenario and expected outcome, e.g. 'returns 404 when user not found'".to_string(),
                    ),
                });
            } else if Self::has_no_verb(&test.name) {
                issues.push(Issue {
                    rule: Rule::VagueTestName,
                    severity: Severity::Info,
                    message: format!(
                        "Test name '{}' may lack a clear verb - consider 'should ...' or 'returns ...'",
                        test.name
                    ),
                    location: test.location.clone(),
                    suggestion: Some("Start with should/returns/throws to describe behavior".to_string()),
                });
            } else if Self::is_sequential_name(&test.name, &all_names) {
                issues.push(Issue {
                    rule: Rule::VagueTestName,
                    severity: Severity::Warning,
                    message: format!(
                        "Sequential test name '{}' - use descriptive names instead of test1, test2",
                        test.name
                    ),
                    location: test.location.clone(),
                    suggestion: Some("Give each test a unique, descriptive name".to_string()),
                });
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let warnings = issues
            .iter()
            .filter(|i| i.rule == Rule::VagueTestName && i.severity == Severity::Warning)
            .count();
        let infos = issues
            .iter()
            .filter(|i| i.rule == Rule::VagueTestName && i.severity == Severity::Info)
            .count();
        let mut score: i32 = 25;
        score -= (warnings as i32 * 3).min(12);
        score -= (infos as i32).min(5);
        score.clamp(0, 25) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vague_detection() {
        assert!(NamingQualityRule::is_vague("test"));
        assert!(NamingQualityRule::is_vague("test1"));
        assert!(NamingQualityRule::is_vague("should work"));
        assert!(!NamingQualityRule::is_vague(
            "returns 404 when user not found"
        ));
    }

    #[test]
    fn test_sequential() {
        let names = vec![
            "test1".to_string(),
            "test2".to_string(),
            "test3".to_string(),
        ];
        assert!(NamingQualityRule::is_sequential_name("test1", &names));
    }
}
