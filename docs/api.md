# Programmatic API (Phase 2.4)

Rigor exposes a **stable JSON contract** for embedding in AI pipelines and tooling: input is test source (file path or stdin) plus optional config; output is score, issues, and per-issue suggestions.

## Invocation

### 1. File path (CLI)

```bash
rigor path/to/test.test.ts --json
rigor path/to/test.test.ts --json --threshold 80
```

- **Input:** File path(s); config from `.rigorrc.json` (and optional `--config`).
- **Output:** JSON to stdout (see [Output schema](#output-schema)).

### 2. Stdin (CLI, for in-memory source)

```bash
echo "$TEST_SOURCE" | rigor --stdin --json
echo "$TEST_SOURCE" | rigor --stdin --stdin-filename my.test.ts --json
```

- **Input:** Test file content on stdin. Optional `--stdin-filename` (default: `stdin.test.ts`) sets the virtual path used for parser (`.ts` vs `.tsx`) and config/test-type detection.
- **Output:** Same JSON as file-based run. Source file mapping is not performed (no source-dependent rules when using stdin).

### 3. Rust library

```rust
use rigor::analyzer::AnalysisEngine;
use std::path::Path;

// From path (same as CLI file mode)
let engine = AnalysisEngine::new();
let result = engine.analyze(Path::new("tests/auth.test.ts"), config.as_ref())?;

// From string (same as CLI stdin mode)
let result = engine.analyze_source(
    &test_source_string,
    Path::new("stdin.test.ts"),
    config.as_ref(),
)?;
```

- **Input:** Either a file path + optional `Config`, or test source string + virtual path + optional `Config`.
- **Output:** `AnalysisResult` (same shape as JSON output).

## Output schema

JSON output is the **AnalysisResult** structure (camelCase keys). Single-file runs emit one object; directory runs can emit an array or a wrapper with `results` and `summary` (see `--json` with a directory).

### Top-level fields

| Field | Type | Description |
|-------|------|-------------|
| `filePath` | string | Path to the test file (or virtual path when using `--stdin`) |
| `score` | object | `{ value: number (0-100), grade: string ("A"\|"B"\|"C"\|"D"\|"F") }` |
| `breakdown` | object | Per-category raw scores (each 0–25): `assertionQuality`, `errorCoverage`, `boundaryConditions`, `testIsolation`, `inputVariety`, `aiSmells` |
| `transparentBreakdown` | object? | Optional weights, penalties, and category breakdown |
| `testScores` | array? | Per-test score and issues when available |
| `issues` | array | List of [Issue](#issue) objects |
| `stats` | object | `totalTests`, `totalAssertions`, `skippedTests`, etc. |
| `framework` | string | Detected framework (e.g. `"Jest"`, `"Vitest"`) |
| `testType` | string | `"Unit"`, `"E2e"`, `"Component"`, `"Integration"` |
| `sourceFile` | string? | Path to mapped source file (null when using `--stdin`) |

### Issue

| Field | Type | Description |
|-------|------|-------------|
| `rule` | string | Rule id (e.g. `"weak-assertion"`, `"ai-smell-tautological-assertion"`) |
| `severity` | string | `"Error"`, `"Warning"`, `"Info"` |
| `message` | string | Human-readable message |
| `location` | object | `{ line: number, column: number }` (1-based) |
| `suggestion` | string? | How to fix (actionable text) |
| `fix` | object? | Optional auto-fix: `{ startLine, startColumn, endLine, endColumn, replacement }` |

Each issue’s `suggestion` (and optional `fix`) is the “improvement instruction” for that finding. For a full improvement prompt (e.g. for an LLM), use the MCP tool `get_improvement_plan` or the `--suggest` CLI flag.

## Exit codes

| Code | Meaning |
|------|--------|
| 0 | Success; score meets threshold (if `--threshold` set) |
| 1 | Score below threshold |
| 2 | No test files found, or file/read error |

## Versioning

The JSON shape is considered **stable** for the same major version. New optional fields may be added; breaking changes will be reflected in the major version (see roadmap).

## SDKs

- **Node.js:** Use the CLI with `--json` or `--stdin --json` and parse stdout. A thin wrapper package `@rigor/sdk` (or local `sdk/node`) can spawn `rigor` and return the parsed result.
- **Python:** Same idea: invoke `rigor --json` or `rigor --stdin --json` with stdin content and parse JSON from stdout.

See the repo for an example Node.js wrapper in `sdk/node` (if present).
