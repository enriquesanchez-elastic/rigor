# Changelog

All notable changes to Rigor will be documented in this file.

## [1.0.1] - 2026-02-10

### Highlights

Post-release hardening cycle addressing scoring credibility, false-positive reduction, and infrastructure reliability. All 355 tests pass.

---

### Scoring Calibration

- **Removed scoring v1** — the old double-counting algorithm has been completely removed; v2 (no double-counting) is now the only scoring path
- **Fixed "no source = free points"** — when source file is unavailable, source-dependent categories (error coverage, boundary conditions) now use proportional scaling (`score × 15/25`) instead of awarding full marks, preserving issue deductions
- **Added no-assertion test floor** — tests with zero assertions are capped at 30/F regardless of other signals
- **Per-test aggregation cap** — file score is now `min(breakdown_score, aggregated_per_test_score)`, preventing inflated per-test averages from overriding poor file-level quality
- **Per-test display scaling** — individual test scores are proportionally scaled to the file's final score, eliminating confusing disconnects (e.g. "all tests B but file is F")
- **Transparent breakdown display** — shows the per-test cap step when it changes the final score, with clear arithmetic
- **Increased penalty constants** — Error: 5→7, Warning: 2→3; max penalty from errors: 35→50

### Heuristic Hardening (False-Positive Reduction)

- **Flaky pattern detection** (`flaky_patterns.rs`) — replaced "line has any digit" heuristic with actual numeric delay argument detection for `setTimeout`/`setInterval`
- **Mock abuse detection** (`mock_abuse.rs`) — exact match on final module path segment instead of substring `contains()` (fixes `UserMap` falsely matching `Map`)
- **Framework detection** (`framework.rs`) — bare `expect()` no longer defaults to Jest; returns `Unknown` to prevent framework misattribution
- **Async test end-line** (`async_patterns.rs`) — removed arbitrary +49 line default; falls back to start line only, preventing issues from being misattributed across distant code
- **Cypress assertion detection** — `.and()` now recognized as assertion alias for `.should()`

### Bug Fixes

- **npm installer** — `main()` is now async and properly awaits downloads; added `process.exit(1)` on failure
- **Binary platform map** — npm installer now translates Node.js platform conventions to CI artifact names correctly
- **GitHub Action** — removed `2>/dev/null`, `|| true`, and `continue-on-error: true` that silently swallowed failures; added `rigor --version` pre-check and graceful "no test files changed" handling
- **Version alignment** — npm package version updated from 0.1.0 to 1.0.0

### Testing

- **9 new semantic scoring tests** — validate scoring *intent* (e.g. "no-assertions should score below 40", "score ordering matches quality ordering") to prevent future regressions
- **Edge case tests** — syntax error test now asserts `result.is_ok()` and `total_tests == 0` instead of silently accepting errors
- **Regression baselines** — all 28 baseline scores updated to match calibrated scoring
- **New unit tests** — mock abuse negative cases (`UserMap` ≠ `Map`), framework detection (`bare expect ≠ Jest`)

### Infrastructure

- **LSP test detection** — `rigor-lsp` now uses shared `TestWatcher::is_test_file()` instead of inline patterns
- **Node SDK types** — replaced `unknown` with fully typed TypeScript interfaces
- **Config schema** — removed deprecated `scoring_version` field

---

## [1.0.0] - 2026-02-06

### Highlights

Rigor v1.0.0 is a major release that brings **38+ analysis rules**, an **LSP server** with **VS Code extension**, a **Node.js SDK**, **auto-fix capabilities**, and an **enhanced MCP server** with 9 tools for AI-native workflows. This release marks production readiness with comprehensive developer tooling across editors, CI, and programmatic APIs.

---

### New Analysis Rules

#### AI Smell Detection (`ai-smells`)

Six new rules targeting patterns common in AI-generated tests:

- **Tautological Assertion** — detects `expect(x).toBe(x)` self-comparison patterns
- **Over-Mocking** — flags excessive mocks relative to test count
- **Shallow Variety** — identifies tests with limited input diversity
- **Happy-Path-Only** — detects missing error/failure path coverage
- **Parrot Assertion** — flags vague test names like "works" or "returns value"
- **Boilerplate Padding** — detects heavy setup with few actual assertions

#### Critical Quality Rules (10 stubs added)

Rule stubs added to the enum and config schema for future implementation. These rules are
**excluded from scoring** until detection logic is complete:

- **test-complexity** — flags overly complex tests with too many branches or assertions
- **vacuous-test** — detects tests that always pass regardless of behavior
- **implementation-coupling** — flags tests tightly coupled to implementation details
- **incomplete-mock-verification** — detects mocks created without proper verification
- **async-error-mishandling** — flags missing async error handling in tests
- **redundant-test** — detects duplicate or overlapping test coverage
- **unreachable-test-code** — flags dead code after return/throw statements
- **excessive-setup** — detects overly complex `beforeEach`/`beforeAll` blocks
- **type-assertion-abuse** — flags overuse of type assertions (`as Type`)
- **missing-cleanup** — detects tests missing teardown for resources they acquire

### New Components

#### Language Server Protocol (LSP) — `rigor-lsp`

- Real-time diagnostics published on save for test files
- Supports `.test.ts`, `.spec.ts`, `.cy.ts` file patterns
- Integrates with any LSP-compatible editor

#### VS Code Extension — `vscode-rigor`

- Inline diagnostics powered by `rigor-lsp`
- Configurable via VS Code settings (`rigor.enable`, `rigor.path`)
- Auto-detects `.rigorrc.json` project configuration

#### Node.js SDK — `@rigor/sdk`

- Programmatic API wrapper for Node.js (>=16)
- Supports file-based and in-memory analysis
- Stable JSON contract for AI pipeline integration

#### Auto-Fixer — `--fix` / `--fix-dry-run`

- Automatic fixes for `focused-test` (removes `.only`) and `debug-code` (removes `console.log`, `debugger`)
- Fix metadata included in JSON and SARIF output
- Dry-run mode for previewing changes

#### Tree-Sitter Query Cache — `src/parser/queries.rs`

- Shared compiled query cache for improved performance
- Migrated rules from regex to tree-sitter AST queries
- Supports console calls, debugger statements, and focused test detection

### Enhanced MCP Server (9 tools)

Six new tools added to the existing three:

- **`analyze_with_source`** — analyzes test file with optional source file context
- **`get_improvement_plan`** — returns prioritized action plan ordered by severity
- **`explain_rule`** — explains rules with good/bad code examples
- **`iterate_improvement`** — tracks improvement across iterations with session memory
- **`get_test_template`** — generates test templates from source file exports
- **`compare_tests`** — compares two test files with score and issue breakdown

### Scoring Improvements

- Added **AI Smells** as 6th scoring category (0–25 points)
- Updated category weights per test type (unit, integration, e2e)
- Enhanced transparent breakdown with per-category and per-test scores
- Improved penalty calculation to avoid double-counting (v2 scoring)

### CLI Enhancements

- Added `--stdin` flag for analyzing test source from standard input
- Added `--stdin-filename` for virtual filename when using stdin
- Added `--fix` and `--fix-dry-run` flags for auto-fixing issues
- Enhanced JSON output with fix metadata

### Library API

- Added `analyze_source()` function for in-memory analysis
- Enhanced `analyze_file()` with improved error handling

### Reporters

- **Console** — enhanced output formatting and category breakdown
- **JSON** — added fix metadata and enhanced score breakdown
- **SARIF** — added fix information for GitHub Code Scanning integration

### Documentation

- **API Documentation** (`docs/api.md`) — JSON contract, CLI stdin usage, Rust API, Node.js SDK, exit codes
- **MCP Integration Guide** (`docs/mcp-integration.md`) — setup for Cursor, Continue, and Cline; all 9 tools documented
- **Roadmap** (`docs/roadmap.md`) — strategic roadmap through Phase 5
- **Scoring** (`docs/scoring.md`) — updated with AI Smells category

### CI/CD

- **PR Workflow** (`.github/workflows/rigor-pr.yml`) — runs Rigor on test file changes, posts score summaries as PR comments
- **Reusable GitHub Action** (`.github/actions/rigor-action/`) — configurable thresholds, SARIF upload support, score delta reporting

### Infrastructure

- Workspace restructured with `rigor-lsp` as workspace member
- Version aligned to `1.0.0` across all packages (core, LSP, VS Code extension, SDK)
- Added `tower-lsp` dependency for LSP server
- Enhanced `.rigorrc.json` support across all components
