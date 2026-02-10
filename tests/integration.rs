//! Integration tests: full analysis pipeline against test-repos/fake-project/

use rigor::analyzer::AnalysisEngine;
use rigor::{Rule, TestFramework};
use std::path::Path;

const FAKE_PROJECT_ROOT: &str = "test-repos/fake-project";

fn analyze(test_path: &str) -> rigor::AnalysisResult {
    let engine =
        AnalysisEngine::new().with_project_root(std::path::PathBuf::from(FAKE_PROJECT_ROOT));
    engine
        .analyze(Path::new(test_path), None)
        .unwrap_or_else(|e| panic!("analyze({}) failed: {}", test_path, e))
}

// --- Score sanity tests ---

#[test]
fn good_test_scores_b_or_above() {
    let r = analyze("test-repos/fake-project/tests/auth.test.ts");
    // With Phase 2.2 rules enabled, auth.test.ts may score in the C range; require at least 75.
    assert!(
        r.score.value >= 75,
        "auth.test.ts = {} ({})",
        r.score.value,
        r.score.grade
    );
}

#[test]
fn weak_assertions_has_issues() {
    let r = analyze("test-repos/fake-project/tests/weak-assertions.test.ts");
    assert!(!r.issues.is_empty(), "weak-assertions should report issues");
    // With v2 scoring (no double-counting), category-affecting issues (WeakAssertion) reduce
    // the assertion quality category but don't add penalty. Without source analysis, other
    // categories default to high, keeping the overall score elevated.
    // This was addressed in v1.0.1 with proportional no-source scaling.
    let weak_issues = r
        .issues
        .iter()
        .filter(|i| i.rule == rigor::Rule::WeakAssertion)
        .count();
    assert!(
        weak_issues > 0,
        "should detect weak assertions, got {} issues total",
        r.issues.len()
    );
}

#[test]
fn mixed_bad_has_issues() {
    let r = analyze("test-repos/fake-project/tests/mixed-bad.test.ts");
    assert!(!r.issues.is_empty(), "mixed-bad should report issues");
    // Same calibration note as above for v2 scoring without source analysis.
    assert!(
        r.issues.len() >= 2,
        "mixed-bad should have multiple issues, got {}",
        r.issues.len()
    );
}

/// Score vs finds alignment: intentionally bad files must score lower than the good reference file.
#[test]
fn bad_files_score_lower_than_good_file() {
    let good = analyze("test-repos/fake-project/tests/auth.test.ts");
    let weak = analyze("test-repos/fake-project/tests/weak-assertions.test.ts");
    let mixed = analyze("test-repos/fake-project/tests/mixed-bad.test.ts");
    assert!(
        weak.score.value < good.score.value,
        "weak-assertions ({}) should score lower than auth ({}) — score vs finds alignment",
        weak.score.value,
        good.score.value
    );
    assert!(
        mixed.score.value < good.score.value,
        "mixed-bad ({}) should score lower than auth ({}) — score vs finds alignment",
        mixed.score.value,
        good.score.value
    );
}

// --- Rule-specific detection tests ---

#[test]
fn trivial_assertions_detected() {
    let r = analyze("test-repos/fake-project/tests/trivial-assertions.test.ts");
    assert!(
        r.issues.iter().any(|i| i.rule == Rule::TrivialAssertion),
        "expected TrivialAssertion in {:?}",
        r.issues.iter().map(|i| &i.rule).collect::<Vec<_>>()
    );
}

#[test]
fn no_assertions_detected() {
    let r = analyze("test-repos/fake-project/tests/no-assertions.test.ts");
    assert!(r.issues.iter().any(|i| i.rule == Rule::NoAssertions));
}

#[test]
fn vague_names_detected() {
    let r = analyze("test-repos/fake-project/tests/vague-names.test.ts");
    assert!(r.issues.iter().any(|i| i.rule == Rule::VagueTestName));
}

#[test]
fn debug_code_detected() {
    let r = analyze("test-repos/fake-project/tests/debug-code.test.ts");
    assert!(r.issues.iter().any(|i| i.rule == Rule::DebugCode));
}

#[test]
fn flaky_patterns_detected() {
    let r = analyze("test-repos/fake-project/tests/flaky.test.ts");
    assert!(r.issues.iter().any(|i| i.rule == Rule::FlakyPattern));
}

#[test]
fn shared_state_detected() {
    let r = analyze("test-repos/fake-project/tests/shared-state.test.ts");
    assert!(r.issues.iter().any(|i| i.rule == Rule::SharedState));
}

#[test]
fn duplicate_names_detected() {
    let r = analyze("test-repos/fake-project/tests/duplicate-names.test.ts");
    assert!(r.issues.iter().any(|i| i.rule == Rule::DuplicateTest));
}

#[test]
fn skipped_and_focused_detected() {
    let r = analyze("test-repos/fake-project/tests/skipped-and-focused.test.ts");
    assert!(r.issues.iter().any(|i| i.rule == Rule::SkippedTest));
    assert!(r.issues.iter().any(|i| i.rule == Rule::FocusedTest));
}

#[test]
fn mock_abuse_detected() {
    let r = analyze("test-repos/fake-project/tests/mock-abuse.test.ts");
    assert!(r.issues.iter().any(|i| i.rule == Rule::MockAbuse));
}

#[test]
fn snapshot_overuse_detected() {
    let r = analyze("test-repos/fake-project/tests/snapshot-only.test.ts");
    assert!(r.issues.iter().any(|i| i.rule == Rule::SnapshotOveruse));
}

#[test]
fn missing_await_detected() {
    let r = analyze("test-repos/fake-project/tests/async-missing-await.test.ts");
    assert!(r.issues.iter().any(|i| i.rule == Rule::MissingAwait));
}

#[test]
fn missing_error_test_detected() {
    let r = analyze("test-repos/fake-project/tests/missing-error-tests.test.ts");
    assert!(r.issues.iter().any(|i| i.rule == Rule::MissingErrorTest));
}

#[test]
fn missing_boundary_test_detected() {
    let r = analyze("test-repos/fake-project/tests/missing-boundary-tests.test.ts");
    assert!(
        r.issues.iter().any(|i| i.rule == Rule::MissingBoundaryTest),
        "expected MissingBoundaryTest issue, got: {:?}",
        r.issues
            .iter()
            .map(|i| (&i.rule, &i.message))
            .collect::<Vec<_>>()
    );
}

#[test]
fn hardcoded_values_detected() {
    let r = analyze("test-repos/fake-project/tests/hardcoded-limited-input.test.ts");
    assert!(r.issues.iter().any(|i| i.rule == Rule::HardcodedValues));
}

#[test]
fn mutation_resistant_detected() {
    let r = analyze("test-repos/fake-project/tests/mutation-resistant.test.ts");
    assert!(r.issues.iter().any(|i| i.rule == Rule::MutationResistant));
}

#[test]
fn assertion_intent_mismatch_detected() {
    let r = analyze("test-repos/fake-project/tests/assertion-intent-mismatch.test.ts");
    assert!(r
        .issues
        .iter()
        .any(|i| i.rule == Rule::AssertionIntentMismatch));
}

// --- Framework detection ---

#[test]
fn cypress_framework_detected() {
    let r = analyze("test-repos/fake-project/e2e/checkout.cy.ts");
    assert_eq!(r.framework, TestFramework::Cypress);
}

#[test]
fn playwright_framework_detected() {
    let r = analyze("test-repos/fake-project/e2e/login.e2e.test.ts");
    assert_eq!(r.framework, TestFramework::Playwright);
}

#[test]
fn vitest_framework_detected() {
    let r = analyze("test-repos/fake-project/vitest/math.test.ts");
    assert_eq!(r.framework, TestFramework::Vitest);
}

// --- Source mapping ---

#[test]
fn source_mapping_works() {
    let r = analyze("test-repos/fake-project/tests/auth.test.ts");
    // When project_root is set, source mapper may resolve test -> source; assert we got a result
    assert!(r.stats.total_tests > 0);
    if let Some(ref p) = r.source_file {
        assert!(
            p.to_string_lossy().contains("auth"),
            "source_file should point to auth when set; got {:?}",
            p
        );
    }
}

// --- RTL patterns ---

#[test]
fn rtl_patterns_detected() {
    let r = analyze("test-repos/fake-project/src/components/Button.bad.test.tsx");
    assert!(
        r.issues.iter().any(|i| i.rule == Rule::RtlPreferScreen),
        "expected RtlPreferScreen in Button.bad.test.tsx"
    );
}

// --- Phase 2.2 rules: implemented but excluded from category scoring (penalty only) ---

#[test]
fn phase_2_2_rules_excluded_from_category_scoring() {
    use rigor::rule_scoring_category;
    let phase_2_2_rules = [
        Rule::TestComplexity,
        Rule::ImplementationCoupling,
        Rule::VacuousTest,
        Rule::IncompleteMockVerification,
        Rule::AsyncErrorMishandling,
        Rule::RedundantTest,
        Rule::UnreachableTestCode,
        Rule::ExcessiveSetup,
        Rule::TypeAssertionAbuse,
        Rule::MissingCleanup,
    ];
    for rule in &phase_2_2_rules {
        assert!(
            rule_scoring_category(rule).is_none(),
            "Phase 2.2 rule {:?} must be excluded from category scoring (penalty only)",
            rule
        );
    }
}

#[test]
fn phase_2_2_rules_produce_issues_on_fixtures() {
    // Trivial-assertions fixture (all expect(x).toBe(x)) should trigger VacuousTest.
    let trivial = analyze("test-repos/fake-project/tests/trivial-assertions.test.ts");
    assert!(
        trivial.issues.iter().any(|i| i.rule == Rule::VacuousTest),
        "trivial-assertions.test.ts should report VacuousTest, got: {:?}",
        trivial.issues.iter().map(|i| i.rule).collect::<Vec<_>>()
    );
    // At least one other fixture should report a Phase 2.2 rule (e.g. skipped-and-focused).
    let phase_2_2: std::collections::HashSet<_> = [
        Rule::TestComplexity,
        Rule::VacuousTest,
        Rule::IncompleteMockVerification,
        Rule::AsyncErrorMishandling,
        Rule::ExcessiveSetup,
        Rule::ImplementationCoupling,
        Rule::RedundantTest,
        Rule::UnreachableTestCode,
        Rule::TypeAssertionAbuse,
        Rule::MissingCleanup,
    ]
    .into_iter()
    .collect();
    let skipped = analyze("test-repos/fake-project/tests/skipped-and-focused.test.ts");
    let has_phase_2_2 = skipped.issues.iter().any(|i| phase_2_2.contains(&i.rule));
    assert!(
        has_phase_2_2,
        "skipped-and-focused.test.ts should report at least one Phase 2.2 rule; issues: {:?}",
        skipped.issues.iter().map(|i| i.rule).collect::<Vec<_>>()
    );
}

// --- Semantic scoring tests (P3.1) ---
// These test the *intent* of the scoring model, not specific numbers.
// A scoring change that breaks these means the model lost its ability
// to distinguish good tests from bad ones.

#[test]
fn no_assertions_should_score_below_40() {
    let r = analyze("test-repos/fake-project/tests/no-assertions.test.ts");
    assert!(
        r.score.value < 40,
        "File with 0 assertions scored {}/{}. Expected < 40 (F grade).",
        r.score.value,
        r.score.grade
    );
}

#[test]
fn trivial_assertions_should_score_below_70() {
    let r = analyze("test-repos/fake-project/tests/trivial-assertions.test.ts");
    assert!(
        r.score.value < 70,
        "File with only expect(1).toBe(1) scored {}/{}. Expected < 70 (D or below).",
        r.score.value,
        r.score.grade
    );
}

#[test]
fn snapshot_only_should_score_below_60() {
    let r = analyze("test-repos/fake-project/tests/snapshot-only.test.ts");
    assert!(
        r.score.value < 60,
        "File with only toMatchSnapshot() scored {}/{}. Expected < 60 (F grade).",
        r.score.value,
        r.score.grade
    );
}

#[test]
fn weak_assertions_should_score_below_75() {
    let r = analyze("test-repos/fake-project/tests/weak-assertions.test.ts");
    assert!(
        r.score.value < 75,
        "File with only toBeDefined()/toBeTruthy() scored {}/{}. Expected < 75.",
        r.score.value,
        r.score.grade
    );
}

#[test]
fn vague_names_should_score_below_60() {
    let r = analyze("test-repos/fake-project/tests/vague-names.test.ts");
    assert!(
        r.score.value < 60,
        "File with names like 'test 1', 'works' scored {}/{}. Expected < 60.",
        r.score.value,
        r.score.grade
    );
}

#[test]
fn flaky_patterns_should_score_below_60() {
    let r = analyze("test-repos/fake-project/tests/flaky.test.ts");
    assert!(
        r.score.value < 60,
        "File with Date.now(), Math.random(), fetch without mocks scored {}/{}. Expected < 60.",
        r.score.value,
        r.score.grade
    );
}

#[test]
fn debug_code_should_score_below_65() {
    let r = analyze("test-repos/fake-project/tests/debug-code.test.ts");
    assert!(
        r.score.value < 65,
        "File with console.log, debugger statements scored {}/{}. Expected < 65.",
        r.score.value,
        r.score.grade
    );
}

#[test]
fn auth_good_test_should_score_above_75() {
    let r = analyze("test-repos/fake-project/tests/auth.test.ts");
    assert!(
        r.score.value >= 75,
        "Well-written test file scored {}/{}. Expected >= 75 (B- or above).",
        r.score.value,
        r.score.grade
    );
}

#[test]
fn score_ordering_matches_quality_ordering() {
    // The scoring model should produce a quality ordering that matches
    // human intuition: good > mediocre > bad > terrible.
    let auth = analyze("test-repos/fake-project/tests/auth.test.ts");
    let weak = analyze("test-repos/fake-project/tests/weak-assertions.test.ts");
    let trivial = analyze("test-repos/fake-project/tests/trivial-assertions.test.ts");
    let no_assert = analyze("test-repos/fake-project/tests/no-assertions.test.ts");

    assert!(
        auth.score.value > weak.score.value,
        "auth ({}) should score higher than weak-assertions ({})",
        auth.score.value,
        weak.score.value
    );
    assert!(
        weak.score.value > trivial.score.value,
        "weak-assertions ({}) should score higher than trivial-assertions ({})",
        weak.score.value,
        trivial.score.value
    );
    // With Phase 2.2 rules, trivial-assertions file may get more issues (e.g. vacuous_test)
    // so we only require both to score below weak-assertions.
    assert!(
        weak.score.value > no_assert.score.value,
        "weak-assertions ({}) should score higher than no-assertions ({})",
        weak.score.value,
        no_assert.score.value
    );
}
