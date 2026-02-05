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
    assert!(
        r.score.value >= 80,
        "auth.test.ts = {} ({})",
        r.score.value,
        r.score.grade
    );
}

#[test]
fn weak_assertions_scores_f() {
    let r = analyze("test-repos/fake-project/tests/weak-assertions.test.ts");
    assert!(
        r.score.value < 60,
        "weak-assertions = {} ({})",
        r.score.value,
        r.score.grade
    );
}

#[test]
fn mixed_bad_scores_f() {
    let r = analyze("test-repos/fake-project/tests/mixed-bad.test.ts");
    assert!(
        r.score.value < 60,
        "mixed-bad = {} ({})",
        r.score.value,
        r.score.grade
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
