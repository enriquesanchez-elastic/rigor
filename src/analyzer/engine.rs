//! Analysis engine - orchestrates all rules

use crate::config::{Config, RuleSeverity, SourceMappingMode};
use crate::detector::{FrameworkDetector, SourceMapper};
use crate::parser::{IgnoreDirectives, SourceFileParser, TestFileParser, TypeScriptParser};
use crate::{AnalysisResult, Issue, Score};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use super::rules::{
    AnalysisRule, AssertionIntentRule, AssertionQualityRule, AsyncPatternsRule,
    BehavioralCompletenessRule, BoundaryConditionsRule, BoundarySpecificityRule,
    CouplingAnalysisRule, DebugCodeRule, ErrorCoverageRule, FlakyPatternsRule, InputVarietyRule,
    MockAbuseRule, MutationResistantRule, NamingQualityRule, ReactTestingLibraryRule,
    ReturnPathCoverageRule, SideEffectVerificationRule, StateVerificationRule, TestIsolationRule,
    TrivialAssertionRule,
};
use super::ScoreCalculator;

/// Main analysis engine that orchestrates all rules
pub struct AnalysisEngine {
    /// Whether to include source file analysis
    analyze_source: bool,
    /// Project root for source mapping
    project_root: Option<PathBuf>,
}

impl AnalysisEngine {
    /// Create a new analysis engine
    pub fn new() -> Self {
        Self {
            analyze_source: true,
            project_root: None,
        }
    }

    /// Disable source file analysis
    pub fn without_source_analysis(mut self) -> Self {
        self.analyze_source = false;
        self
    }

    /// Set project root for source mapping
    pub fn with_project_root(mut self, root: PathBuf) -> Self {
        self.project_root = Some(root);
        self
    }

    /// Apply config to filter and adjust issue severity
    fn apply_config_to_issues(
        &self,
        issues: Vec<Issue>,
        config: Option<&Config>,
        test_path: &Path,
    ) -> Vec<Issue> {
        let Some(config) = config else {
            return issues;
        };

        // Get effective config for this file (with overrides applied)
        let effective = config.effective_for_file(test_path);

        let mut out = Vec::with_capacity(issues.len());
        for mut issue in issues {
            let rule_id = issue.rule.to_string();
            match effective.rules.get(&rule_id) {
                Some(RuleSeverity::Off) => continue,
                Some(rs) => {
                    if let Some(sev) = rs.to_severity() {
                        issue.severity = sev;
                    }
                    out.push(issue);
                }
                None => {
                    // Check base config
                    match config.rule_severity(&rule_id) {
                        Some(RuleSeverity::Off) => continue,
                        Some(rs) => {
                            if let Some(sev) = rs.to_severity() {
                                issue.severity = sev;
                            }
                            out.push(issue);
                        }
                        None => out.push(issue),
                    }
                }
            }
        }
        out
    }

    /// Analyze a test file and return the result
    pub fn analyze(&self, test_path: &Path, config: Option<&Config>) -> Result<AnalysisResult> {
        // Check if this is a test utility file (skip analysis or handle differently)
        if SourceMapper::is_test_utility(test_path) {
            // Still analyze but with reduced expectations
            // We don't skip entirely because the file might have real tests
        }

        // Read and parse the test file
        let source = fs::read_to_string(test_path)
            .with_context(|| format!("Failed to read test file: {}", test_path.display()))?;

        let mut parser = TypeScriptParser::for_file(test_path)?;
        let tree = parser
            .parse(&source)
            .with_context(|| format!("Failed to parse test file: {}", test_path.display()))?;

        // Extract test cases
        let test_parser = TestFileParser::new(&source);
        let tests = test_parser.extract_tests(&tree);
        let mut stats = test_parser.extract_stats(&tree);

        // Detect framework and test type
        let framework_detector = FrameworkDetector::new(&source);
        let framework = framework_detector.detect(&tree);
        let test_type = framework_detector.detect_test_type(test_path, framework);

        // Determine if source analysis should be skipped for this file
        let skip_source = if let Some(cfg) = config {
            let effective = cfg.effective_for_file(test_path);
            effective.skip_source_analysis || cfg.source_mapping.mode == SourceMappingMode::Off
        } else {
            false
        };

        // Find and parse source file (if enabled)
        let source_file = if self.analyze_source && !skip_source {
            // Create source mapper with config
            let mapper = if let Some(cfg) = config {
                let mut mapper = SourceMapper::with_config(cfg.source_mapping.clone());
                if let Some(ref root) = self.project_root {
                    mapper = mapper.with_project_root(root.clone());
                }
                mapper
            } else {
                let mut mapper = SourceMapper::new();
                if let Some(ref root) = self.project_root {
                    mapper = mapper.with_project_root(root.clone());
                }
                mapper
            };
            mapper.find_source_file(test_path)
        } else {
            None
        };

        let (source_content, source_tree) = if let Some(ref src_path) = source_file {
            if let Ok(content) = fs::read_to_string(src_path) {
                if let Ok(mut src_parser) = TypeScriptParser::for_file(src_path) {
                    if let Ok(tree) = src_parser.parse(&content) {
                        (Some(content), Some(tree))
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // Calculate function coverage if source is available
        if let (Some(ref src_content), Some(ref src_tree)) = (&source_content, &source_tree) {
            let source_parser = SourceFileParser::new(src_content);
            let coverage = source_parser.calculate_coverage(src_tree, &source);
            stats.function_coverage = Some(coverage);
        }

        // Run all rules
        let assertion_rule = AssertionQualityRule::new();
        let error_rule =
            if let (Some(content), Some(tree)) = (source_content.clone(), source_tree.clone()) {
                ErrorCoverageRule::new().with_source(content, tree)
            } else {
                ErrorCoverageRule::new()
            };
        let boundary_rule =
            if let (Some(content), Some(tree)) = (source_content.clone(), source_tree.clone()) {
                BoundaryConditionsRule::new().with_source(content, tree)
            } else {
                BoundaryConditionsRule::new()
            };
        let isolation_rule = TestIsolationRule::new();
        let variety_rule = InputVarietyRule::new();
        let debug_rule = DebugCodeRule::new();
        let flaky_rule = FlakyPatternsRule::new();
        let mock_rule = MockAbuseRule::new();
        let naming_rule = NamingQualityRule::new();
        let async_rule = AsyncPatternsRule::new();
        let rtl_rule = ReactTestingLibraryRule::new();
        let mutation_resistant_rule = MutationResistantRule::new();
        let boundary_specificity_rule = BoundarySpecificityRule::new();
        let state_verification_rule = StateVerificationRule::new();
        let assertion_intent_rule = AssertionIntentRule::new();
        let trivial_assertion_rule = TrivialAssertionRule::new();
        let return_path_rule =
            if let (Some(content), Some(tree)) = (source_content.clone(), source_tree.clone()) {
                ReturnPathCoverageRule::new().with_source(content, tree)
            } else {
                ReturnPathCoverageRule::new()
            };
        let behavioral_completeness_rule =
            if let (Some(content), Some(tree)) = (source_content.clone(), source_tree.clone()) {
                BehavioralCompletenessRule::new().with_source(content, tree)
            } else {
                BehavioralCompletenessRule::new()
            };
        let side_effect_rule =
            if let (Some(content), Some(tree)) = (source_content.clone(), source_tree.clone()) {
                SideEffectVerificationRule::new().with_source(content, tree)
            } else {
                SideEffectVerificationRule::new()
            };

        // Collect all issues
        let mut issues = Vec::new();
        issues.extend(assertion_rule.analyze(&tests, &source, &tree));
        issues.extend(error_rule.analyze(&tests, &source, &tree));
        issues.extend(boundary_rule.analyze(&tests, &source, &tree));
        issues.extend(isolation_rule.analyze(&tests, &source, &tree));
        issues.extend(variety_rule.analyze(&tests, &source, &tree));
        issues.extend(debug_rule.analyze(&tests, &source, &tree));
        issues.extend(flaky_rule.analyze(&tests, &source, &tree));
        issues.extend(mock_rule.analyze(&tests, &source, &tree));
        issues.extend(naming_rule.analyze(&tests, &source, &tree));
        issues.extend(async_rule.analyze(&tests, &source, &tree));
        issues.extend(rtl_rule.analyze(&tests, &source, &tree));
        issues.extend(mutation_resistant_rule.analyze(&tests, &source, &tree));
        issues.extend(boundary_specificity_rule.analyze(&tests, &source, &tree));
        issues.extend(state_verification_rule.analyze(&tests, &source, &tree));
        issues.extend(assertion_intent_rule.analyze(&tests, &source, &tree));
        issues.extend(trivial_assertion_rule.analyze(&tests, &source, &tree));
        issues.extend(return_path_rule.analyze(&tests, &source, &tree));
        issues.extend(behavioral_completeness_rule.analyze(&tests, &source, &tree));
        issues.extend(side_effect_rule.analyze(&tests, &source, &tree));

        // Run coupling analysis if we have function coverage data
        if let Some(ref fc) = stats.function_coverage {
            let coupling_rule =
                CouplingAnalysisRule::new().with_source_exports(fc.untested_exports.clone());
            issues.extend(coupling_rule.analyze(&tests, &source, &tree));
        }

        // Apply ignore comments: filter issues that have rigor-ignore on their line
        let ignore_directives = IgnoreDirectives::parse(&source);
        let issues: Vec<Issue> = issues
            .into_iter()
            .filter(|i| !ignore_directives.is_ignored(i.location.line, i.rule))
            .collect();

        // Apply config: filter rules set to "off", override severity
        let issues = self.apply_config_to_issues(issues, config, test_path);

        // Calculate scores (after filtering so breakdown reflects config)
        let breakdown = ScoreCalculator::calculate_breakdown(
            &tests,
            &issues,
            &assertion_rule,
            &error_rule,
            &boundary_rule,
            &isolation_rule,
            &variety_rule,
        );
        // Use weighted scoring based on test type for more accurate assessment
        let score = ScoreCalculator::calculate_weighted(&breakdown, test_type);
        // Apply issue-based penalty so problems (errors/warnings/info) lower the grade
        let score = ScoreCalculator::apply_issue_penalty(score, &issues);

        Ok(AnalysisResult {
            file_path: test_path.to_path_buf(),
            score,
            breakdown,
            issues,
            stats,
            framework,
            test_type,
            source_file,
        })
    }

    /// Analyze multiple test files sequentially
    pub fn analyze_many(
        &self,
        paths: &[&Path],
        config: Option<&Config>,
    ) -> Vec<Result<AnalysisResult>> {
        paths.iter().map(|p| self.analyze(p, config)).collect()
    }

    /// Analyze multiple test files in parallel using rayon
    pub fn analyze_parallel(
        &self,
        paths: &[PathBuf],
        config: Option<&Config>,
    ) -> Vec<Result<AnalysisResult>> {
        use rayon::prelude::*;

        paths.par_iter().map(|p| self.analyze(p, config)).collect()
    }

    /// Get aggregate stats from multiple results
    pub fn aggregate_stats(results: &[AnalysisResult]) -> AggregateStats {
        if results.is_empty() {
            return AggregateStats::default();
        }

        let total_score: u32 = results.iter().map(|r| r.score.value as u32).sum();
        let avg_score = (total_score / results.len() as u32) as u8;

        let total_tests: usize = results.iter().map(|r| r.stats.total_tests).sum();
        let total_issues: usize = results.iter().map(|r| r.issues.len()).sum();

        AggregateStats {
            files_analyzed: results.len(),
            average_score: Score::new(avg_score),
            total_tests,
            total_issues,
        }
    }
}

impl Default for AnalysisEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Aggregate statistics from multiple file analyses
#[derive(Debug, Default)]
pub struct AggregateStats {
    /// Number of files analyzed
    pub files_analyzed: usize,
    /// Average score across all files
    pub average_score: Score,
    /// Total number of tests across all files
    pub total_tests: usize,
    /// Total number of issues found
    pub total_issues: usize,
}

impl Default for Score {
    fn default() -> Self {
        Score::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_analyze_simple_file() {
        let mut file = NamedTempFile::with_suffix(".test.ts").unwrap();
        writeln!(
            file,
            r#"
            describe('example', () => {{
                it('should work', () => {{
                    expect(1).toBe(1);
                }});
            }});
        "#
        )
        .unwrap();

        let engine = AnalysisEngine::new().without_source_analysis();
        let result = engine.analyze(file.path(), None).unwrap();

        assert_eq!(result.stats.total_tests, 1);
        assert_eq!(result.stats.total_assertions, 1);
        assert!(result.score.value > 0);
        // Trivial assertion (expect(1).toBe(1)) and vague name should incur penalty — not A
        assert!(
            result.score.value < 90,
            "file with trivial assertion should not get A (got {})",
            result.score.value
        );
    }

    #[test]
    fn test_issue_penalty_lowers_grade() {
        let mut file = NamedTempFile::with_suffix(".test.ts").unwrap();
        writeln!(
            file,
            r#"
            describe('bad', () => {{
                it('test 1', () => {{
                    const x = 1;
                    expect(x).toBeDefined();
                    expect(x).toBeTruthy();
                }});
                it('test 2', () => {{
                    expect(1).toBe(1);
                }});
            }});
        "#
        )
        .unwrap();

        let engine = AnalysisEngine::new().without_source_analysis();
        let result = engine.analyze(file.path(), None).unwrap();

        assert_eq!(result.stats.total_tests, 2);
        assert!(
            !result.issues.is_empty(),
            "should report weak/trivial issues"
        );
        // Many issues (weak assertions, trivial, vague names) should pull score down to C or worse
        assert!(
            result.score.value < 80,
            "file with weak + trivial assertions and vague names should not get B or A (got {} = {})",
            result.score.value,
            result.score.grade
        );
    }
}
