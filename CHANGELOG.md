# Changelog

All notable changes to Rigor will be documented in this file.

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

#### Critical Quality Rules (11 new)

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
