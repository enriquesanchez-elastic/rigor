//! Regression tests: baseline score and issue count per fake-project file.
//! Update baselines intentionally when rules or scoring change.
//!
//! NOTE: These tests protect against *accidental* score changes, not *incorrect* scores.
//! See tests/integration.rs for semantic correctness tests.
//! Baselines updated for:
//!   - v2-only scoring (no double-counting) with penalty 7/3/1
//!   - P1.3 fix: source-dependent categories capped at 15/25 when no source available
//!   - No-assertion test floor: tests with 0 assertions capped at score 30
//!   - Per-test aggregation capped by file-level breakdown score

use rigor::analyzer::AnalysisEngine;
use std::path::Path;

const FAKE_PROJECT_ROOT: &str = "test-repos/fake-project";

fn analyze(test_path: &str) -> rigor::AnalysisResult {
    let engine =
        AnalysisEngine::new().with_project_root(std::path::PathBuf::from(FAKE_PROJECT_ROOT));
    engine
        .analyze(Path::new(test_path), None)
        .unwrap_or_else(|e| panic!("analyze({}) failed: {}", test_path, e))
}

macro_rules! regression {
    ($name:ident, $path:expr, $score:expr, $issues:expr) => {
        #[test]
        fn $name() {
            let r = analyze($path);
            assert_eq!(
                r.score.value, $score,
                "{} score changed from baseline",
                $path
            );
            assert_eq!(
                r.issues.len(),
                $issues,
                "{} issue count changed from baseline",
                $path
            );
        }
    };
}

// tests/ (baselines updated for redundant-test precision + missing-error-test fix + hardcoded dedup)
regression!(
    assertion_intent_mismatch,
    "test-repos/fake-project/tests/assertion-intent-mismatch.test.ts",
    61,
    17
);
regression!(
    async_missing_await,
    "test-repos/fake-project/tests/async-missing-await.test.ts",
    71,
    8
);
regression!(auth, "test-repos/fake-project/tests/auth.test.ts", 91, 11);
regression!(cart, "test-repos/fake-project/tests/cart.test.ts", 79, 15);
regression!(
    debug_code,
    "test-repos/fake-project/tests/debug-code.test.ts",
    51,
    19
);
regression!(
    duplicate_names,
    "test-repos/fake-project/tests/duplicate-names.test.ts",
    73,
    6
);
regression!(flaky, "test-repos/fake-project/tests/flaky.test.ts", 59, 21);
regression!(
    hardcoded_limited_input,
    "test-repos/fake-project/tests/hardcoded-limited-input.test.ts",
    75,
    9
);
regression!(
    missing_boundary_tests,
    "test-repos/fake-project/tests/missing-boundary-tests.test.ts",
    82,
    3
);
regression!(
    missing_error_tests,
    "test-repos/fake-project/tests/missing-error-tests.test.ts",
    80,
    6
);
regression!(
    mixed_bad,
    "test-repos/fake-project/tests/mixed-bad.test.ts",
    36,
    21
);
regression!(
    mock_abuse,
    "test-repos/fake-project/tests/mock-abuse.test.ts",
    75,
    8
);
regression!(
    mutation_resistant,
    "test-repos/fake-project/tests/mutation-resistant.test.ts",
    78,
    7
);
regression!(
    no_assertions,
    "test-repos/fake-project/tests/no-assertions.test.ts",
    30,
    11
);
regression!(
    shared_state,
    "test-repos/fake-project/tests/shared-state.test.ts",
    78,
    5
);
regression!(
    skipped_and_focused,
    "test-repos/fake-project/tests/skipped-and-focused.test.ts",
    25,
    30
);
regression!(
    snapshot_only,
    "test-repos/fake-project/tests/snapshot-only.test.ts",
    42,
    21
);
regression!(
    trivial_assertions,
    "test-repos/fake-project/tests/trivial-assertions.test.ts",
    31,
    25
);
regression!(
    vague_names,
    "test-repos/fake-project/tests/vague-names.test.ts",
    32,
    20
);
regression!(
    weak_assertions,
    "test-repos/fake-project/tests/weak-assertions.test.ts",
    66,
    15
);

// e2e/
regression!(
    checkout_cy,
    "test-repos/fake-project/e2e/checkout.cy.ts",
    78,
    8
);
regression!(
    flaky_playwright,
    "test-repos/fake-project/e2e/flaky-playwright.e2e.test.ts",
    79,
    10
);
regression!(
    login_e2e,
    "test-repos/fake-project/e2e/login.e2e.test.ts",
    79,
    7
);
regression!(
    weak_cypress,
    "test-repos/fake-project/e2e/weak-cypress.cy.ts",
    78,
    9
);

// vitest/
regression!(
    vitest_math,
    "test-repos/fake-project/vitest/math.test.ts",
    72,
    6
);

// src/
regression!(
    button_test,
    "test-repos/fake-project/src/components/Button.test.tsx",
    86,
    9
);
regression!(
    button_bad_test,
    "test-repos/fake-project/src/components/Button.bad.test.tsx",
    71,
    11
);
regression!(
    validators_test,
    "test-repos/fake-project/src/__tests__/validators.test.ts",
    51,
    14
);

/// Run with: cargo test --test regression print_baselines -- --ignored --nocapture
/// Then paste output to update the regression!() macros above.
#[test]
#[ignore]
fn print_baselines() {
    let items: &[(&str, &str)] = &[
        (
            "assertion_intent_mismatch",
            "test-repos/fake-project/tests/assertion-intent-mismatch.test.ts",
        ),
        (
            "async_missing_await",
            "test-repos/fake-project/tests/async-missing-await.test.ts",
        ),
        ("auth", "test-repos/fake-project/tests/auth.test.ts"),
        ("cart", "test-repos/fake-project/tests/cart.test.ts"),
        (
            "debug_code",
            "test-repos/fake-project/tests/debug-code.test.ts",
        ),
        (
            "duplicate_names",
            "test-repos/fake-project/tests/duplicate-names.test.ts",
        ),
        ("flaky", "test-repos/fake-project/tests/flaky.test.ts"),
        (
            "hardcoded_limited_input",
            "test-repos/fake-project/tests/hardcoded-limited-input.test.ts",
        ),
        (
            "missing_boundary_tests",
            "test-repos/fake-project/tests/missing-boundary-tests.test.ts",
        ),
        (
            "missing_error_tests",
            "test-repos/fake-project/tests/missing-error-tests.test.ts",
        ),
        (
            "mixed_bad",
            "test-repos/fake-project/tests/mixed-bad.test.ts",
        ),
        (
            "mock_abuse",
            "test-repos/fake-project/tests/mock-abuse.test.ts",
        ),
        (
            "mutation_resistant",
            "test-repos/fake-project/tests/mutation-resistant.test.ts",
        ),
        (
            "no_assertions",
            "test-repos/fake-project/tests/no-assertions.test.ts",
        ),
        (
            "shared_state",
            "test-repos/fake-project/tests/shared-state.test.ts",
        ),
        (
            "skipped_and_focused",
            "test-repos/fake-project/tests/skipped-and-focused.test.ts",
        ),
        (
            "snapshot_only",
            "test-repos/fake-project/tests/snapshot-only.test.ts",
        ),
        (
            "trivial_assertions",
            "test-repos/fake-project/tests/trivial-assertions.test.ts",
        ),
        (
            "vague_names",
            "test-repos/fake-project/tests/vague-names.test.ts",
        ),
        (
            "weak_assertions",
            "test-repos/fake-project/tests/weak-assertions.test.ts",
        ),
        ("checkout_cy", "test-repos/fake-project/e2e/checkout.cy.ts"),
        (
            "flaky_playwright",
            "test-repos/fake-project/e2e/flaky-playwright.e2e.test.ts",
        ),
        ("login_e2e", "test-repos/fake-project/e2e/login.e2e.test.ts"),
        (
            "weak_cypress",
            "test-repos/fake-project/e2e/weak-cypress.cy.ts",
        ),
        ("vitest_math", "test-repos/fake-project/vitest/math.test.ts"),
        (
            "button_test",
            "test-repos/fake-project/src/components/Button.test.tsx",
        ),
        (
            "button_bad_test",
            "test-repos/fake-project/src/components/Button.bad.test.tsx",
        ),
        (
            "validators_test",
            "test-repos/fake-project/src/__tests__/validators.test.ts",
        ),
    ];
    for (name, path) in items {
        let r = analyze(path);
        println!(
            "regression!({}, \"{}\", {}, {});",
            name,
            path,
            r.score.value,
            r.issues.len()
        );
    }
}
