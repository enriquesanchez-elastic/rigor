# Action Plan: Resolving All Review Findings

> Generated 2026-02-06 from REVIEW.md findings

This plan is ordered by priority (HIGH first) and dependency (prerequisites before dependents). Each task includes exact file locations, what to change, and estimated scope.

---

## Phase 1: Score Integrity (HIGH - Trust is the Product)

### Task 1.1: Remove Stub Rules from Scoring

**Problem:** 10 Phase 2.2 rules always return `vec![]` but their `calculate_score` methods return 25/25, inflating every score.

**Option A (Recommended): Exclude stubs from engine until implemented**
- **`src/analyzer/engine.rs:308-317`** — Remove instantiation and `.analyze()` calls for these 10 rules:
  - `TestComplexityRule` (line 308)
  - `ImplementationCouplingRule` (line 309)
  - `VacuousTestRule` (line 310)
  - `IncompleteMockVerificationRule` (line 311)
  - `AsyncErrorMishandlingRule` (line 312)
  - `RedundantTestRule` (line 313)
  - `UnreachableTestCodeRule` (line 314)
  - `ExcessiveSetupRule` (line 315)
  - `TypeAssertionAbuseRule` (line 316)
  - `MissingCleanupRule` (line 317)
- Also remove the corresponding `issues.extend(...)` calls (lines 340-349)
- Keep the rule files and `Rule` enum variants for future implementation
- The stub rules don't participate in `calculate_breakdown()` (only the 6 category rules do), so removing them from the engine is safe — they just add dead calls today

**Option B: Implement the 10 rules with tree-sitter**
- This is the long-term fix but requires significant work per rule
- Each rule file already has a TODO comment explaining what it should do
- Recommended: do this incrementally in future PRs, not in this cleanup

**Files to modify:**
- `src/analyzer/engine.rs` — remove 10 instantiations + 10 `issues.extend()` calls
- `src/analyzer/rules/mod.rs` — keep exports (for future implementation)

**Verification:** Run `cargo test` and update any regression test baselines if scores change. Scores should NOT change because these stubs never returned issues anyway — but verify.

---

### Task 1.2: Fix Parallel Cache Write Bug

**Problem:** `analyze_files_parallel_cached` reads from cache but never writes back. Only sequential mode calls `cache.set`.

**File:** `src/main.rs:1046-1087`

**Fix:** After the `engine.analyze()` call succeeds (line 1071-1072), write the result to cache. The challenge is that `cache` is `&AnalysisCache` (shared ref for parallel), not `&mut`. Two options:

**Option A: Use interior mutability**
- Wrap the cache's internal `HashMap` with `RwLock` or `DashMap`
- Change `cache.set()` to take `&self` instead of `&mut self`
- Files: `src/cache.rs` (add `RwLock`), `src/main.rs` (add `cache.set` call after line 1072)

**Option B: Collect results, write cache after parallel section**
- After `results` is collected (line 1087), iterate sequentially and call `cache.set` for each result that wasn't a cache hit
- Simpler change, doesn't require modifying cache internals
- Files: `src/main.rs` only — add a post-collection loop

**Recommended:** Option B is simpler and less risky.

```rust
// After line 1087, add:
// Write new results to cache (sequential, after parallel analysis)
let mut cache_mut = cache.clone(); // or take &mut from caller
for result in &results {
    if let Ok(content) = std::fs::read_to_string(&result.file_path) {
        cache_mut.set(&result.file_path, &content, None, result.clone());
    }
}
```

**Note:** The function signature takes `&AnalysisCache` — the caller may need to be updated to pass `&mut` or the cache needs interior mutability. Check how `analyze_files_sequential_cached` (line 952) receives `&mut AnalysisCache`.

**Verification:** Add a test that analyzes a file with `--parallel`, then verifies the cache file contains the result.

---

## Phase 2: Documentation Accuracy (HIGH)

### Task 2.1: Update Rule Count and Docs

**Files to modify:**

1. **`README.md:67`** — Change "28 rules" to actual count. If stubs are removed from engine (Task 1.1), document only the active rules. If stubs remain, clarify which are implemented vs. planned.
2. **`README.md:186`** — Same update for "All 28 rules with descriptions"
3. **`docs/rules.md:3`** — Change "28 rules" to actual count
4. **`docs/rules.md`** — Add sections for:
   - **Critical Quality** (10 rules): `test-complexity`, `implementation-coupling`, `vacuous-test`, `incomplete-mock-verification`, `async-error-mishandling`, `redundant-test`, `unreachable-test-code`, `excessive-setup`, `type-assertion-abuse`, `missing-cleanup` — mark as "Planned" or "Stub" if not implemented
   - **AI Smells** (6 rules): `ai-smell-tautological-assertion`, `ai-smell-over-mocking`, `ai-smell-shallow-variety`, `ai-smell-happy-path-only`, `ai-smell-parrot-assertion`, `ai-smell-boilerplate-padding`

### Task 2.2: Fix Category Count in README

**File:** `README.md:92`
**Change:** "five categories (assertion quality, error coverage, boundary conditions, test isolation, input variety)" → "six categories (assertion quality, error coverage, boundary conditions, test isolation, input variety, AI smells)"

### Task 2.3: Fix `--fix` Description in README

**File:** `README.md:125`
**Change:** `--fix    Generate AI improvement prompt` → `--fix    Apply auto-fixes for fixable rules`
**Add:** `--suggest    Generate AI improvement prompt` (if not already listed)

### Task 2.4: Fix Version in Doc Comment

**File:** `src/suggestions/claude.rs:6`
**Change:** `rigor = { version = "0.1", features = ["ai"] }` → `rigor = { version = "1.0", features = ["ai"] }`

---

## Phase 3: Fix Own Test Suite (MEDIUM)

### Task 3.1: Fix Tautological Assertion

**File:** `tests/edge_cases.rs:52`
**Current:** `assert!(result.is_err() || result.is_ok());`
**Fix:** This test should verify that syntax errors are handled without panicking. Replace with:
```rust
// The analyzer should either return an error or return Ok with 0 tests — not panic
match result {
    Ok(r) => assert_eq!(r.stats.total_tests, 0),
    Err(_) => {} // Graceful error is acceptable
}
```
Remove the tautological assertion entirely — the `match` already covers both cases. The `if let Ok(r)` on line 53-55 already does the right thing, so just delete line 52.

### Task 3.2: Fix Misleading Test Names

**File:** `tests/integration.rs:31`
**Current:** `fn weak_assertions_scores_f()` with assertion `r.score.value < 95`
**Fix options:**
- **Option A:** Rename to `weak_assertions_penalized` and keep the `< 95` threshold
- **Option B:** Tighten the assertion to match the actual score range (e.g., `< 80` for a real F)
- **Recommended:** Option A — the test name should describe what it actually checks

**File:** `tests/integration.rs:44`
**Current:** `fn mixed_bad_scores_f()` with assertion `r.score.value < 95`
**Fix:** Same as above — rename to `mixed_bad_penalized`

### Task 3.3: Remove Unused `proptest` Dependency

**File:** `Cargo.toml:45`
**Change:** Remove `proptest = "1"` from `[dev-dependencies]`
**Verification:** `cargo check` should still pass

### Task 3.4: Add Integration Tests for MCP Server

**New file:** `tests/mcp.rs` (or extend `tests/integration.rs`)
**Scope:** At minimum, test that the MCP server can:
- Start and respond to `initialize` request
- Handle `tools/list` and return 9 tools
- Execute `analyze_test_quality` with valid input
- Return proper error for malformed JSON-RPC

This is a larger task and could be a separate PR.

---

## Phase 4: Code Quality (MEDIUM)

### Task 4.1: Deduplicate Penalty Constants

**File:** `src/analyzer/scoring.rs`
**Problem:** Lines 25-30 define module-level constants; lines 143-148 redefine the same constants locally.
**Fix:** Delete lines 143-148 (the local redeclarations). The function already has access to the module-level constants.

### Task 4.2: Add AI Smells to Suggestion Breakdown

**File:** `src/suggestions/ai.rs:435-456`
**Fix:** Add a 6th row to the `format_score_breakdown` table:
```rust
| AI Smells | {}/25 | {} |
```
With `result.breakdown.ai_smells` and `Self::score_status(result.breakdown.ai_smells)`.

### Task 4.3: Replace Magic Number in `ScoreBreakdown::total()`

**File:** `src/lib.rs:90-97`
**Current:** `((sum * 100) / 150).min(100) as u8`
**Fix:**
```rust
const NUM_CATEGORIES: u16 = 6;
const MAX_PER_CATEGORY: u16 = 25;
const MAX_TOTAL: u16 = NUM_CATEGORIES * MAX_PER_CATEGORY; // 150
((sum * 100) / MAX_TOTAL).min(100) as u8
```

### Task 4.4: Add Timeout to Claude API Client

**File:** `src/suggestions/claude.rs:92`
**Current:** `let client = reqwest::blocking::Client::new();`
**Fix:**
```rust
let client = reqwest::blocking::Client::builder()
    .timeout(std::time::Duration::from_secs(30))
    .build()
    .map_err(|e| ClaudeError::RequestFailed(e.to_string()))?;
```

### Task 4.5: Remove Unused Coverage Loading

**File:** `src/main.rs:381`
**Current:** `let _coverage_report = if let Some(ref coverage_path) = args.coverage { ... }`
**Fix:** Either:
- Remove the entire block if `--coverage` flag is not meant to do anything yet
- Or add a TODO comment and don't load/parse the file (just validate the path exists)

---

## Phase 5: Architecture Refactoring (LOW - Future PRs)

These are larger refactors that should each be their own PR.

### Task 5.1: Extract Rule Registry Pattern

**File:** `src/analyzer/engine.rs:262-356`
**Goal:** Replace 30+ manual instantiations with a rule registry:
```rust
// Pseudocode
let rules: Vec<Box<dyn AnalysisRule>> = RuleRegistry::default()
    .with_source_context(source_content_ref, source_tree_ref)
    .with_framework(framework)
    .build();

for rule in &rules {
    issues.extend(rule.analyze(&tests, source, tree));
}
```
**Scope:** Medium-large refactor. Requires updating the `AnalysisRule` trait to support `.with_source()` uniformly.

### Task 5.2: Break Up `run()` Function

**File:** `src/main.rs:157-616`
**Goal:** Extract into ~5 smaller functions:
- `collect_files(args, config)` — file collection (staged/changed/glob)
- `run_analysis(engine, files, config, cache)` — dispatch to seq/parallel
- `format_output(results, args)` — JSON/SARIF/quiet/verbose rendering
- `run_mutation_testing(results, args)` — mutation testing flow
- `check_threshold(results, threshold)` — exit code logic

### Task 5.3: Consolidate `analyze_files_*` Variants

**File:** `src/main.rs:950-1100`
**Goal:** Merge the 3 variants into one function with boolean flags for `parallel` and `use_cache`.

### Task 5.4: Split `lib.rs` into Type Modules

**File:** `src/lib.rs` (804 lines)
**Goal:** Move types to:
- `src/types/rule.rs` — `Rule` enum, `Severity`
- `src/types/score.rs` — `Score`, `ScoreBreakdown`, `Grade`, etc.
- `src/types/issue.rs` — `Issue`, `Location`, `Fix`
- `src/types/assertion.rs` — `AssertionKind`, `AssertionQuality`
- Re-export all from `lib.rs` for backwards compatibility

### Task 5.5: Split MCP Server into Modules

**File:** `src/mcp/mod.rs` (926 lines)
**Goal:** Extract into:
- `src/mcp/protocol.rs` — JSON-RPC parsing, response formatting
- `src/mcp/tools/*.rs` — one file per tool handler
- `src/mcp/mod.rs` — routing and startup

---

## Execution Order

```
Phase 1 (Score Integrity)     ← Do first, most critical
  1.1 Remove stubs from engine
  1.2 Fix parallel cache write

Phase 2 (Documentation)       ← Do second, quick wins
  2.1 Update rule count
  2.2 Fix category count
  2.3 Fix --fix description
  2.4 Fix version in doc comment

Phase 3 (Test Suite)           ← Do third, builds credibility
  3.1 Fix tautological assertion
  3.2 Fix misleading test names
  3.3 Remove unused proptest
  3.4 Add MCP tests (separate PR)

Phase 4 (Code Quality)        ← Do fourth, targeted fixes
  4.1 Deduplicate penalty constants
  4.2 Add AI Smells to suggestions
  4.3 Replace magic number
  4.4 Add API timeout
  4.5 Remove unused coverage loading

Phase 5 (Architecture)        ← Future PRs, each standalone
  5.1 Rule registry pattern
  5.2 Break up run()
  5.3 Consolidate analyze_files variants
  5.4 Split lib.rs
  5.5 Split MCP server
```

---

## Estimated Scope

| Phase | Tasks | Files Changed | Risk |
|-------|-------|---------------|------|
| Phase 1 | 2 | 2-3 | Medium (scoring changes need regression test updates) |
| Phase 2 | 4 | 4 | Low (documentation only, plus 1 doc comment) |
| Phase 3 | 4 | 3-4 | Low (test changes don't affect production code) |
| Phase 4 | 5 | 4 | Low (targeted, localized changes) |
| Phase 5 | 5 | 10+ | Higher (structural refactors, needs careful testing) |

Phases 1-4 can be done in a single PR. Phase 5 should be separate PRs.
