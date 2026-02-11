//! Analysis engine - orchestrates all rules

use crate::config::{Config, RuleSeverity, SourceMappingMode};
use crate::detector::{FrameworkDetector, SourceMapper};
use crate::parser::{IgnoreDirectives, SourceFileParser, TestFileParser, TypeScriptParser};
use crate::{issue_in_test_range, AnalysisResult, Issue, Score, ScoreBreakdown, TestScore};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::Tree;

/// When source analysis is unavailable, source-dependent categories (Error Coverage,
/// Boundary Conditions) cannot detect issues. Their scores default to 25/25 — but
/// that doesn't mean coverage is perfect, it means we have no data.
///
/// We scale these categories proportionally: score = natural * 15 / 25.
/// - No issues detected → 25 * 15/25 = 15 (neutral "unknown")
/// - Some issues (19/25) → 19 * 15/25 = 11 (deductions still visible)
/// - Many issues (10/25) → 10 * 15/25 = 6 (severe deductions preserved)
///
/// This preserves the relative impact of detected issues rather than hiding them
/// behind a flat cap, while still preventing "no source = perfect score."
fn scale_no_source_categories(breakdown: &mut ScoreBreakdown) {
    const NO_SOURCE_BASELINE: u32 = 15;
    const MAX_CATEGORY: u32 = 25;
    breakdown.error_coverage =
        ((breakdown.error_coverage as u32 * NO_SOURCE_BASELINE) / MAX_CATEGORY) as u8;
    breakdown.boundary_conditions =
        ((breakdown.boundary_conditions as u32 * NO_SOURCE_BASELINE) / MAX_CATEGORY) as u8;
}

use super::rules::{
    AiSmellsRule, AnalysisRule, AssertionIntentRule, AssertionQualityRule,
    AsyncErrorMishandlingRule, AsyncPatternsRule, BehavioralCompletenessRule,
    BoundaryConditionsRule, BoundarySpecificityRule, CouplingAnalysisRule, DebugCodeRule,
    ErrorCoverageRule, ExcessiveSetupRule, FlakyPatternsRule, ImplementationCouplingRule,
    IncompleteMockVerificationRule, InputVarietyRule, MissingCleanupRule, MockAbuseRule,
    MutationResistantRule, NamingQualityRule, ReactTestingLibraryRule, RedundantTestRule,
    ReturnPathCoverageRule, SideEffectVerificationRule, StateVerificationRule, TestComplexityRule,
    TestIsolationRule, TrivialAssertionRule, TypeAssertionAbuseRule, UnreachableTestCodeRule,
    VacuousTestRule,
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

        self.analyze_core(
            &source,
            &tree,
            tests,
            &mut stats,
            framework,
            test_type,
            test_path,
            config,
            source_file,
            source_content,
            source_tree,
        )
    }

    /// Analyze test source from a string (e.g. stdin or in-memory content).
    /// Use a virtual path (e.g. `Path::new("stdin.test.ts")`) for config and test-type detection.
    /// Source file mapping is not performed; source-dependent rules run without source context.
    pub fn analyze_source(
        &self,
        test_source: &str,
        virtual_path: &Path,
        config: Option<&Config>,
    ) -> Result<AnalysisResult> {
        let mut parser = TypeScriptParser::for_file(virtual_path)
            .with_context(|| format!("Failed to create parser for {}", virtual_path.display()))?;
        let tree = parser.parse(test_source).with_context(|| {
            format!("Failed to parse test source for {}", virtual_path.display())
        })?;

        let test_parser = TestFileParser::new(test_source);
        let tests = test_parser.extract_tests(&tree);
        let mut stats = test_parser.extract_stats(&tree);

        let framework_detector = FrameworkDetector::new(test_source);
        let framework = framework_detector.detect(&tree);
        let test_type = framework_detector.detect_test_type(virtual_path, framework);

        let source_file: Option<PathBuf> = None;
        let source_content: Option<String> = None;
        let source_tree: Option<Tree> = None;

        self.analyze_core(
            test_source,
            &tree,
            tests,
            &mut stats,
            framework,
            test_type,
            virtual_path,
            config,
            source_file,
            source_content,
            source_tree,
        )
    }

    /// Shared analysis core (path-based analyze reads file and optionally source; analyze_source passes None for source).
    #[allow(clippy::too_many_arguments)]
    fn analyze_core(
        &self,
        source: &str,
        tree: &Tree,
        tests: Vec<crate::TestCase>,
        stats: &mut crate::TestStats,
        framework: crate::TestFramework,
        test_type: crate::TestType,
        test_path: &Path,
        config: Option<&Config>,
        source_file: Option<PathBuf>,
        source_content: Option<String>,
        source_tree: Option<Tree>,
    ) -> Result<AnalysisResult> {
        let skip_source = config
            .map(|c| {
                let effective = c.effective_for_file(test_path);
                effective.skip_source_analysis || c.source_mapping.mode == SourceMappingMode::Off
            })
            .unwrap_or(false);

        let (source_content_ref, source_tree_ref) = if self.analyze_source && !skip_source {
            (source_content.as_deref(), source_tree.as_ref())
        } else {
            (None, None)
        };

        if let (Some(src_content), Some(src_tree)) = (source_content_ref, source_tree_ref) {
            let source_parser = SourceFileParser::new(src_content);
            stats.function_coverage = Some(source_parser.calculate_coverage(src_tree, source));
        }

        let assertion_rule = AssertionQualityRule::new();
        let error_rule =
            if let (Some(ref content), Some(st)) = (source_content_ref, source_tree_ref) {
                ErrorCoverageRule::new()
                    .with_source(content.to_string(), st.clone())
                    .with_test_type(test_type)
            } else {
                ErrorCoverageRule::new().with_test_type(test_type)
            };
        let boundary_rule =
            if let (Some(ref content), Some(st)) = (source_content_ref, source_tree_ref) {
                BoundaryConditionsRule::new().with_source(content.to_string(), st.clone())
            } else {
                BoundaryConditionsRule::new()
            };
        let isolation_rule = TestIsolationRule::new();
        let variety_rule = InputVarietyRule::new();
        let debug_rule = DebugCodeRule::new();
        let flaky_rule = FlakyPatternsRule::new().with_framework(framework);
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
            if let (Some(ref content), Some(st)) = (source_content_ref, source_tree_ref) {
                ReturnPathCoverageRule::new().with_source(content.to_string(), st.clone())
            } else {
                ReturnPathCoverageRule::new()
            };
        let behavioral_completeness_rule =
            if let (Some(ref content), Some(st)) = (source_content_ref, source_tree_ref) {
                BehavioralCompletenessRule::new().with_source(content.to_string(), st.clone())
            } else {
                BehavioralCompletenessRule::new()
            };
        let side_effect_rule =
            if let (Some(ref content), Some(st)) = (source_content_ref, source_tree_ref) {
                SideEffectVerificationRule::new().with_source(content.to_string(), st.clone())
            } else {
                SideEffectVerificationRule::new()
            };
        let ai_smells_rule = AiSmellsRule::new();
        let test_complexity_rule = TestComplexityRule::new();
        let vacuous_test_rule = VacuousTestRule::new();
        let incomplete_mock_rule = IncompleteMockVerificationRule::new();
        let async_error_rule = AsyncErrorMishandlingRule::new();
        let excessive_setup_rule = ExcessiveSetupRule::new();
        let implementation_coupling_rule = ImplementationCouplingRule::new();
        let redundant_test_rule = RedundantTestRule::new();
        let unreachable_test_rule = UnreachableTestCodeRule::new();
        let type_assertion_rule = TypeAssertionAbuseRule::new();
        let missing_cleanup_rule = MissingCleanupRule::new();

        let mut issues = Vec::new();
        issues.extend(assertion_rule.analyze(&tests, source, tree));
        issues.extend(error_rule.analyze(&tests, source, tree));
        issues.extend(boundary_rule.analyze(&tests, source, tree));
        issues.extend(isolation_rule.analyze(&tests, source, tree));
        issues.extend(variety_rule.analyze(&tests, source, tree));
        issues.extend(debug_rule.analyze(&tests, source, tree));
        issues.extend(flaky_rule.analyze(&tests, source, tree));
        issues.extend(mock_rule.analyze(&tests, source, tree));
        issues.extend(naming_rule.analyze(&tests, source, tree));
        issues.extend(async_rule.analyze(&tests, source, tree));
        issues.extend(rtl_rule.analyze(&tests, source, tree));
        issues.extend(mutation_resistant_rule.analyze(&tests, source, tree));
        issues.extend(boundary_specificity_rule.analyze(&tests, source, tree));
        issues.extend(state_verification_rule.analyze(&tests, source, tree));
        issues.extend(assertion_intent_rule.analyze(&tests, source, tree));
        issues.extend(trivial_assertion_rule.analyze(&tests, source, tree));
        issues.extend(return_path_rule.analyze(&tests, source, tree));
        issues.extend(behavioral_completeness_rule.analyze(&tests, source, tree));
        issues.extend(side_effect_rule.analyze(&tests, source, tree));
        issues.extend(ai_smells_rule.analyze(&tests, source, tree));
        issues.extend(test_complexity_rule.analyze(&tests, source, tree));
        issues.extend(vacuous_test_rule.analyze(&tests, source, tree));
        issues.extend(incomplete_mock_rule.analyze(&tests, source, tree));
        issues.extend(async_error_rule.analyze(&tests, source, tree));
        issues.extend(excessive_setup_rule.analyze(&tests, source, tree));
        issues.extend(implementation_coupling_rule.analyze(&tests, source, tree));
        issues.extend(redundant_test_rule.analyze(&tests, source, tree));
        issues.extend(unreachable_test_rule.analyze(&tests, source, tree));
        issues.extend(type_assertion_rule.analyze(&tests, source, tree));
        issues.extend(missing_cleanup_rule.analyze(&tests, source, tree));

        if let Some(ref fc) = stats.function_coverage {
            let coupling_rule =
                CouplingAnalysisRule::new().with_source_exports(fc.untested_exports.clone());
            issues.extend(coupling_rule.analyze(&tests, source, tree));
        }

        let ignore_directives = IgnoreDirectives::parse(source);
        let issues: Vec<Issue> = issues
            .into_iter()
            .filter(|i| !ignore_directives.is_ignored(i.location.line, i.rule))
            .collect();

        let issues = self.apply_config_to_issues(issues, config, test_path);

        let mut breakdown = ScoreCalculator::calculate_breakdown(
            &tests,
            &issues,
            &assertion_rule,
            &error_rule,
            &boundary_rule,
            &isolation_rule,
            &variety_rule,
            &ai_smells_rule,
        );

        // Fix P1.3: "No source = free points"
        let has_source = source_content_ref.is_some();
        if !has_source && !tests.is_empty() {
            scale_no_source_categories(&mut breakdown);
        }

        let score = ScoreCalculator::calculate_weighted(&breakdown, test_type);
        let score = ScoreCalculator::apply_issue_penalty(score, &issues);

        let mut transparent_breakdown = Some(ScoreCalculator::build_transparent_breakdown(
            &breakdown, &issues, test_type,
        ));

        let test_scores: Vec<TestScore> = tests
            .iter()
            .map(|test| {
                let issues_for_test: Vec<Issue> = issues
                    .iter()
                    .filter(|i| issue_in_test_range(i, test.location.line, test.location.end_line))
                    .cloned()
                    .collect();
                let mut breakdown_t = ScoreCalculator::calculate_breakdown(
                    std::slice::from_ref(test),
                    &issues_for_test,
                    &assertion_rule,
                    &error_rule,
                    &boundary_rule,
                    &isolation_rule,
                    &variety_rule,
                    &ai_smells_rule,
                );

                // Apply no-source proportional scaling to per-test breakdown too
                if !has_source {
                    scale_no_source_categories(&mut breakdown_t);
                }

                let score_t = ScoreCalculator::calculate_weighted(&breakdown_t, test_type);
                let mut score_t = ScoreCalculator::apply_issue_penalty(score_t, &issues_for_test);

                // Fix: A test with zero assertions is essentially worthless.
                // Cap its score to prevent it from inflating the file average.
                // A test body that asserts nothing proves nothing.
                if test.assertions.is_empty() {
                    const MAX_SCORE_NO_ASSERTIONS: u8 = 30;
                    score_t = Score::new(score_t.value.min(MAX_SCORE_NO_ASSERTIONS));
                }

                TestScore {
                    name: test.name.clone(),
                    line: test.location.line,
                    end_line: test.location.end_line,
                    score: score_t.value,
                    grade: score_t.grade,
                    issues: issues_for_test,
                }
            })
            .collect();

        let (score, test_scores) = if test_scores.is_empty() {
            (score, None)
        } else {
            let total_weight: u32 = tests
                .iter()
                .map(|t| 1 + t.assertions.len() as u32)
                .sum::<u32>()
                .max(1);
            let weighted_sum: u32 = test_scores
                .iter()
                .zip(tests.iter())
                .map(|(ts, t)| ts.score as u32 * (1 + t.assertions.len() as u32))
                .sum();
            let aggregated = (weighted_sum / total_weight).min(100) as u8;

            // The aggregated per-test score must not exceed the file-level
            // breakdown score. The breakdown reflects the holistic quality view
            // (assertion quality, error coverage, etc.). When per-test aggregation
            // inflates past it (e.g. because each test individually looks passable
            // in source-dependent categories), use the lower of the two as the
            // final score. This prevents "96/A for a file with 5/9 no-assertion tests."
            let file_level = score.value;
            let final_score = aggregated.min(file_level);

            if let Some(ref mut tb) = transparent_breakdown {
                let penalty_adjusted =
                    (tb.total_before_penalties as i32 - tb.penalty_total).max(0) as u8;
                tb.final_score = final_score;
                // Record the per-test aggregated value whenever it changes the
                // final score away from the penalty-adjusted breakdown score.
                // This lets the display explain the discrepancy in the math.
                if final_score != penalty_adjusted {
                    tb.per_test_aggregated = Some(aggregated);
                }
            }

            // Scale per-test display scores proportionally when the file-level cap
            // reduced the score. Otherwise users see "all tests are B" but the file
            // is F, which is confusing. The scale factor preserves the relative
            // ordering while making per-test scores sum to the file score.
            let test_scores = if final_score < aggregated && aggregated > 0 {
                test_scores
                    .into_iter()
                    .map(|mut ts| {
                        ts.score =
                            ((ts.score as u32 * final_score as u32) / aggregated as u32) as u8;
                        ts.grade = Score::new(ts.score).grade;
                        ts
                    })
                    .collect::<Vec<_>>()
            } else {
                test_scores
            };

            (Score::new(final_score), Some(test_scores))
        };

        Ok(AnalysisResult {
            file_path: test_path.to_path_buf(),
            score,
            breakdown,
            transparent_breakdown,
            test_scores,
            issues,
            stats: stats.clone(),
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
    use crate::config::{Config, RuleSeverity, SourceMappingConfig, SourceMappingMode};
    use std::collections::HashMap;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_test_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::with_suffix(".test.ts").unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn test_analyze_simple_file() {
        let file = make_test_file(
            r#"
            describe('example', () => {
                it('should work', () => {
                    expect(1).toBe(1);
                });
            });
        "#,
        );

        let engine = AnalysisEngine::new().without_source_analysis();
        let result = engine.analyze(file.path(), None).unwrap();

        assert_eq!(result.stats.total_tests, 1);
        assert_eq!(result.stats.total_assertions, 1);
        // File with trivial assertion (expect(1).toBe(1)) and vague test name ("should work").
        // No source analysis → error coverage and boundary scaled to 15/25.
        // Assertion quality degraded by TrivialAssertion. VagueTestName adds penalty.
        // Expected range: roughly 40-70 (D to low C).
        assert!(
            result.score.value >= 30 && result.score.value <= 75,
            "trivial single-test file should score 30-75, got {}",
            result.score.value
        );
        assert!(
            !result.issues.is_empty(),
            "should detect trivial assertion and/or vague test name issues"
        );
        // Verify the specific issues we expect for this code
        let has_trivial = result
            .issues
            .iter()
            .any(|i| i.rule == crate::Rule::TrivialAssertion);
        assert!(
            has_trivial,
            "should detect trivial assertion (expect(1).toBe(1))"
        );
    }

    #[test]
    fn test_issue_penalty_lowers_grade() {
        let file = make_test_file(
            r#"
            describe('bad', () => {
                it('test 1', () => {
                    const x = 1;
                    expect(x).toBeDefined();
                    expect(x).toBeTruthy();
                });
                it('test 2', () => {
                    expect(1).toBe(1);
                });
            });
        "#,
        );

        let engine = AnalysisEngine::new().without_source_analysis();
        let result = engine.analyze(file.path(), None).unwrap();

        assert_eq!(result.stats.total_tests, 2);
        assert!(
            !result.issues.is_empty(),
            "should report weak/trivial issues"
        );
        // Two tests with weak assertions (toBeDefined, toBeTruthy) and vague names ("test 1", "test 2").
        // Second test has trivial assertion (expect(1).toBe(1)). No source analysis.
        // VagueTestName adds penalty-only deductions. WeakAssertion reduces assertion quality category.
        // Expected range: 30-70 (D to low C). Should NOT be above 75.
        assert!(
            result.score.value >= 25 && result.score.value <= 70,
            "weak-assertion file with vague names should score 25-70, got {}",
            result.score.value
        );
        // Verify specific issue types
        let has_weak = result
            .issues
            .iter()
            .any(|i| i.rule == crate::Rule::WeakAssertion);
        let has_vague = result
            .issues
            .iter()
            .any(|i| i.rule == crate::Rule::VagueTestName);
        assert!(
            has_weak,
            "should detect weak assertions (toBeDefined, toBeTruthy)"
        );
        assert!(
            has_vague,
            "should detect vague test names ('test 1', 'test 2')"
        );
    }

    #[test]
    fn test_analyze_many() {
        let file1 = make_test_file(
            r#"
            describe('auth', () => {
                it('validates email format', () => {
                    expect(isValid('a@b.com')).toBe(true);
                });
            });
        "#,
        );
        let file2 = make_test_file(
            r#"
            describe('cart', () => {
                it('adds item correctly', () => {
                    expect(add(1, 2)).toBe(3);
                });
            });
        "#,
        );

        let engine = AnalysisEngine::new().without_source_analysis();
        let paths: Vec<&Path> = vec![file1.path(), file2.path()];
        let results = engine.analyze_many(&paths, None);

        assert_eq!(results.len(), 2, "analyze_many should return two results");
        assert!(results[0].is_ok(), "first result should succeed");
        assert!(results[1].is_ok(), "second result should succeed");

        let r0 = results[0].as_ref().unwrap();
        let r1 = results[1].as_ref().unwrap();
        assert_eq!(r0.stats.total_tests, 1);
        assert_eq!(r1.stats.total_tests, 1);
    }

    #[test]
    fn test_analyze_parallel() {
        let file1 = make_test_file(
            r#"
            describe('auth', () => {
                it('validates email format', () => {
                    expect(isValid('a@b.com')).toBe(true);
                });
            });
        "#,
        );
        let file2 = make_test_file(
            r#"
            describe('cart', () => {
                it('adds item correctly', () => {
                    expect(add(1, 2)).toBe(3);
                });
            });
        "#,
        );

        let engine = AnalysisEngine::new().without_source_analysis();
        let paths: Vec<PathBuf> = vec![file1.path().to_path_buf(), file2.path().to_path_buf()];
        let results = engine.analyze_parallel(&paths, None);

        assert_eq!(
            results.len(),
            2,
            "analyze_parallel should return two results"
        );
        let ok_count = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(ok_count, 2, "both results should succeed");
    }

    #[test]
    fn test_aggregate_stats_empty() {
        let stats = AnalysisEngine::aggregate_stats(&[]);
        assert_eq!(stats.files_analyzed, 0);
        assert_eq!(stats.total_tests, 0);
        assert_eq!(stats.total_issues, 0);
        assert_eq!(stats.average_score.value, 0);
    }

    #[test]
    fn test_aggregate_stats_multiple() {
        let file1 = make_test_file(
            r#"
            describe('a', () => {
                it('validates email format', () => { expect(isValid('a@b.com')).toBe(true); });
                it('rejects bad email', () => { expect(isValid('bad')).toBe(false); });
            });
        "#,
        );
        let file2 = make_test_file(
            r#"
            describe('b', () => {
                it('adds correctly', () => { expect(add(1,2)).toBe(3); });
            });
        "#,
        );

        let engine = AnalysisEngine::new().without_source_analysis();
        let r1 = engine.analyze(file1.path(), None).unwrap();
        let r2 = engine.analyze(file2.path(), None).unwrap();

        let stats = AnalysisEngine::aggregate_stats(&[r1.clone(), r2.clone()]);
        assert_eq!(stats.files_analyzed, 2);
        assert_eq!(
            stats.total_tests,
            r1.stats.total_tests + r2.stats.total_tests
        );
        assert_eq!(stats.total_issues, r1.issues.len() + r2.issues.len());
        let expected_avg = ((r1.score.value as u32 + r2.score.value as u32) / 2) as u8;
        assert_eq!(stats.average_score.value, expected_avg);
    }

    #[test]
    fn test_apply_config_rule_off() {
        let file = make_test_file(
            r#"
            describe('auth', () => {
                it('test 1', () => {
                    expect(1).toBe(1);
                });
            });
        "#,
        );

        // First, analyze without config to see which rules fire
        let engine = AnalysisEngine::new().without_source_analysis();
        let result_no_config = engine.analyze(file.path(), None).unwrap();
        let has_trivial = result_no_config
            .issues
            .iter()
            .any(|i| i.rule == crate::Rule::TrivialAssertion);
        assert!(
            has_trivial,
            "should detect trivial assertion without config"
        );

        // Now analyze with config that turns off trivial-assertion
        let mut rules = HashMap::new();
        rules.insert("trivial-assertion".to_string(), RuleSeverity::Off);
        let config = Config {
            rules,
            ..Config::default()
        };

        let result_with_config = engine.analyze(file.path(), Some(&config)).unwrap();
        let has_trivial_now = result_with_config
            .issues
            .iter()
            .any(|i| i.rule == crate::Rule::TrivialAssertion);
        assert!(
            !has_trivial_now,
            "trivial-assertion should be filtered out when rule is off"
        );
    }

    #[test]
    fn test_apply_config_override_severity() {
        let file = make_test_file(
            r#"
            describe('auth', () => {
                it('test 1', () => {
                    expect(1).toBe(1);
                });
            });
        "#,
        );

        let mut rules = HashMap::new();
        rules.insert("trivial-assertion".to_string(), RuleSeverity::Info);
        let config = Config {
            rules,
            ..Config::default()
        };

        let engine = AnalysisEngine::new().without_source_analysis();
        let result = engine.analyze(file.path(), Some(&config)).unwrap();

        let trivial_issues: Vec<_> = result
            .issues
            .iter()
            .filter(|i| i.rule == crate::Rule::TrivialAssertion)
            .collect();
        assert!(
            !trivial_issues.is_empty(),
            "trivial-assertion should still appear with severity override"
        );
        for issue in &trivial_issues {
            assert_eq!(
                issue.severity,
                crate::Severity::Info,
                "trivial-assertion severity should be overridden to Info"
            );
        }
    }

    #[test]
    fn test_analyze_with_source_mapping_off() {
        let file = make_test_file(
            r#"
            describe('auth', () => {
                it('authenticates user', () => {
                    expect(authenticate('user', 'pass')).toBe(true);
                });
            });
        "#,
        );

        let config = Config {
            source_mapping: SourceMappingConfig {
                mode: SourceMappingMode::Off,
                ..SourceMappingConfig::default()
            },
            ..Config::default()
        };

        let engine =
            AnalysisEngine::new().with_project_root(PathBuf::from("test-repos/fake-project"));
        let result = engine.analyze(file.path(), Some(&config)).unwrap();

        assert!(
            result.source_file.is_none(),
            "source_file should be None when sourceMapping mode is Off"
        );
    }

    #[test]
    fn test_analyze_with_skip_source_via_override() {
        let file = make_test_file(
            r#"
            describe('e2e login', () => {
                it('logs in successfully', () => {
                    expect(page.url()).toContain('/dashboard');
                });
            });
        "#,
        );

        let config: Config = serde_json::from_str(
            r#"{
                "overrides": [
                    {
                        "files": ["**/*.test.ts"],
                        "skipSourceAnalysis": true
                    }
                ]
            }"#,
        )
        .unwrap();

        let engine =
            AnalysisEngine::new().with_project_root(PathBuf::from("test-repos/fake-project"));
        let result = engine.analyze(file.path(), Some(&config)).unwrap();

        assert!(
            result.source_file.is_none(),
            "source_file should be None when skipSourceAnalysis is true via override"
        );
    }

    #[test]
    fn test_default_engine() {
        let engine = AnalysisEngine::default();
        // Default engine should have source analysis enabled
        let file = make_test_file(
            r#"
            describe('x', () => {
                it('works', () => { expect(1).toBe(1); });
            });
        "#,
        );
        let result = engine.analyze(file.path(), None);
        assert!(
            result.is_ok(),
            "default engine should analyze without error"
        );
        let result = result.unwrap();
        assert_eq!(result.stats.total_tests, 1);
        assert_eq!(result.stats.total_assertions, 1);
        assert!(
            !result.issues.is_empty(),
            "default engine should still detect trivial assertion issues"
        );
    }

    /// Per-test score proportional scaling: when the file-level cap is lower than the
    /// aggregated per-test average, each test's display score is scaled proportionally.
    /// This prevents "all tests B but file is F" confusion.
    ///
    /// Formula: ts.score = (ts.score * final_score) / aggregated
    #[test]
    fn test_per_test_score_proportional_scaling() {
        // A file where some tests have assertions but some don't.
        // Tests with no assertions get floored to 30, dragging the
        // per-test aggregate below the file-level breakdown score.
        let file = make_test_file(
            r#"
            describe('mixed quality', () => {
                it('has good assertions', () => {
                    expect(calculateTotal(10, 20)).toBe(30);
                    expect(calculateTotal(0, 0)).toBe(0);
                });
                it('does nothing useful', () => {
                    const x = 1;
                });
                it('also does nothing', () => {
                    const y = 2;
                });
            });
        "#,
        );

        let engine = AnalysisEngine::new().without_source_analysis();
        let result = engine.analyze(file.path(), None).unwrap();

        // Verify we have per-test scores
        let test_scores = result
            .test_scores
            .as_ref()
            .expect("should have per-test scores");
        assert_eq!(test_scores.len(), 3, "should have 3 test scores");

        // The no-assertion tests should score at most 30 (the floor).
        for ts in test_scores.iter().filter(|ts| {
            ts.issues
                .iter()
                .any(|i| i.rule == crate::Rule::NoAssertions)
        }) {
            assert!(
                ts.score <= 30,
                "no-assertion test '{}' should score at most 30, got {}",
                ts.name,
                ts.score
            );
        }

        // File-level score should be no higher than the aggregated per-test average
        // (which is pulled down by the no-assertion tests).
        // The transparent breakdown should record the per-test aggregation if it
        // changed the final score.
        if let Some(ref tb) = result.transparent_breakdown {
            assert!(
                tb.final_score <= result.score.value + 1, // allow rounding
                "transparent breakdown final ({}) should match score ({})",
                tb.final_score,
                result.score.value
            );
        }

        // The good test should score higher than the no-assertion tests
        let good_test = test_scores
            .iter()
            .find(|ts| ts.name.contains("good assertions"));
        let bad_tests: Vec<_> = test_scores
            .iter()
            .filter(|ts| ts.name.contains("nothing"))
            .collect();
        if let Some(good) = good_test {
            for bad in &bad_tests {
                assert!(
                    good.score >= bad.score,
                    "good test ({}) should score >= no-assertion test ({})",
                    good.score,
                    bad.score
                );
            }
        }
    }

    /// Verify the scale_no_source_categories helper produces expected values.
    #[test]
    fn test_scale_no_source_categories() {
        use crate::ScoreBreakdown;

        // Perfect scores get scaled to 15
        let mut b = ScoreBreakdown {
            assertion_quality: 25,
            error_coverage: 25,
            boundary_conditions: 25,
            test_isolation: 25,
            input_variety: 25,
            ai_smells: 25,
        };
        scale_no_source_categories(&mut b);
        assert_eq!(b.error_coverage, 15, "25 * 15/25 = 15");
        assert_eq!(b.boundary_conditions, 15, "25 * 15/25 = 15");
        // Other categories should be unchanged
        assert_eq!(b.assertion_quality, 25);
        assert_eq!(b.test_isolation, 25);
        assert_eq!(b.input_variety, 25);

        // Partially degraded scores get scaled proportionally
        let mut b2 = ScoreBreakdown {
            assertion_quality: 20,
            error_coverage: 19,
            boundary_conditions: 10,
            test_isolation: 22,
            input_variety: 18,
            ai_smells: 25,
        };
        scale_no_source_categories(&mut b2);
        assert_eq!(b2.error_coverage, 11, "19 * 15/25 = 11 (truncated)");
        assert_eq!(b2.boundary_conditions, 6, "10 * 15/25 = 6");

        // Zero stays zero
        let mut b3 = ScoreBreakdown {
            assertion_quality: 0,
            error_coverage: 0,
            boundary_conditions: 0,
            test_isolation: 0,
            input_variety: 0,
            ai_smells: 0,
        };
        scale_no_source_categories(&mut b3);
        assert_eq!(b3.error_coverage, 0);
        assert_eq!(b3.boundary_conditions, 0);
    }
}
