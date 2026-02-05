# Rigor Testing Strategy

## Current State

- **71 tests**, all passing, 0.05s runtime
- **26 of 56** source files have `#[cfg(test)]` modules
- **10 of 21** analysis rules (48%) have **zero tests**
- **0 integration tests** — the fixtures exist but nothing runs the engine against them
- The CLI panics in debug mode due to a clap configuration bug (`required_unless_present = "command"` references a non-existent group)
- The `test-repos/fake-project/` contains **28 intentionally flawed test files** covering every rule category — but is never used by any Rust test

## Test Assets Available

### `tests/fixtures/` — Small, Focused Examples

| File | Purpose | Frameworks |
|------|---------|------------|
| `auth.ts` | Source: auth functions with throw, boundaries, sessions | — |
| `good-tests.test.ts` | High-quality test example (strong assertions, boundaries, error tests) | Jest |
| `weak-assertions.test.ts` | Weak assertions, no assertions, skipped, shared state | Jest |
| `dunder-tests/__tests__/Button.test.tsx` | React component testing (9 tests, RTL) | Jest + RTL |
| `e2e-example/login.e2e.test.ts` | E2E login flow | Playwright |
| `monorepo-example/packages/auth/` | Async auth service + tests | Jest |

### `test-repos/fake-project/` — Comprehensive Anti-Pattern Catalog

**Source files (4):**
- `src/auth/auth.ts` — AuthError, authenticate, validateAge (boundary at 18), createSession
- `src/cart/cart.ts` — Cart class with add (boundary 1-100), remove (throws), getTotal, clear
- `src/components/Button.tsx` — React component with variants/sizes
- `src/utils/validators.ts` — isValidEmail, ParseError, parsePrice (throws, negative boundary), clamp

**Test files by intentional anti-pattern (20 in `tests/`):**

| File | Intended Anti-Pattern | Rules That Should Fire | Currently Fires? |
|------|----------------------|----------------------|-----------------|
| `weak-assertions.test.ts` | toBeDefined, toBeTruthy only | weak-assertion, assertion-intent-mismatch | YES |
| `trivial-assertions.test.ts` | expect(1).toBe(1) | trivial-assertion | YES |
| `no-assertions.test.ts` | Tests without expect() | no-assertions | YES |
| `vague-names.test.ts` | "test 1", "should work" | vague-test-name | YES |
| `debug-code.test.ts` | console.log, debugger | debug-code | YES |
| `flaky.test.ts` | Date.now(), Math.random(), setTimeout | flaky-pattern | YES |
| `shared-state.test.ts` | Module-level let without beforeEach | shared-state | YES |
| `duplicate-names.test.ts` | Identical test names | duplicate-test | YES |
| `skipped-and-focused.test.ts` | it.skip, it.only, fit, xit | skipped-test, focused-test | YES |
| `mock-abuse.test.ts` | Mocks fs, path, os, crypto, http | mock-abuse | YES |
| `snapshot-only.test.ts` | Only toMatchSnapshot assertions | snapshot-overuse | YES |
| `async-missing-await.test.ts` | Missing await on promises | missing-await | YES |
| `missing-error-tests.test.ts` | Only success paths | missing-error-test | YES |
| `missing-boundary-tests.test.ts` | No edge values | missing-boundary-test | WEAK (scores 95/A) |
| `hardcoded-limited-input.test.ts` | Same email repeated 9 times | hardcoded-values, limited-input-variety | YES |
| `mutation-resistant.test.ts` | Uses > instead of exact values | mutation-resistant | YES |
| `assertion-intent-mismatch.test.ts` | Name says "throws" but no toThrow | assertion-intent-mismatch | YES |
| `mixed-bad.test.ts` | Multiple problems combined | multiple rules | YES |
| `cart.test.ts` | beforeAll (not beforeEach) shared state | shared-state | YES |
| `auth.test.ts` | Good quality (control) | minimal issues | YES (score: 83 B) |

**E2E / Component test files (8):**

| File | Framework | Intended Pattern |
|------|-----------|-----------------|
| `e2e/checkout.cy.ts` | Cypress | Good Cypress patterns |
| `e2e/weak-cypress.cy.ts` | Cypress | Weak `.should('exist')` only |
| `e2e/login.e2e.test.ts` | Playwright | Good E2E flow |
| `e2e/flaky-playwright.e2e.test.ts` | Playwright | Flaky E2E patterns |
| `vitest/math.test.ts` | Vitest | Vitest-specific patterns |
| `src/components/Button.test.tsx` | Jest + RTL | Good component test |
| `src/components/Button.bad.test.tsx` | Jest + RTL | RTL anti-patterns |
| `src/__tests__/validators.test.ts` | Jest | Snapshot-only |

## Testing Strategy

### Layer 1: Unit Tests for Analysis Rules

**Goal:** Every rule produces correct output for known inputs.

**Pattern:** For each of the 21 rules in `src/analyzer/rules/`, add a `#[cfg(test)]` module with:

1. **Positive test** — Input that should trigger the rule. Assert the rule fires, check severity and rule ID.
2. **Negative test** — Clean input that should NOT trigger the rule. Assert no issues returned.
3. **Score test** — Verify `calculate_score()` returns expected values for known issue counts.

**Priority order** (untested rules first, sorted by risk):

| Rule | Lines | Risk | Why |
|------|-------|------|-----|
| `flaky_patterns.rs` | 178 | HIGH | False positive here means telling users their tests are flaky when they aren't |
| `debug_code.rs` | 149 | HIGH | Simple pattern match — easy to over-match or under-match |
| `mock_abuse.rs` | 161 | HIGH | Mock detection heuristics are fragile |
| `async_patterns.rs` | 113 | HIGH | Missing-await detection must be precise |
| `behavioral_completeness.rs` | 229 | HIGH | Complex source analysis, many edge cases |
| `return_path_coverage.rs` | 212 | HIGH | Depends on correct source parsing |
| `side_effect_verification.rs` | 186 | HIGH | Mutation detection in source must be accurate |
| `state_verification.rs` | 85 | MEDIUM | Simpler heuristic |
| `react_testing_library.rs` | 115 | MEDIUM | Only applies to RTL users |
| `boundary_specificity.rs` | 78 | MEDIUM | Boundary value specifics |
| `mutation_resistant.rs` | 87 | MEDIUM | Mutation resistance heuristic |

**Estimated effort:** ~35 new tests (3 per untested rule + a few for under-tested rules).

### Layer 2: Integration Tests Using fake-project

**Goal:** The full analysis pipeline (parse → detect → map source → run rules → score) produces expected results for known-quality files.

**Location:** `tests/integration.rs` (new file)

**Structure:**

```rust
// tests/integration.rs

use rigor::analyzer::AnalysisEngine;
use std::path::Path;

fn analyze(test_path: &str) -> rigor::AnalysisResult {
    let engine = AnalysisEngine::new()
        .with_project_root("test-repos/fake-project".into());
    engine.analyze(Path::new(test_path), None).unwrap()
}

// --- Score sanity tests ---

#[test]
fn good_test_scores_b_or_above() {
    let r = analyze("test-repos/fake-project/tests/auth.test.ts");
    assert!(r.score.value >= 80, "auth.test.ts = {} ({})", r.score.value, r.score.grade);
}

#[test]
fn weak_assertions_scores_f() {
    let r = analyze("test-repos/fake-project/tests/weak-assertions.test.ts");
    assert!(r.score.value < 60, "weak-assertions = {} ({})", r.score.value, r.score.grade);
}

#[test]
fn trivial_assertions_detected() {
    let r = analyze("test-repos/fake-project/tests/trivial-assertions.test.ts");
    assert!(r.issues.iter().any(|i| i.rule == rigor::Rule::TrivialAssertion));
}

// --- Rule-specific detection tests ---

#[test]
fn flaky_patterns_detected() {
    let r = analyze("test-repos/fake-project/tests/flaky.test.ts");
    assert!(r.issues.iter().any(|i| i.rule == rigor::Rule::FlakyPattern));
}

#[test]
fn debug_code_detected() {
    let r = analyze("test-repos/fake-project/tests/debug-code.test.ts");
    assert!(r.issues.iter().any(|i| i.rule == rigor::Rule::DebugCode));
}

// ... one per anti-pattern file
```

**Test cases (one per fake-project file):**

| Test | File | Assertion |
|------|------|-----------|
| `good_test_scores_b_or_above` | auth.test.ts | score >= 80 |
| `weak_assertions_scores_f` | weak-assertions.test.ts | score < 60 |
| `trivial_assertions_detected` | trivial-assertions.test.ts | has TrivialAssertion issue |
| `no_assertions_detected` | no-assertions.test.ts | has NoAssertions issue |
| `vague_names_detected` | vague-names.test.ts | has VagueTestName issue |
| `debug_code_detected` | debug-code.test.ts | has DebugCode issue |
| `flaky_patterns_detected` | flaky.test.ts | has FlakyPattern issue |
| `shared_state_detected` | shared-state.test.ts | has SharedState issue |
| `duplicate_names_detected` | duplicate-names.test.ts | has DuplicateTest issue |
| `skipped_and_focused_detected` | skipped-and-focused.test.ts | has SkippedTest + FocusedTest |
| `mock_abuse_detected` | mock-abuse.test.ts | has MockAbuse issue |
| `snapshot_overuse_detected` | snapshot-only.test.ts | has SnapshotOveruse issue |
| `missing_await_detected` | async-missing-await.test.ts | has MissingAwait issue |
| `missing_error_test_detected` | missing-error-tests.test.ts | has MissingErrorTest issue |
| `missing_boundary_test_detected` | missing-boundary-tests.test.ts | has MissingBoundaryTest issue |
| `hardcoded_values_detected` | hardcoded-limited-input.test.ts | has HardcodedValues issue |
| `mutation_resistant_detected` | mutation-resistant.test.ts | has MutationResistant issue |
| `assertion_intent_mismatch_detected` | assertion-intent-mismatch.test.ts | has AssertionIntentMismatch issue |
| `mixed_bad_scores_f` | mixed-bad.test.ts | score < 60, multiple rules fire |
| `cypress_framework_detected` | e2e/checkout.cy.ts | framework == Cypress |
| `playwright_framework_detected` | e2e/login.e2e.test.ts | framework == Playwright |
| `vitest_framework_detected` | vitest/math.test.ts | framework == Vitest |
| `rtl_patterns_detected` | src/components/Button.bad.test.tsx | has RtlPreferScreen |
| `source_mapping_works` | tests/auth.test.ts | source_file == Some("src/auth/auth.ts") |

**Estimated effort:** ~24 integration tests.

### Layer 3: Fixture-Based Regression Tests

**Goal:** Score stability. Prevent rule changes from silently shifting scores.

**Location:** `tests/regression.rs` (new file)

**Approach:** Snapshot the current scores and issue counts for every fake-project file. If a rule change shifts a score by more than 5 points or adds/removes issues, the test fails and forces a deliberate decision.

```rust
#[test]
fn score_regression_auth_test() {
    let r = analyze("test-repos/fake-project/tests/auth.test.ts");
    // Baseline: 83 B, 7 issues — update intentionally when rules change
    assert_eq!(r.score.value, 83, "Score changed from baseline 83");
    assert_eq!(r.issues.len(), 7, "Issue count changed from baseline 7");
}
```

One test per fake-project file = 28 regression tests. These are strict and must be updated manually when rules change intentionally.

**Estimated effort:** ~28 regression tests (boilerplate-heavy but simple).

### Layer 4: Edge Case and Error Handling Tests

**Goal:** The engine handles degenerate inputs without panicking.

| Test Case | Input | Expected |
|-----------|-------|----------|
| Empty file | `""` | 0 tests, no panic, score 0 or low |
| Not TypeScript | `"hello world"` | Parses without error, 0 tests |
| Only comments | `"// nothing here"` | 0 tests, no crash |
| Syntax error | `"function {{{ broken"` | Handles gracefully (no panic) |
| 1000+ tests | Generated large file | Completes, no timeout |
| No describe block | `"it('test', () => { expect(1).toBe(1); });"` | Extracts 1 test |
| Deeply nested describes | 10 levels of describe | Extracts tests correctly |
| UTF-8 edge cases | Unicode identifiers, emojis in strings | No crash |
| File with BOM | `"\xEF\xBB\xBF..."` | Parses correctly |

**Location:** `tests/edge_cases.rs`

**Estimated effort:** ~12 tests.

### Layer 5: CLI Behavior Tests

**Goal:** The binary produces correct exit codes, respects flags, handles errors.

| Test Case | Command | Expected |
|-----------|---------|----------|
| Below threshold | `rigor file.test.ts --threshold 90` | Exit code 1 |
| Above threshold | `rigor file.test.ts --threshold 20` | Exit code 0 |
| JSON output | `rigor file.test.ts --json` | Valid JSON on stdout |
| SARIF output | `rigor file.test.ts --sarif` | Valid SARIF on stdout |
| File not found | `rigor nonexistent.test.ts` | Exit code 2, error message |
| Init creates config | `rigor init` | Creates .rigorrc.json |
| Debug mode panic | `cargo run -- file.test.ts` | **BUG: Currently panics** |

**Location:** `tests/cli.rs` (using `assert_cmd` or `std::process::Command`)

**Note:** The debug-mode panic is a known bug to fix first.

**Estimated effort:** ~8 tests.

### Layer 6: Tighten Existing Tests

**Goal:** Existing tests use precise assertions instead of loose bounds.

| Current | Problem | Fix |
|---------|---------|-----|
| `scoring.rs: assert!(score.value >= 70)` | Value is 95, bound is useless | `assert_eq!(score.value, 95)` |
| `scoring.rs: test_grade_from_score` | No boundary tests | Add 89, 90, 79, 80, 69, 70, 59, 60, 0, 100 |
| `engine.rs: assert!(score.value > 0)` | Any positive score passes | Assert specific expected score |
| `engine.rs: assert!(score.value < 90)` | Loose upper bound | Assert specific expected score |
| `error_coverage.rs: test_error_test_detection` | Passes on "error" keyword, not function name | Fix test to use non-error name |
| `boundary_conditions.rs: test_boundary_detection` | Only positive case | Add negative case |
| `test_isolation.rs` | Tests detection, not full rule analyze() | Add analyze() integration test |

**Estimated effort:** ~15 test modifications.

## Execution Order

1. **Fix the debug-mode CLI panic** — This is a real bug, not just a test issue
2. **Layer 1** — Unit tests for 10 untested rules (~35 tests)
3. **Layer 2** — Integration tests using fake-project (~24 tests)
4. **Layer 6** — Tighten existing tests (~15 modifications)
5. **Layer 3** — Regression baselines (~28 tests)
6. **Layer 4** — Edge case tests (~12 tests)
7. **Layer 5** — CLI tests (~8 tests)

**Total new tests: ~122.** This brings coverage from 71 to ~193 tests and covers every rule, the full pipeline, edge cases, and CLI behavior.

## Known Issues Found During Analysis

1. **CLI panics in debug mode** — `required_unless_present = "command"` references `"command"` but the subcommand field is named `command` as an `Option<Commands>`. In debug builds, clap's assertion checker catches this mismatch and panics. Release builds skip the assertion and work fine.

2. **`missing-boundary-tests.test.ts` scores 95/A** — This file is *designed* to demonstrate missing boundary tests, but rigor gives it an A. The rule only fires when it has source file context (boundary conditions extracted from source), and without `--project-root` pointing to `test-repos/fake-project`, source mapping may not find the corresponding source. Even with source mapping, the boundary detection may need tuning.

3. **`error_coverage::test_error_test_detection` tests the wrong thing** — The test passes because the test name contains "error", not because it matches the function name "someFunction". If the function name were changed to "xyz", the test would still pass. The test should use a function-specific name instead.

4. **Score assertions are too loose** — Multiple existing tests use `>= 70` or `< 90` bounds when the actual values are known. These provide no regression protection.

## Metrics to Track

After implementing this strategy, measure:

- **Rule coverage:** 21/21 rules tested (currently 11/21)
- **Integration coverage:** All fixture files exercised (currently 0)
- **Test count:** Target ~193 (currently 71)
- **Regression baselines:** 28 files baselined (currently 0)
- **Time to run:** Should stay under 2s for the full suite
