//! Parse rigor-ignore comment directives from source

use crate::Rule;
use std::collections::{HashMap, HashSet};

/// Directive for which rules are ignored on a given line
#[derive(Debug, Clone)]
pub enum LineIgnoreSet {
    /// Ignore all rules on this line
    All,
    /// Ignore only these rules
    Rules(HashSet<Rule>),
}

/// Parsed ignore directives from a file
#[derive(Debug, Default)]
pub struct IgnoreDirectives {
    /// Per-line: which rules to ignore (line is 1-indexed)
    line_rules: HashMap<usize, LineIgnoreSet>,
    /// Ranges (start_line, end_line) where all rules are disabled (1-indexed, inclusive)
    disabled_ranges: Vec<(usize, usize)>,
}

impl IgnoreDirectives {
    /// Check if an issue at the given location and rule should be ignored
    pub fn is_ignored(&self, line: usize, rule: Rule) -> bool {
        // Check per-line directive
        if let Some(set) = self.line_rules.get(&line) {
            return match set {
                LineIgnoreSet::All => true,
                LineIgnoreSet::Rules(r) => r.contains(&rule),
            };
        }
        // Check disabled ranges
        for (start, end) in &self.disabled_ranges {
            if line >= *start && line <= *end {
                return true;
            }
        }
        false
    }

    /// Parse source code for rigor-ignore comments
    pub fn parse(source: &str) -> Self {
        let mut line_rules: HashMap<usize, LineIgnoreSet> = HashMap::new();
        let mut disabled_ranges: Vec<(usize, usize)> = Vec::new();
        let mut disable_start: Option<usize> = None;

        for (zero_indexed, line) in source.lines().enumerate() {
            let line_no = zero_indexed + 1;
            let line = line.trim();

            // Single-line: // rigor-ignore-next-line
            if line.contains("rigor-ignore-next-line") {
                line_rules.insert(line_no + 1, LineIgnoreSet::All);
            }

            // Single-line: ... rigor-ignore [rule-id] ... (anywhere in line)
            if line.contains("rigor-ignore") && !line.contains("rigor-ignore-next-line") {
                if let Some(idx) = line.find("rigor-ignore") {
                    let rest = line[idx..].strip_prefix("rigor-ignore").unwrap_or("").trim_start();
                    let rest = rest.trim_end_matches('*').trim_end().trim_end_matches('/');
                    let rules = parse_rule_list(rest);
                    if rules.is_empty() {
                        line_rules.insert(line_no, LineIgnoreSet::All);
                    } else {
                        line_rules.insert(line_no, LineIgnoreSet::Rules(rules));
                    }
                }
            }

            // Block: /* rigor-disable */ ... /* rigor-enable */
            if line.contains("rigor-disable") {
                disable_start = Some(line_no);
            }
            if line.contains("rigor-enable") {
                if let Some(start) = disable_start.take() {
                    disabled_ranges.push((start, line_no));
                }
            }
        }

        // If we never saw rigor-enable, ignore from disable to end of file
        if let Some(start) = disable_start {
            let end = source.lines().count().max(1);
            disabled_ranges.push((start, end));
        }

        Self {
            line_rules,
            disabled_ranges,
        }
    }
}

fn parse_rule_list(s: &str) -> HashSet<Rule> {
    let mut set = HashSet::new();
    for word in s.split_whitespace() {
        if let Some(rule) = rule_from_id(word.trim_matches(',')) {
            set.insert(rule);
        }
    }
    set
}

fn rule_from_id(id: &str) -> Option<Rule> {
    match id {
        "weak-assertion" => Some(Rule::WeakAssertion),
        "missing-error-test" => Some(Rule::MissingErrorTest),
        "missing-boundary-test" => Some(Rule::MissingBoundaryTest),
        "shared-state" => Some(Rule::SharedState),
        "hardcoded-values" => Some(Rule::HardcodedValues),
        "no-assertions" => Some(Rule::NoAssertions),
        "skipped-test" => Some(Rule::SkippedTest),
        "empty-test" => Some(Rule::EmptyTest),
        "duplicate-test" => Some(Rule::DuplicateTest),
        "limited-input-variety" => Some(Rule::LimitedInputVariety),
        "debug-code" => Some(Rule::DebugCode),
        "focused-test" => Some(Rule::FocusedTest),
        "flaky-pattern" => Some(Rule::FlakyPattern),
        "mock-abuse" => Some(Rule::MockAbuse),
        "snapshot-overuse" => Some(Rule::SnapshotOveruse),
        "vague-test-name" => Some(Rule::VagueTestName),
        "missing-await" => Some(Rule::MissingAwait),
        "rtl-prefer-screen" => Some(Rule::RtlPreferScreen),
        "rtl-prefer-semantic" => Some(Rule::RtlPreferSemantic),
        "rtl-prefer-user-event" => Some(Rule::RtlPreferUserEvent),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ignore_next_line() {
        let source = r#"
        // rigor-ignore-next-line
        expect(result).toBeDefined();
        "#;
        let dir = IgnoreDirectives::parse(source);
        // Directive on line 2 applies to the *next* line (3)
        assert!(dir.is_ignored(3, Rule::WeakAssertion));
        assert!(!dir.is_ignored(1, Rule::WeakAssertion));
        assert!(!dir.is_ignored(2, Rule::WeakAssertion));
    }

    #[test]
    fn test_ignore_specific_rule() {
        let source = "  // rigor-ignore weak-assertion\n  expect(x).toBeDefined();";
        let dir = IgnoreDirectives::parse(source);
        assert!(dir.is_ignored(1, Rule::WeakAssertion));
        assert!(!dir.is_ignored(1, Rule::NoAssertions));
    }

    #[test]
    fn test_disable_block() {
        let source = r#"
        /* rigor-disable */
        expect(a).toBeDefined();
        expect(b).toBeTruthy();
        /* rigor-enable */
        expect(c).toBe(1);
        "#;
        let dir = IgnoreDirectives::parse(source);
        // Disabled range is inclusive [2, 5]: lines 2–5 are ignored
        assert!(dir.is_ignored(2, Rule::WeakAssertion));
        assert!(dir.is_ignored(3, Rule::WeakAssertion));
        assert!(dir.is_ignored(4, Rule::WeakAssertion));
        assert!(dir.is_ignored(5, Rule::WeakAssertion));
        assert!(!dir.is_ignored(6, Rule::WeakAssertion));
    }
}
