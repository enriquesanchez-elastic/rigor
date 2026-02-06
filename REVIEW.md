# Project Review: Rigor v1.0.0

> Independent code review conducted on 2026-02-06

## Overall Assessment: Ambitious and Promising, With Integrity Issues

Rigor is a well-conceived TypeScript test quality linter that fills a real gap between "did my tests pass?" and "are my tests actually good?" The Rust + tree-sitter foundation is solid, and the multi-interface approach (CLI, MCP, LSP, SDK, GitHub Action) is impressive for a v1.0. However, there are several issues that undermine the tool's core value proposition of trustworthy quality scores.

---

## What's Good

- **Clear product vision.** The positioning against Stryker/PIT (fast static feedback vs. deep mutation testing) is smart and honest.
- **Solid technology choices.** Rust for speed, tree-sitter for AST parsing, rustls to avoid OpenSSL pain, rayon for parallelism. Dependencies are lean and up-to-date.
- **No unsafe code.** Zero `unsafe` blocks across the entire Rust codebase.
- **Clean builds.** `cargo check` and `cargo clippy` pass with zero warnings.
- **Good feature isolation.** The `ai` feature flag properly gates the reqwest dependency.
- **Comprehensive tooling surface.** MCP server (9 tools), LSP, VS Code extension, Node SDK, GitHub Action, SARIF output — wide and useful for a v1.0.

---

## Critical Issues

### 1. Ten Phantom Rules Inflate Every Score (HIGH)

**Location:** `src/analyzer/rules/{test_complexity,implementation_coupling,vacuous_test,incomplete_mock_verification,async_error_mishandling,redundant_test,unreachable_test_code,excessive_setup,type_assertion_abuse,missing_cleanup}.rs`

The Phase 2.2 rules are all stubs that return `vec![]`. They participate in scoring but never fire, meaning every analysis gives them a perfect 25/25. This **artificially inflates all scores**. For a tool whose purpose is providing trustworthy quality scores, this is the most serious problem.

### 2. Documentation Significantly Out of Sync (HIGH)

| What | Documentation Says | Reality |
|------|-------------------|---------|
| Rule count | 28 rules | 43 `Rule` enum variants |
| Score categories | "five categories" (README:92) | Six categories (AI Smells added) |
| `--fix` flag | "Generate AI improvement prompt" (README:125) | Auto-fixes files; `--suggest` is the AI prompt |
| Version in doc comments | `0.1` (suggestions/claude.rs) | `1.0.0` (Cargo.toml) |

15 rules (10 stubs + 5 AI smells + 1 missing-cleanup) are not documented in `docs/rules.md`.

### 3. Parallel Cache Write Bug (HIGH)

**Location:** `src/main.rs:1046-1087`

`analyze_files_parallel_cached` reads from cache (`cache.get`) but never calls `cache.set` after analysis. Only sequential mode writes to cache. Since parallel mode auto-triggers for >10 files, most real-world usage gets no cache writes.

---

## Medium Issues

### 4. The Tool's Own Tests Have the Flaws It Detects

| Finding | Location | Irony |
|---------|----------|-------|
| Tautological assertion: `assert!(result.is_err() \|\| result.is_ok())` | `tests/edge_cases.rs:52` | Rigor's own `TautologicalAssertion` rule should catch this |
| Tests named `scores_f` allow Grade A scores (threshold `< 95`) | `tests/integration.rs:31,44` | Rigor flags `vague-test-name` |
| `proptest` in dev-deps but zero property-based tests | `Cargo.toml:45` | Unused dependency |
| MCP server (926 lines), watcher, cache eviction: zero integration tests | `src/mcp/mod.rs` | 48% test coverage gap per PR #1 |

### 5. `main.rs` Complexity

- `run()` function: 460 lines, 12+ branches, handles everything from arg parsing to output formatting
- Three near-identical `analyze_files_*` variants with copy-pasted error handling
- Unused coverage data: `let _coverage_report = ...` loads a file for no purpose

### 6. Scoring Constants Defined Twice

**Location:** `src/analyzer/scoring.rs:25-30` and `src/analyzer/scoring.rs:143-148`

Penalty constants (`PENALTY_PER_ERROR`, etc.) are declared at module level AND re-declared locally in `build_transparent_breakdown`. If one set changes without the other, scores silently diverge.

### 7. AI Suggestions Missing Category

**Location:** `src/suggestions/ai.rs:435-456`

The AI prompt's score breakdown only shows 5 categories, omitting AI Smells. AI-generated suggestions never see the 6th scoring dimension.

---

## Architecture Notes

**Strengths:**
- Clean module dependency chain: `parser -> analyzer -> reporter`
- Rust's module system prevents circular dependencies
- Optional `ai` feature properly gated

**Weaknesses:**
- `lib.rs` is 804 lines — a data model dumping ground with 20+ types and a 43-variant enum
- `engine.rs` directly instantiates all 30+ rules by name (3 changes per new rule)
- `mcp/mod.rs` is 926 lines — an entire JSON-RPC server in one file
- `analyze_core` takes 12 parameters (suppresses clippy warning)
- Magic number `150` in score normalization assumes exactly 6 × 25 categories

---

## Security

- **Good:** API key from env var only, never logged, HTTPS enforced via rustls
- **Good:** No `unsafe` blocks anywhere
- **Note:** No timeout on Claude API blocking requests — could hang indefinitely (`src/suggestions/claude.rs:92`)
- **Note:** `RIGOR_APPLY_CMD` and `RIGOR_TEST_CMD` execute shell commands without validation (expected for dev tools)

---

## Recommendations (Priority Order)

1. **Remove stub rules from scoring or implement them.** The 10 phantom rules that inflate scores are a credibility risk for the tool's core value proposition.
2. **Fix the parallel cache write bug.** Add `cache.set` in `analyze_files_parallel_cached`.
3. **Update all documentation to match the code.** Rule count, category count, CLI flag descriptions.
4. **Fix the ironic test quality issues.** Tautological assertions and misleading test names in the test suite of a test quality tool.
5. **Deduplicate penalty constants** in `scoring.rs` to prevent silent divergence.
6. **Refactor `run()` into smaller functions** and consolidate the three `analyze_files_*` variants.
7. **Add timeout to Claude API client** to prevent indefinite hangs.

---

## Bottom Line

Rigor is a strong concept with solid engineering foundations, but shipped v1.0.0 with 10 stub rules that inflate scores and significant documentation drift. For a tool that asks developers to trust its quality scores, the phantom rules problem is the #1 priority to fix. The roadmap is ambitious — the project needs to finish what it started before adding more surface area.
