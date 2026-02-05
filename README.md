# Rigor

[![Build](https://github.com/rigor-dev/rigor/actions/workflows/ci.yml/badge.svg)](https://github.com/rigor-dev/rigor/actions)
[![npm version](https://img.shields.io/npm/v/rigor-cli.svg)](https://www.npmjs.com/package/rigor-cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

**Fast Test Quality Linting for TypeScript** — Rigor analyzes your test files with static analysis and returns a quality score (0–100) with actionable issues. No test execution required; feedback in under 100ms.

## How Rigor Differs from Mutation Testing

| | **Rigor** | **Stryker / PIT / mutmut** |
|--|------------|----------------------------|
| **Method** | Static analysis (pattern-based) | Runtime: mutate code and run tests |
| **Speed** | Instant (~50–100ms per file) | Slow (minutes; runs full suite per mutant) |
| **Runs tests?** | No | Yes |
| **What it finds** | Weak assertions, missing error/boundary tests, flaky patterns, RTL anti-patterns, debug code | Whether tests would catch injected bugs |
| **Best for** | Fast feedback, pre-commit, CI gates, teaching good test patterns | Deep validation of test effectiveness |

Rigor is a **test quality linter**, not a traditional mutation testing tool. Use it for instant feedback; use Stryker (or similar) when you need to verify that tests actually kill mutants. They are complementary.

## Overview

Rigor analyzes TypeScript test files and returns a quality score (0-100) with specific issues and suggestions for improvement. It uses [tree-sitter](https://tree-sitter.github.io/) for fast, accurate parsing without requiring your code to compile.

### What It Checks

| Category | What It Looks For |
|----------|-------------------|
| **Assertion Quality** | Weak assertions like `toBeDefined()`, `toBeTruthy()`, snapshot-only tests; strong: `toBe()`, `toEqual()`, `toThrow()` |
| **Error Coverage** | Functions that throw errors but lack corresponding error tests |
| **Boundary Conditions** | Numeric comparisons in source code without boundary value tests |
| **Test Isolation** | Shared mutable state, missing `beforeEach` hooks, test dependencies |
| **Input Variety** | Limited test data, missing edge cases (0, null, empty values) |
| **Flaky patterns** | `Date.now()`, `Math.random()`, `setTimeout`/`setInterval`, fetch without mocks |
| **Mock abuse** | Too many mocks, mocking standard library |
| **Debug code** | `console.log`, `debugger`, focused tests (`.only`) |
| **Naming** | Vague test names ("test 1", "should work"), sequential names |
| **Async** | Missing `await` on `expect().resolves`/`.rejects` |
| **React Testing Library** | `querySelector` vs `getByRole`, `getByTestId` overuse, `fireEvent` vs `userEvent` |

## Installation

### Via npm (recommended)

```bash
npm install -g rigor-cli
# or
npx rigor-cli src/
```

The package will use a prebuilt binary when available for your platform; otherwise build from source (see below).

### From Source

```bash
git clone https://github.com/rigor-dev/rigor.git
cd rigor
cargo build --release

# Binary will be at ./target/release/rigor
```

### Add to PATH (optional)

```bash
cp target/release/rigor /usr/local/bin/
```

## Testing

**Run the unit tests:**

```bash
cd rigor
cargo test
```

**Try the CLI on the included fixtures** (from the `rigor` directory):

```bash
# Build first
cargo build --release

# Analyze a single test file (expect low score – weak assertions)
./target/release/rigor tests/fixtures/weak-assertions.test.ts

# Analyze the “good” fixture (expect higher score)
./target/release/rigor tests/fixtures/good-tests.test.ts

# Analyze all test files in fixtures
./target/release/rigor tests/fixtures/

# JSON output
./target/release/rigor tests/fixtures/weak-assertions.test.ts --json

# Quiet mode (score only)
./target/release/rigor tests/fixtures/ --quiet

# Create a config file
./target/release/rigor init --threshold 70

# Generate AI improvement prompt
./target/release/rigor tests/fixtures/weak-assertions.test.ts --fix
```

The fixtures `weak-assertions.test.ts` and `good-tests.test.ts` sit next to `auth.ts`, so Rigor can resolve the source file and report error coverage and boundary issues.

## Usage

### Basic Usage

Analyze a single test file:

```bash
rigor path/to/file.test.ts
```

Analyze all test files in a directory:

```bash
rigor src/
```

### Output Formats

**Console output** (default) - colorful, human-readable:

```
📊 Test Quality Analysis: src/auth.test.ts
   Framework: Jest | Tests: 8 | Assertions: 12

   Score: [████████████████░░░░]  82% B
   Good - Tests are solid but have room for improvement

   Score Breakdown:
   [▓▓▓▓▓▓▓▓░░] 20/25 Assertion Quality
   [▓▓▓▓▓▓▓▓▓▓] 25/25 Error Coverage
   [▓▓▓▓▓▓░░░░] 15/25 Boundary Conditions
   [▓▓▓▓▓▓▓▓▓▓] 25/25 Test Isolation
   [▓▓▓▓▓▓▓░░░] 17/25 Input Variety

   Issues Found:
   ⚠ L15:5 [weak-assertion] Weak assertion: expect(result).toBeDefined()
   ⚠ L23:5 [weak-assertion] Weak assertion: expect(valid).toBeTruthy()
```

**JSON output** - for CI/CD integration:

```bash
rigor src/auth.test.ts --json
```

```json
{
  "filePath": "src/auth.test.ts",
  "score": { "value": 82, "grade": "B" },
  "breakdown": {
    "assertionQuality": 20,
    "errorCoverage": 25,
    "boundaryConditions": 15,
    "testIsolation": 25,
    "inputVariety": 17
  },
  "issues": [...]
}
```

**Quiet mode** - just scores (with trend when history exists):

```bash
rigor src/ --quiet
```

```
src/auth.test.ts: 82 (B) [was 78, up 4]
src/user.test.ts: 91 (A)
src/api.test.ts: 67 (D) [was 72, down 5]
```

**SARIF output** - for GitHub Code Scanning and VS Code SARIF viewer:

```bash
rigor src/ --sarif > results.sarif
```

**Watch mode** - re-analyze when test files change:

```bash
rigor src/ --watch
```

### Init (create config)

Create a `.rigorrc.json` with sensible defaults:

```bash
rigor init
rigor init --threshold 75 --framework jest
rigor init --dir ./my-app
```

| Option | Description |
|--------|-------------|
| `--threshold N` | Minimum score (e.g. 70) |
| `--framework` | Force framework: jest, vitest, playwright, cypress, mocha |
| `--dir` | Directory in which to create config (default: current) |

### CLI Options (analyze)

| Option | Description |
|--------|-------------|
| `--json`, `-j` | Output results as JSON |
| `--quiet`, `-q` | Minimal output (just file: score); shows trend vs last run when history exists |
| `--verbose`, `-v` | Show all issues and suggestions |
| `--threshold N`, `-t N` | Exit with code 1 if score is below N |
| `--fix` | Generate AI prompt for improving tests |
| `--fix-output FILE` | Write AI prompt to file instead of stdout |
| `--apply` | With `--fix`: run AI suggestion and prompt to apply (requires `RIGOR_APPLY_CMD` or `ANTHROPIC_API_KEY`) |
| `--no-source` | Skip source file analysis |
| `--config PATH` | Path to config file (default: search `.rigorrc.json` in current dir and parents) |
| `--watch` | Watch for file changes and re-analyze |
| `--sarif` | Output in SARIF 2.1 format for GitHub Code Scanning |
| `--staged` | Only analyze staged (git) test files (for pre-commit) |
| `--changed` | Only analyze files changed since last commit (git diff HEAD + untracked) |
| `--mutate[=MODE]` | Run fast mutation testing: quick (10), medium (30), full. Set `RIGOR_TEST_CMD` for test command (default: npm test) |
| `--coverage PATH` | Load Istanbul/c8/nyc coverage JSON (experimental; loaded but not yet integrated into scoring) |

#### Performance Options

| Option | Description |
|--------|-------------|
| `--parallel` | Run analysis in parallel (auto-enabled for >10 files) |
| `--jobs N` | Number of parallel threads (default: CPU cores) |
| `--no-cache` | Disable caching (re-analyze all files even if unchanged) |
| `--clear-cache` | Clear the analysis cache before running |

#### Caching

Rigor caches analysis results in `.rigor-cache.json` in your project root. When a file hasn't changed (based on content hash), the cached result is used instead of re-analyzing. This significantly speeds up repeated runs.

- Cache is automatically enabled and saved after each run
- Use `--no-cache` to force re-analysis of all files
- Use `--clear-cache` to delete the cache before running

### Configuration (`.rigorrc.json`)

Place a config file in your project root (or use `--config` to point to it). CLI flags override config.

```json
{
  "threshold": 70,
  "rules": {
    "weak-assertion": "warning",
    "missing-error-test": "error",
    "flaky-pattern": "error",
    "snapshot-overuse": "off"
  },
  "ignore": ["**/*.e2e.test.ts", "**/legacy/**"],
  "framework": "auto"
}
```

- **threshold** – Minimum score (exit 1 if below). Overridden by `--threshold`.
- **rules** – Per-rule severity: `"error"`, `"warning"`, `"info"`, or `"off"`.
- **ignore** – Glob patterns for files/directories to skip.
- **framework** – `"auto"` (default), or force `"jest"`, `"vitest"`, `"playwright"`, `"cypress"`, `"mocha"`.

### Advanced Configuration

#### Config Inheritance (`extends`)

Share configuration across projects by extending a base config:

```json
{
  "extends": "./base-config.json",
  "threshold": 80
}
```

You can extend:
- Relative paths: `"./base-config.json"`, `"../shared/.rigorrc.json"`
- Absolute paths: `"/path/to/config.json"`
- npm packages: `"@company/rigor-config"` (looks in `node_modules`)

Child config values override parent values. Rules and ignore patterns are merged.

#### Source Mapping

Configure how Rigor finds source files corresponding to test files:

```json
{
  "sourceMapping": {
    "mode": "auto",
    "sourceRoot": "src",
    "testRoot": "tests",
    "mappings": {
      "tests/**/*.test.ts": "src/**/*.ts"
    }
  }
}
```

| Option | Description |
|--------|-------------|
| `mode` | `"auto"` (default) - tries common patterns; `"tsconfig"` - uses tsconfig.json paths; `"manual"` - only explicit mappings; `"off"` - disable source analysis |
| `sourceRoot` | Root directory for source files (relative to project root) |
| `testRoot` | Root directory for test files (relative to project root) |
| `mappings` | Explicit glob-based mappings: `"test-pattern": "source-pattern"` |

**Auto-detected patterns:**
- `auth.test.ts` → `auth.ts` (same directory)
- `__tests__/Button.test.tsx` → `Button.tsx` (parent directory)
- `tests/api/user.test.ts` → `src/api/user.ts` (parallel structure)
- `packages/auth/tests/auth.test.ts` → `packages/auth/src/auth.ts` (monorepo)

#### Per-Path Overrides (Monorepos)

Apply different settings to different parts of your codebase:

```json
{
  "threshold": 80,
  "overrides": [
    {
      "files": ["**/legacy/**"],
      "threshold": 50,
      "rules": { "weak-assertion": "off" }
    },
    {
      "files": ["**/*.e2e.test.ts", "**/*.e2e.spec.ts"],
      "skipSourceAnalysis": true
    },
    {
      "files": ["packages/experimental/**"],
      "threshold": 60
    }
  ]
}
```

| Override Option | Description |
|-----------------|-------------|
| `files` | Glob patterns this override applies to |
| `threshold` | Override threshold for matched files |
| `rules` | Override rule severities for matched files |
| `skipSourceAnalysis` | Skip source file analysis (useful for E2E tests) |

#### Test Root Directory

Specify a directory where tests should be searched recursively:

```json
{
  "testRoot": "tests"
}
```

When `testRoot` is set, Rigor searches for test files recursively starting from that directory relative to the project root, instead of the path provided on the command line. This is useful when all your tests are in a specific folder.

#### Custom Test Patterns

By default, Rigor recognizes these test file patterns:
- `.test.ts`, `.test.tsx`, `.test.js`, `.test.jsx`
- `.spec.ts`, `.spec.tsx`, `.spec.js`, `.spec.jsx`
- `.cy.ts`, `.cy.tsx`, `.cy.js`, `.cy.jsx` (Cypress)

If your tests use different naming conventions, override with:

```json
{
  "testPatterns": [
    ".test.ts",
    ".spec.ts",
    ".integration.ts",
    "_test.ts"
  ]
}
```

#### Full Configuration Example

```json
{
  "extends": "@company/rigor-config",
  "threshold": 75,
  "framework": "auto",
  "rules": {
    "weak-assertion": "warning",
    "missing-error-test": "error",
    "snapshot-overuse": "warning",
    "flaky-pattern": "error"
  },
  "ignore": [
    "**/node_modules/**",
    "**/dist/**",
    "**/*.generated.test.ts"
  ],
  "sourceMapping": {
    "mode": "auto",
    "sourceRoot": "src"
  },
  "testRoot": "tests",
  "testPatterns": [".test.ts", ".spec.ts"],
  "overrides": [
    {
      "files": ["**/legacy/**"],
      "threshold": 50,
      "rules": { "weak-assertion": "off", "missing-error-test": "warning" }
    },
    {
      "files": ["**/*.e2e.test.ts"],
      "skipSourceAnalysis": true
    }
  ]
}
```

### Ignore comments

Suppress issues inline:

```typescript
// rigor-ignore-next-line
expect(result).toBeDefined();

// rigor-ignore weak-assertion
expect(valid).toBeTruthy();

/* rigor-disable */
// ... block of code to ignore ...
/* rigor-enable */
```

### Pre-commit and Husky

Analyze only staged test files before commit:

```bash
rigor . --staged --threshold 70
```

**pre-commit (pre-commit.com):** Add to `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: local
    hooks:
      - id: rigor
        name: rigor
        entry: rigor . --staged --threshold 70
        language: system
        files: \.(test|spec)\.(ts|tsx|js|jsx)$
```

**Husky:** In `.husky/pre-commit`:

```bash
#!/bin/sh
rigor . --staged --threshold 70 || exit 1
```

### CI/CD Integration

Use `--threshold` to fail builds when test quality drops:

```bash
# Fail if any test file scores below 70
rigor src/ --threshold 70 --quiet

# Exit codes:
# 0 - All files pass threshold
# 1 - One or more files below threshold
# 2 - Error (file not found, parse error, etc.)
```

#### GitHub Action (recommended)

Use the official action to run Rigor and post a quality report on PRs:

```yaml
- uses: rigor-dev/rigor-action@v1
  with:
    path: src
    threshold: 75
    comment: true
    upload-sarif: true
```

See [github-action/README.md](../github-action/README.md) for full options.

#### GitHub Actions (manual)

```yaml
- name: Check Test Quality
  run: |
    rigor src/ --threshold 75 --json > test-quality.json

- name: Upload Report
  uses: actions/upload-artifact@v3
  with:
    name: test-quality-report
    path: test-quality.json
```

#### GitHub Code Scanning (SARIF)

```yaml
- name: Run Rigor
  run: rigor src/ --sarif > rigor-results.sarif

- name: Upload SARIF
  uses: github/codeql-action/upload-sarif@v2
  with:
    sarif_file: rigor-results.sarif
```

## Test relevance: valid vs. meaningful

**Valid** tests are well-formed (correct syntax, no obvious anti-patterns). Rigor’s static analysis and score cover this: assertion quality, isolation, error coverage, boundary checks, etc.

**Relevant / meaningful** tests actually test what they claim to test and would catch real bugs. Rigor addresses this in two ways:

### 1. Static relevance (analyzer rules)

These rules flag tests that are not *meaningful* even when they run and have assertions:

- **Assertion–intent mismatch** (`assertion-intent-mismatch`): The test name implies a specific outcome (e.g. “returns 404”, “throws when invalid”, “is empty”) but no assertion actually verifies it. Example: a test named “returns 404 when not found” that only does `expect(res).toBeDefined()` — it doesn’t test what it says.
- **Trivial assertions** (`trivial-assertion`): Assertions that always pass and don’t verify behavior (e.g. `expect(1).toBe(1)`, `expect(true).toBe(true)`). Such tests add noise and false confidence.

Use these to catch tests that “make no sense” or don’t verify what their name claims.

### 2. Mutation-driven relevance (`--mutate`)

When mutants **survive** (tests still pass after mutating the source), that means tests did not react to that behavior — i.e. they are not relevant to those code paths.

1. **Run mutation testing** on a single test file that has a mapped source file:
   ```bash
   rigor path/to/file.test.ts --mutate
   ```
   Rigor mutates the source, runs your tests, and reports how many mutants were **killed** (tests failed) vs. **survived** (tests still passed).

   **Mutation operators (28 total):**

   | Category | Mutations |
   |----------|-----------|
   | **Boundary** | `>=` → `>`, `<=` → `<`, `>` → `>=`, `<` → `<=` |
   | **Boolean** | `true` → `false`, `false` → `true` |
   | **Arithmetic** | `+` → `-`, `-` → `+`, `*` → `/` |
   | **Equality** | `===` → `!=`, `!==` → `==` |
   | **String** | `"string"` → `""`, `""` → `" "` (single quotes too) |
   | **Array** | `[a, b]` → `[]`, `[]` → `[0]`, `[0]` → `[1]` |
   | **Return** | `return x` → `return null`, `return x` → `return undefined` |
   | **Increment** | `++` → `--`, `--` → `++`, `+= 1` → `-= 1`, `-= 1` → `+= 1` |
   | **TypeScript** | `?.` → `.` (optional chaining), `??` → `\|\|` (nullish coalescing), `!.` → `.` (non-null assertion) |

2. **Use the relevance section** in the output when mutants survive. It reports:
   - Which **source lines** had at least one survived mutant (tests did not catch that change).
   - **Suggestions** by mutation type (e.g. “Assert on the exact return value so null/undefined mutants are caught”, “Add boundary tests so comparison mutants are caught”).

3. **Improve tests** so they assert on the exact outcomes (return value, boundaries, array length, etc.). Then run `--mutate` again; the kill rate should go up and the “Test relevance” list should shrink.

**Summary:** Use the **quality score** for “are my tests valid and well-structured?” Use **static relevance rules** for “does this test verify what it claims?” and **mutation testing + relevance** for “do my tests actually catch relevant bugs?”

## Scoring

### Grade Scale

| Grade | Score | Description |
|-------|-------|-------------|
| **A** | 90-100 | Excellent - Tests are well-structured with strong assertions |
| **B** | 80-89 | Good - Tests are solid but have room for improvement |
| **C** | 70-79 | Fair - Tests provide basic coverage but need strengthening |
| **D** | 60-69 | Poor - Tests have significant quality issues |
| **F** | 0-59 | Failing - Tests need major improvements |

### Formal Scoring Formula

The final score \( S \) is computed as:

\[
S = \max\left(0,\; S_{\text{weighted}} - P\right)
\]

Where:

**1. Weighted Category Score** \( S_{\text{weighted}} \)

\[
S_{\text{weighted}} = \frac{1}{25} \sum_{i=1}^{5} C_i \cdot W_i
\]

- \( C_i \) = Category score (0–25) for category \( i \)
- \( W_i \) = Weight for category \( i \) based on test type (see table below)

**2. Issue Penalty** \( P \)

\[
P = \min(E \cdot 5,\, 35) + \min(W \cdot 2,\, 40) + \min(I \cdot 1,\, 15)
\]

- \( E \) = Number of Error-severity issues
- \( W \) = Number of Warning-severity issues
- \( I \) = Number of Info-severity issues

The penalty is capped to prevent extreme deductions: max 35 from errors, 40 from warnings, 15 from info (90 total max penalty).

**3. Category Weights by Test Type**

| Category | Unit | E2E | Component | Integration |
|----------|------|-----|-----------|-------------|
| Assertion Quality | 25 | 35 | 30 | 25 |
| Error Coverage | 20 | 15 | 15 | 20 |
| Boundary Conditions | 25 | 5 | 15 | 15 |
| Test Isolation | 15 | 25 | 20 | 20 |
| Input Variety | 15 | 20 | 20 | 20 |
| **Total** | 100 | 100 | 100 | 100 |

Weights are tuned per test type: E2E tests de-emphasize boundary conditions (which are less relevant to user-flow tests) and emphasize assertion quality and isolation; unit tests balance boundary conditions and assertions equally.

**4. Worked Example**

A **unit test** file with these category scores and issues:

| Category | Score |
|----------|-------|
| Assertion Quality | 20 |
| Error Coverage | 18 |
| Boundary Conditions | 15 |
| Test Isolation | 22 |
| Input Variety | 17 |

Issues: 0 errors, 3 warnings, 2 info

**Step 1: Weighted score** (using unit test weights)

\[
S_{\text{weighted}} = \frac{(20 \times 25) + (18 \times 20) + (15 \times 25) + (22 \times 15) + (17 \times 15)}{25} = \frac{500 + 360 + 375 + 330 + 255}{25} = \frac{1820}{25} = 72.8
\]

**Step 2: Issue penalty**

\[
P = \min(0 \times 5, 35) + \min(3 \times 2, 40) + \min(2 \times 1, 15) = 0 + 6 + 2 = 8
\]

**Step 3: Final score**

\[
S = \max(0, 72.8 - 8) = 64.8 \approx 65 \quad \Rightarrow \quad \text{Grade: } \mathbf{D}
\]

### Category Breakdown

Each category contributes up to 25 points:

**Assertion Quality (0-25)**
- Strong: `toBe()`, `toEqual()`, `toStrictEqual()`, `toThrow()`, `toHaveBeenCalledTimes()`, `toHaveText()` (Playwright), `should('have.text')`, `should('have.length')`, `should('eq')`, `should('have.attr')` (Cypress)
- Moderate: `toContain()`, `toMatch()`, `toHaveLength()`, `toBeInstanceOf()`, `toHaveClass()` (RTL), `toBeVisible()` (Playwright), `should('be.visible')`, `should('contain')`, `should('be.disabled')` (Cypress)
- Weak: `toBeDefined()`, `toBeTruthy()`, `toBeFalsy()`, `not.toBeNull()`, `toMatchSnapshot()`, `toMatchInlineSnapshot()`, `should('exist')` (Cypress)
- Snapshot overuse: file with >50% snapshot assertions, or tests that only use snapshots

**Error Coverage (0-25)**
- Checks if functions that throw have corresponding error tests
- Looks for `toThrow()` and `rejects.toThrow()` assertions

**Boundary Conditions (0-25)**
- Analyzes source code for numeric comparisons (`>=`, `<=`, `<`, `>`)
- Checks if tests cover boundary values (e.g., for `age >= 18`, tests 17, 18, 19)

**Test Isolation (0-25)**
- Detects mutable module-level variables
- Checks for `beforeEach`/`afterEach` hooks
- Identifies tests that depend on execution order

**Input Variety (0-25)**
- Checks for diverse test inputs
- Flags missing edge cases: 0, negative numbers, empty strings, null

## Rules Reference

| Rule | Severity | Description |
|------|----------|-------------|
| `weak-assertion` | Warning | Assertion doesn't verify a specific value |
| `no-assertions` | Error | Test has no expect() calls |
| `empty-test` | Error | Test block has no body or statements |
| `skipped-test` | Info | Test is marked with .skip or .todo |
| `snapshot-overuse` | Warning | File or test uses only snapshots; >50% snapshot assertions |
| `missing-error-test` | Warning | Throwable function lacks error test |
| `missing-boundary-test` | Warning | Boundary condition not tested |
| `shared-state` | Warning | Mutable state shared between tests |
| `duplicate-test` | Error | Multiple tests with same name |
| `limited-input-variety` | Info | Test inputs lack diversity |
| `hardcoded-values` | Info | Test uses hardcoded data like emails |
| `debug-code` | Info/Warning | `console.log`/`debug`/`warn`, `debugger` in tests |
| `focused-test` | Warning | Test uses `.only` (it.only, fit, etc.) |
| `flaky-pattern` | Warning/Info | `Date.now()`, `Math.random()`, timers, fetch without mocks |
| `mock-abuse` | Warning | Too many mocks (>5), mocking standard library |
| `vague-test-name` | Warning/Info | Vague names ("test 1", "should work"), sequential names |
| `missing-await` | Warning/Info | `expect().resolves`/`.rejects` without `await`; async test with no await |
| `rtl-prefer-screen` | Warning | `container.querySelector` instead of screen/getByRole (RTL) |
| `rtl-prefer-semantic` | Info | `getByTestId` over semantic queries (RTL) |
| `rtl-prefer-user-event` | Info | `fireEvent` instead of `userEvent` (RTL) |
| `mutation-resistant` | Info | Assertion may let mutants survive (e.g. toBeGreaterThan(0) vs toBe(3)) |
| `boundary-specificity` | Info | Boundary/edge test doesn't assert exact value |
| `state-verification` | Info | Test may have side effects but only checks return value |
| `assertion-intent-mismatch` | Warning | Test name suggests an outcome (returns X, throws, 404, empty) but no assertion verifies it |
| `trivial-assertion` | Warning/Error | Assertion always passes and doesn't verify behavior (e.g. expect(1).toBe(1)) |
| `return-path-coverage` | Warning | Return paths in source file not covered by test assertions |
| `behavioral-completeness` | Info | Test only verifies partial behavior (e.g., checks one field but not others) |
| `side-effect-not-verified` | Info | Function has side effects (writes, API calls) but test doesn't verify them |

RTL rules run only when `@testing-library/react` (or `@testing-library/dom`) is imported.

## AI-Assisted Fixes

Generate a prompt to improve your tests with AI:

```bash
rigor src/auth.test.ts --fix
```

This outputs a structured prompt containing:
- Current test file content
- Source file under test (if found)
- All identified issues
- Improvement instructions

Save to a file for use with your preferred AI tool:

```bash
rigor src/auth.test.ts --fix --fix-output prompt.md
```

### Auto-Apply with `--apply`

Use `--fix --apply` to automatically generate and apply AI suggestions. There are two ways to use this:

**Option 1: Built-in Claude Integration**

Build Rigor with the AI feature enabled and set your API key:

```bash
# Build with AI feature
cargo build --release --features ai

# Set your Anthropic API key
export ANTHROPIC_API_KEY="sk-ant-..."

# Run with auto-apply
rigor src/auth.test.ts --fix --apply
```

Rigor will call the Claude API, show the suggested improvements, and prompt you to apply them.

**Option 2: Custom Command**

Set `RIGOR_APPLY_CMD` to a command that reads the prompt on stdin and prints the improved code:

```bash
export RIGOR_APPLY_CMD="node my-openai-script.js"
rigor src/auth.test.ts --fix --apply
```

The command receives the full improvement prompt on stdin and should output the improved test file code.

## Framework Support

Rigor automatically detects these test frameworks:

- **Jest** - Default for most React/Node projects
- **Vitest** - Vite-native testing
- **Playwright** - E2E testing (assertions like `toBeVisible()`, `toHaveText()`)
- **Cypress** - E2E testing (assertions like `cy.get().should('exist')`, `should('have.text')`, `should('be.visible')`)
- **Mocha** - Classic test runner
- **React Testing Library** - When `@testing-library/react` is imported; enables RTL-specific rules

Detection is based on imports and code patterns. The framework affects how certain patterns are interpreted.

## Test Type Classification

Rigor classifies tests into four types, which affects scoring weights (see [Scoring](#scoring)):

| Type | How It's Detected |
|------|-------------------|
| **E2E** | File path contains `e2e`, `.e2e.`, `/e2e/`, `.cy.`, `/cypress/`; or Playwright/Cypress framework detected |
| **Integration** | File path contains `integration` or `.integration.` |
| **Component** | File path contains `component` or `.component.`; or `@testing-library`, `render()`, `screen.get` patterns found |
| **Unit** | Default fallback when no other type matches |

Test type affects which categories are emphasized in scoring. For example, E2E tests de-emphasize boundary conditions (less relevant to user flows) while emphasizing assertion quality and test isolation.

## MCP Server (Claude / Cursor)

Run Rigor as an MCP server for AI assistants:

```bash
rigor mcp
```

Configure Cursor (or another MCP client) to start the server with command `rigor mcp`. The server exposes:

- **analyze_test_quality** — Analyze a test file and return score and issues (input: `file` path).
- **suggest_improvements** — Generate an AI improvement prompt for a test file (input: `file` path).
- **get_mutation_score** — Run fast mutation testing and return kill rate (input: `file` path, optional `count`).

## Trend tracking

When you run Rigor, it can store results in `.rigor-history.json` in your project root (found by walking up until `package.json`, `.git`, or `.rigor-history.json`). In **quiet mode**, the output then includes deltas vs the last run:

```
src/auth.test.ts: 78 (B) [was 82, down 4]
```

History is written after each run (normal and quiet). The file keeps the last 50 runs with timestamp and per-file scores.

## Examples

### Weak Assertions (Before)

```typescript
it('should authenticate user', () => {
  const result = authenticate('user@example.com', 'password');
  expect(result).toBeDefined();  // ⚠️ Weak - only checks existence
  expect(result.success).toBeTruthy();  // ⚠️ Weak - only checks truthiness
});
```

### Strong Assertions (After)

```typescript
it('should authenticate user with valid credentials', () => {
  const result = authenticate('user@example.com', 'correctPassword');
  expect(result.success).toBe(true);  // ✓ Checks specific value
  expect(result.user.email).toBe('user@example.com');  // ✓ Verifies data
  expect(result.token).toMatch(/^[a-z0-9]{64}$/);  // ✓ Pattern validation
});
```

### Missing Error Tests (Before)

```typescript
// Source: throws AuthError for invalid credentials
// Tests: No error case covered ⚠️

it('should authenticate valid user', () => {
  const result = authenticate('user@example.com', 'correct');
  expect(result.success).toBe(true);
});
```

### With Error Coverage (After)

```typescript
it('should authenticate valid user', () => {
  const result = authenticate('user@example.com', 'correct');
  expect(result.success).toBe(true);
});

it('should throw AuthError for invalid credentials', () => {  // ✓ Error case
  expect(() => authenticate('user@example.com', 'wrong'))
    .toThrow(AuthError);
});

it('should throw AuthError for empty email', () => {  // ✓ Edge case
  expect(() => authenticate('', 'password'))
    .toThrow('Email is required');
});
```

### Test Isolation (Before)

```typescript
let sessions: Session[] = [];  // ⚠️ Shared mutable state

it('creates session', () => {
  sessions.push(createSession('user1'));
  expect(sessions).toHaveLength(1);
});

it('creates another session', () => {
  sessions.push(createSession('user2'));
  expect(sessions).toHaveLength(2);  // ⚠️ Depends on previous test
});
```

### Isolated Tests (After)

```typescript
describe('Session Management', () => {
  let sessions: Session[];

  beforeEach(() => {  // ✓ Reset before each test
    sessions = [];
  });

  it('creates session', () => {
    sessions.push(createSession('user1'));
    expect(sessions).toHaveLength(1);
  });

  it('creates another session', () => {
    sessions.push(createSession('user2'));
    expect(sessions).toHaveLength(1);  // ✓ Independent
  });
});
```

## Troubleshooting

### "No source file found" for --mutate

Mutation testing requires a mapped source file. Rigor uses naming conventions to find sources:
- `auth.test.ts` → `auth.ts` (same directory)
- `__tests__/Button.test.tsx` → `Button.tsx` (parent directory)
- `tests/api/user.test.ts` → `src/api/user.ts` (parallel structure)

**Solutions:**
1. Ensure source file exists with matching name (without `.test`/`.spec`)
2. Configure explicit mappings in `.rigorrc.json`:
   ```json
   {
     "sourceMapping": {
       "mode": "manual",
       "mappings": { "tests/**/*.test.ts": "src/**/*.ts" }
     }
   }
   ```
3. E2E tests (Cypress/Playwright) typically don't have single source files — this warning is expected

### Framework detected incorrectly

Force the framework in config:
```json
{ "framework": "vitest" }
```

Or check that your imports are explicit (e.g., `import { vi } from 'vitest'`).

### Cache issues

If you suspect stale results:
```bash
rigor src/ --clear-cache
# or
rigor src/ --no-cache
```

### Score seems wrong

- Use `--verbose` to see all issues and how each affects the score
- Check the test type classification (E2E tests use different weights)
- Verify the source file was found (affects error/boundary coverage scoring)

### AI --apply not working

Ensure one of:
1. Built with `--features ai` AND `ANTHROPIC_API_KEY` is set
2. `RIGOR_APPLY_CMD` is set to a valid command

## Limitations

- **Static analysis only** - Cannot detect runtime issues; mock/flaky detection is heuristic (e.g. looks for `useFakeTimers`, `jest.mock`)
- **TypeScript/JavaScript** - Does not support other languages
- **Heuristic-based** - May produce false positives for complex patterns
- **Source mapping** - Relies on naming conventions to find source files
- **Watch mode** - Requires a supported filesystem (uses `notify`); best used from project root
- **Coverage integration** - `--coverage` loads Istanbul data but is not yet integrated into scoring (experimental)

## License

MIT
