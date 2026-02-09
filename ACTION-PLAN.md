# Rigor Action Plan: Path to World-Class

> An honest, prioritized action plan based on a deep code review of the `main` branch.
> Organized by impact and urgency, not by phase number.

---

## Guiding Principles

1. **Trust is the product.** A linting tool that gives wrong scores, fails silently, or contradicts its own docs destroys the trust it needs to be useful. Fix trust issues first.
2. **Fewer features, done well.** Rigor has broad surface area (CLI, LSP, VS Code, MCP, SDK, GitHub Action, mutation testing, AI suggestions). Most of them are half-finished. Finish or cut.
3. **The scoring model is the moat.** If the score is credible, developers and AI agents adopt it. If a file full of `expect(1).toBe(1)` gets a 90/A, the score is not credible. Calibrate before adding features.
4. **Heuristics must earn their keep.** Every string-matching heuristic (flaky detection, mock abuse, framework detection, source mapping) needs a false-positive budget. If a rule fires incorrectly >15% of the time on real codebases, it hurts more than it helps.

---

## Priority 0: Critical Bugs (Do First)

These are broken things shipping today that will cause user-facing failures.

### 0.1 Fix the npm installer async bug
**File:** `npm/install.js:75-81`
**Problem:** `tryDownloadPrebuilt()` returns a Promise that is never awaited. `npm install rigor-cli` completes "successfully" with no binary installed.
**Fix:** Make `main()` async and await the download, or use `execSync` / a synchronous HTTP client. Add an exit code check. This is a one-line category fix with outsized impact.

### 0.2 Fix version mismatch across packages
**Files:** `npm/package.json` (says 0.1.0), `Cargo.toml` (says 1.0.0), `vscode-rigor/package.json` (says 1.0.0)
**Problem:** Users installing via npm get version 0.1.0 while the binary is 1.0.0. The installer constructs download URLs using the npm version, so it will look for `v0.1.0` releases that don't exist.
**Fix:** Align all versions. Add a CI check that fails if versions diverge.

### 0.3 Fix binary name mismatch in npm installer
**File:** `npm/install.js:44`
**Problem:** Installer constructs `rigor-win32-x64.exe` but CI builds `rigor-windows-x86_64.exe`. Similar mismatches for other platforms (`linux-x64` vs `linux-x86_64`).
**Fix:** Align the naming convention between CI build artifacts and the installer's expected names. Pick one convention and enforce it in both places.

### 0.4 Fix GitHub Action silent failure
**File:** `.github/actions/rigor-action/action.yml:32,73`
**Problem:** Uses `2>/dev/null || echo '{"results":[]}'` and `continue-on-error: true`. If Rigor crashes, CI says "No test files changed" instead of failing. This is worse than not having CI at all.
**Fix:** Remove `continue-on-error`. Let the action fail loudly when the tool fails. Add a separate step that handles "no test files changed" as a distinct known-good state.

### 0.5 Resolve documentation contradictions about Phase 2.2 rules
**Files:** `docs/roadmap.md:71`, `docs/rules.md:3`, `CHANGELOG.md:26`, `src/analyzer/engine.rs:305-309`
**Problem:** Four documents disagree about whether 10 rules are implemented:
- Roadmap says "Done"
- CHANGELOG says "11 new critical rules" shipped
- Rules.md says "planned but not yet implemented"
- Code says "stubs excluded from instantiation"
**Fix:** Update roadmap and changelog to say "10 rule stubs added, detection logic pending." Update rules.md to list them clearly as "Planned" with no ambiguity. Users and AI agents read these docs to understand what they're getting.

---

## Priority 1: Scoring Calibration (The Core Value Proposition)

If the score isn't credible, nothing else matters. Currently the scoring is too generous to be useful.

### 1.1 Recalibrate scoring so bad tests get bad scores
**Problem:** `weak-assertions.test.ts` (all `toBeDefined`/`toBeTruthy`) scores 93/A. `trivial-assertions.test.ts` (all `expect(1).toBe(1)`) scores 90/A. These should be C or D grade at best. If a file full of tautological assertions gets an A, the tool provides no signal.
**Approach:**
- Audit each category's `calculate_score` function. The base score per category appears to start high (25/25) and only decreases on specific issue patterns. This means files where no rule fires get perfect scores, even if the tests are meaningless.
- Consider an "evidence of quality" model instead of "deduction from perfection." Start at 0 and earn points for demonstrated good practices: diverse inputs, specific assertions, error path coverage, etc.
- Alternatively, increase penalty weights significantly. Currently warnings cost 2 points with a 40-point cap. A file with 20 warnings (all trivial assertions) only loses 40 points from a base of ~96, landing at 56. But the penalty system caps independently per severity, so mixed files still score high.
- **Deliverable:** A calibration pass where the 8 existing test fixtures produce scores that a senior engineer would agree with. Publish the expected-vs-actual table and justify each score.

### 1.2 Build a scoring benchmark dataset
**Problem:** The roadmap mentions "benchmark dataset" but none exists. Without one, scoring changes are arbitrary — there's no way to know if a change improved accuracy or made it worse.
**Approach:**
- Collect 50-100 real-world test files across quality levels (excellent, good, mediocre, poor, terrible).
- Have 3-5 experienced developers independently grade each file (A-F).
- Use the human consensus as ground truth.
- Measure Rigor's agreement rate. Target: >80% within one grade of human consensus.
- Run this benchmark on every scoring change.

### 1.3 Justify and document penalty caps
**File:** `src/analyzer/scoring.rs:26-30`
**Problem:** `MAX_PENALTY_FROM_ERRORS = 35`, `MAX_PENALTY_FROM_WARNINGS = 40`, `MAX_PENALTY_FROM_INFO = 15`. These magic numbers aren't documented or justified. Total max penalty is 90, but in practice files rarely hit all three caps.
**Fix:** Document the rationale. Consider whether caps should exist at all — if a file has 40 warnings, maybe it should score 0. The current caps protect truly terrible files from looking as bad as they are.

### 1.4 Fix the "no source = free points" problem
**Problem:** When no source file is found (common for SDK users, stdin analysis, new projects), error coverage, boundary conditions, and other source-dependent rules can't fire. The category score defaults to high because there are no issues to penalize. This means `--stdin` analysis is systematically more generous than file-based analysis.
**Fix:** When source analysis is unavailable, those categories should score "unknown" or "N/A" rather than defaulting to 25/25. Re-weight remaining categories to compensate.

---

## Priority 2: Harden the Heuristics

Every rule that fires on the wrong code erodes trust. Fix the rules that will generate false positives on real codebases.

### 2.1 Fix flaky pattern detection
**Problem:** `trimmed.matches(char::is_numeric).count() >= 1` flags any line with a digit as flaky. `expect(array[0])` would trigger. `expect(result.count).toBe(42)` would trigger. This will produce massive false positive volume on real codebases.
**Fix:** Only flag actual non-determinism patterns: `Date.now()`, `Math.random()`, `setTimeout`/`setInterval` in assertions, `new Date()` in test setup without mocking. Drop the "has a number" heuristic entirely.

### 2.2 Fix mock abuse detection
**Problem:** `mod_trimmed.contains("Map")` flags mocking `UserMap`, `SiteMapper`, etc. as mocking the built-in `Map` type. String matching on module names is too coarse.
**Fix:** Use exact matching on known built-ins (`Map`, `Set`, `Array`, `Promise`, `Date`, `JSON`, `Math`). Or better: use tree-sitter to resolve what's actually being mocked.

### 2.3 Fix framework detection fallback
**Problem:** Falls back to `if source.contains("expect(")` and declares Jest. But Playwright, Vitest, and custom frameworks also use `expect()`.
**Fix:** Framework detection should check imports first (`import { test } from '@playwright/test'`, `import { describe } from 'vitest'`). Only fall back to `expect()` heuristic if no import is found, and mark it as "Unknown" rather than guessing Jest.

### 2.4 Fix source mapper glob handling
**Problem:** `pattern.replace("**", "").replace('*', "")` turns `tests/**/*.test.ts` into `tests/.test.ts`, which substring-matches `tests/helpers/setup.test.ts` (utility file, not a real test).
**Fix:** Use the `glob` crate's actual matching instead of string manipulation. The project already depends on `globset`.

### 2.5 Fix async test end-line guessing
**Problem:** `test.location.end_line.unwrap_or(test.location.line + 49)` — if tree-sitter doesn't provide an end line, the code assumes the test is 49 lines long. Issues up to 49 lines after the test start are attributed to it.
**Fix:** Use tree-sitter to always get the end line. If parsing fails, attribute issues only to the exact test line, not a 49-line range.

### 2.6 Fix return path coverage string matching
**Problem:** A test named "zero tolerance" gets credit for testing the "zero path." "negative scenarios" covers the "negative branch." This is name-to-path matching via string contains.
**Fix:** Match test assertions and inputs to return paths, not test names. If a test calls `func(0)` and the source has `if (x === 0) return`, that's a real match. Test names lie.

### 2.7 Audit AI smell thresholds
**Problem:** `mock_count >= 5 && tests.len() <= 3` is arbitrary. "Shallow variety" only checks for numbers 0 and 1. These thresholds need to be validated against real AI-generated tests and real human-written tests.
**Fix:** Run the AI smell rules against a corpus of known AI-generated tests (from Copilot, Claude, GPT) and known human-written tests. Adjust thresholds until false positive rate is <15% on human tests and detection rate is >50% on AI tests.

---

## Priority 3: Testing Infrastructure

The tool that analyzes test quality has shallow tests itself. Fix this.

### 3.1 Replace regression baselines with correctness tests
**File:** `tests/regression.rs`
**Problem:** Tests like `regression!(weak_assertions, ..., 93, 16)` validate that the score hasn't changed, not that it's correct. If a bug inflates all scores by 10 points, just update the baselines and tests pass.
**Fix:** Add semantic tests:
- "A file with only `toBeDefined` assertions should score below 70"
- "A file with no assertions should score below 50"
- "A file that tests error paths, boundary conditions, and uses specific assertions should score above 80"
These test the *intent* of the scoring model, not its current output.

### 3.2 Fix edge case tests that accept any outcome
**File:** `tests/edge_cases.rs`
**Problem:** `match &result { Ok(r) => assert_eq!(r.stats.total_tests, 0), Err(_) => {} }` — this test passes on success AND failure. It's a no-op.
**Fix:** Decide what the correct behavior is for each edge case and assert it. If syntax errors should return `Ok` with 0 tests, assert `Ok`. If they should return `Err`, assert `Err`. Never accept both.

### 3.3 Add integration tests for the distribution pipeline
**Missing tests:**
- npm installer: `node install.js` on a fresh system actually produces a working binary
- GitHub Action: runs against a test repository and produces expected output
- Node SDK: `analyze()` returns typed results on a real test file
- VS Code extension: activates and displays diagnostics
- LSP: connects, receives requests, returns valid responses

These don't need to run on every PR — a nightly CI job is fine. But they need to exist.

### 3.4 Add CI for Windows
**Problem:** CI builds Windows binaries but never tests them. Path handling bugs (backslash vs forward slash) are the most common cross-platform issue in Rust.
**Fix:** Add a Windows runner to CI that runs `cargo test` on Windows.

### 3.5 Add property-based tests for scoring
**Problem:** Unit tests for scoring only check a few hand-picked inputs. There's no guarantee the scoring formula is monotonic (more issues = lower score), bounded (0-100), or consistent.
**Fix:** Use `proptest` or `quickcheck`:
- Score is always 0-100
- Adding an issue never increases the score
- Removing an issue never decreases the score
- Perfect file (no issues, good assertions) scores >= 90
- Empty file scores <= 50

---

## Priority 4: Finish or Cut Half-Done Features

Each of these is currently shipping in a state that hurts more than it helps.

### 4.1 LSP: Fix silent failure and shared logic
**File:** `rigor-lsp/src/main.rs`
**Problems:**
- Analysis failure clears all diagnostics (user sees warnings disappear with no explanation)
- Test file detection is hardcoded string matching instead of using shared `TestWatcher::is_test_file()`
- No timeout on analysis
- Doesn't reload config when `.rigorrc.json` changes
**Fix:**
- On error: keep existing diagnostics, show error notification via `window/showMessage`
- Import and use the shared test file detection logic
- Add a 10-second timeout for analysis
- Watch `.rigorrc.json` and reload config on change

### 4.2 VS Code Extension: Make it useful or remove it
**File:** `vscode-rigor/src/extension.ts` (58 lines)
**Current state:** Forwards LSP diagnostics. No score display, no hover info, no code actions, no manual trigger, no error handling if LSP binary isn't found.
**Decision point:** Either invest in making it a real extension (status bar score, hover explanations, quick-fix code actions, command palette commands) or remove it and tell users to use the CLI. A broken extension is worse than no extension.
**If keeping:** Add at minimum:
- Status bar item showing file score (A/B/C/D/F)
- Hover provider showing rule explanation on diagnostic hover
- Code action provider for auto-fixable rules
- Error message when `rigor-lsp` binary not found
- Manual "Rigor: Analyze File" command

### 4.3 Node SDK: Type it properly or call it a wrapper
**File:** `sdk/node/index.d.ts`
**Problem:** Types use `unknown` everywhere. `stats: { totalTests: number; totalAssertions: number; [k: string]: unknown }`. Users get zero IDE support.
**Fix:**
- Generate TypeScript types from the Rust `AnalysisResult` struct (or maintain them manually matching the JSON schema)
- Add proper error types
- Add timeout option
- Document that it requires `rigor` binary on PATH (currently undocumented)
- Consider: is this SDK adding enough value over `JSON.parse(execSync('rigor --json'))`? If not, document the CLI approach and cut the SDK.

### 4.4 MCP Server: Fix global state
**File:** `src/mcp/mod.rs`
**Problem:** `static CELL: OnceLock<Mutex<HashMap<String, ImprovementSession>>>` means all users sharing the same MCP server process contaminate each other's improvement sessions.
**Fix:** Key sessions by a client-provided session ID (or generate one per connection). Clear stale sessions after a timeout.

### 4.5 Implement Phase 2.2 rules or remove the stubs
**Problem:** 10 rule variants exist in the `Rule` enum, have `Display` implementations, appear in the config schema, but always return `vec![]`. They're excluded from scoring but still exist as dead code paths.
**Decision:** Either implement the 3-4 highest-value rules (vacuous-test, redundant-test, implementation-coupling, excessive-setup) or remove the stubs entirely. Stub rules that never fire are code debt.
**Recommended priority if implementing:**
1. `vacuous-test` — detect `expect(true).toBe(true)`, always-passing conditions
2. `redundant-test` — detect tests with identical assertion targets
3. `excessive-setup` — detect `beforeEach` blocks longer than the tests they set up
4. `implementation-coupling` — detect tests that mock internal functions

### 4.6 Mutation testing: Add actual tests
**Problem:** `src/mutation/` has 28 operators but zero unit tests for them. No integration test verifies mutations are generated correctly.
**Fix:** For each mutation operator, test that:
- It produces the expected mutant code
- The mutant is different from the original
- The mutation is syntactically valid (parseable)

---

## Priority 5: Architecture Improvements

These won't fix user-facing bugs but will make the codebase healthier for ongoing development.

### 5.1 Break up the god function in main.rs
**File:** `src/main.rs`, function `run()` (~450 lines)
**Problem:** One function handles all CLI modes: analyze, watch, mutate, suggest, apply, fix, init, mcp, stdin, SARIF, JSON, quiet, verbose, staged, changed. Every new feature adds more branches.
**Fix:** Extract each mode into its own function or module. The CLI should dispatch to handlers, not be one giant match block.

### 5.2 Separate Rule enum from stub rules
**Problem:** The `Rule` enum has 40+ variants including 10 stubs. The engine has to explicitly skip stub rules. Adding a new rule means touching the enum, Display impl, rule_scoring_category, config schema, and engine.
**Fix:** Consider a registry pattern where rules register themselves with metadata (category, severity default, is_implemented). Or split the enum into `ActiveRule` and `PlannedRule`.

### 5.3 Consolidate test file detection
**Problem:** `TestWatcher::is_test_file()` in the crate and `rigor-lsp/src/main.rs` lines 100-104 have independent implementations. The LSP uses hardcoded string matching.
**Fix:** Make the LSP import and use the shared logic. One definition, one place to update.

### 5.4 Add structured error types
**Problem:** Most errors use `anyhow::Result` with string messages. Callers can't programmatically distinguish "file not found" from "parse error" from "config invalid."
**Fix:** Define an error enum for the public API: `RigorError::FileNotFound`, `RigorError::ParseError`, `RigorError::ConfigError`, etc. Keep `anyhow` for internal plumbing.

---

## Priority 6: New Features That Would Make Rigor World-Class

Only pursue these after Priorities 0-5 are solid.

### 6.1 "Explain this score" mode
**Problem:** Users see "72/C" and don't know what to do. The current recommendations are generic ("Focus on using stronger assertions").
**Fix:** Add `--explain` that shows a step-by-step walkthrough:
```
Score: 72/C

Why not higher:
  - 4 tests use toBeDefined() instead of specific values (-8 points)
  - No error path testing for validateEmail() which throws on invalid input (-12 points)
  - Tests 'should work' and 'should handle input' have identical assertions (-4 points)

Highest-impact fix:
  Add error tests for validateEmail(). This alone would raise your score to ~84/B.
```
This is the killer feature for adoption. Make the score actionable.

### 6.2 `--diff` mode for PR integration
**Problem:** `--changed` analyzes changed files but doesn't show what changed. CI can't tell if the PR made tests better or worse.
**Fix:** `rigor --diff base..head` that shows score delta per file with root cause analysis:
```
auth.test.ts: 72 → 78 (+6) — added error path test
cart.test.ts: 85 → 81 (-4) — new test uses toBeDefined() instead of specific value
```

### 6.3 Auto-fix for more rules
**Current:** Only `focused-test` (remove `.only`) and `debug-code` (remove `console.log`) are auto-fixable.
**High-value additions:**
- `weak-assertion`: `toBeDefined()` → `toBe(expectedValue)` (with value from context)
- `vague-test-name`: Suggest descriptive name based on assertion content
- `skipped-test`: Remove `.skip` / `xit` (with confirmation)
- `missing-await`: Add `await` before async call in test

### 6.4 Real preset system
**Problem:** Every project has to configure from scratch. Teams can't share configurations.
**Fix:** Built-in presets:
- `rigor:recommended` — default for new projects
- `rigor:strict` — higher thresholds, more rules as errors
- `rigor:ai-review` — focused on AI-generated test detection
- `rigor:legacy` — lenient, for gradual adoption on old codebases
Config: `{ "extends": "rigor:strict" }`

### 6.5 HTML report with trends
**Problem:** SARIF and JSON are for machines. Console output is ephemeral. There's no artifact for code reviews or team dashboards.
**Fix:** `rigor --report` generates an HTML file with:
- Score trend over last N runs (from history.json)
- File-level heatmap (worst tests first)
- Issue breakdown by rule
- Comparison to previous run

### 6.6 JavaScript/JSX support
**Problem:** Rigor only supports TypeScript. Many projects use JavaScript or mixed JS/TS.
**Fix:** tree-sitter has a JavaScript grammar. Most rules would work unchanged since they analyze assertion patterns, not types. Add `.test.js`, `.test.jsx`, `.spec.js` detection.

### 6.7 Python test support (long-term)
**Problem:** The test quality gap exists in every language, not just TypeScript. Python's `pytest` ecosystem has no equivalent tool.
**Fix:** tree-sitter has a Python grammar. The scoring model and most rule concepts (weak assertions, missing error tests, shared state) transfer directly. This would double the addressable market.

---

## Priority 7: Things to Consider Removing

Not everything in the codebase is earning its complexity cost.

### 7.1 AI suggestion integration (`--suggest`, `--apply`)
**Problem:** Couples a static analysis tool to a paid external API (Claude). Requires `RIGOR_APPLY_CMD` environment variable. Complex code path for a feature that most users won't use.
**Recommendation:** Move to a separate companion tool or plugin. The core tool should be self-contained and free.

### 7.2 Watch mode
**Problem:** The LSP already provides real-time analysis on save. Watch mode duplicates this for terminal users, but with minimal testing and no documented use case beyond what `nodemon rigor` would provide.
**Recommendation:** Keep but don't invest further. Document that LSP is the preferred real-time experience.

### 7.3 Scoring v1/v2 toggle
**File:** `src/analyzer/scoring.rs` — two separate penalty functions based on `scoring_version` config
**Problem:** Maintaining two scoring algorithms doubles the testing surface. v1 has the double-counting bug that v2 fixes.
**Recommendation:** Remove v1, make v2 the default. Breaking change, but if the tool is at version 1.0, this is the time to do it.

---

## Implementation Order

```
Week 1-2:  Priority 0 (Critical bugs — npm, versions, docs, GH Action)
Week 3-4:  Priority 1.1 (Scoring recalibration — the core value)
Week 5-6:  Priority 2 (Top 3-4 heuristic fixes by false positive volume)
Week 7-8:  Priority 3.1-3.2 (Replace tautological tests with correctness tests)
Week 9-10: Priority 4.1, 4.5 (LSP fixes, implement top 2 Phase 2.2 rules)
Week 11-12: Priority 6.1 (Explain mode — the killer feature for adoption)
Ongoing:   Priority 1.2 (Benchmark dataset, built incrementally)
```

---

## Success Criteria

How to know if Rigor is world-class:

1. **A senior engineer agrees with the score.** Show 20 test files to 5 engineers. If Rigor's grade matches their grade (within one letter) >80% of the time, the scoring is credible.

2. **Zero silent failures.** npm install works. CI fails loudly when it should. The LSP shows errors to users. There is no mode where the tool fails and the user doesn't know.

3. **<15% false positive rate.** Run Rigor on 10 popular open-source TypeScript projects. Manually review flagged issues. If >15% are wrong, the rules need work.

4. **AI agents use the score as a feedback signal.** When Claude or Copilot generates tests and runs Rigor, the score improves on iteration. The MCP tools return actionable feedback, not just numbers.

5. **Adoption signal: teams set it as a CI gate.** The tool is trusted enough that teams use `--threshold 70` in CI and don't override it. If teams immediately disable rules or lower thresholds, the scoring needs calibration.
