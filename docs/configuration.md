# Configuration

Rigor looks for `.rigorrc.json` in the current directory and parent directories.

## Basic Config

```json
{
  "threshold": 70,
  "framework": "auto",
  "rules": {
    "weak-assertion": "warning",
    "missing-error-test": "error",
    "snapshot-overuse": "off"
  },
  "ignore": ["**/node_modules/**", "**/dist/**"]
}
```

## Options

| Option | Type | Description |
|--------|------|-------------|
| `threshold` | number | Minimum score (0-100). Exit 1 if below. |
| `framework` | string | `auto`, `jest`, `vitest`, `playwright`, `cypress`, `mocha` |
| `rules` | object | Per-rule severity: `error`, `warning`, `info`, `off` |
| `ignore` | array | Glob patterns to skip |
| `testRoot` | string | Directory to search for tests |
| `testPatterns` | array | Custom test file patterns (default: `.test.ts`, `.spec.ts`, etc.) |

## Config Inheritance

Share config across projects:

```json
{
  "extends": "./base-config.json",
  "threshold": 80
}
```

You can extend:
- Relative paths: `"./base-config.json"`
- Absolute paths: `"/path/to/config.json"`
- npm packages: `"@company/rigor-config"`

Child values override parent. Rules and ignore patterns are merged.

## Source Mapping

Configure how Rigor finds source files for test files:

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
| `mode` | `auto` (default), `tsconfig`, `manual`, `off` |
| `sourceRoot` | Source directory relative to project root |
| `testRoot` | Test directory relative to project root |
| `mappings` | Explicit glob mappings |

**Auto-detected patterns:**
- `auth.test.ts` → `auth.ts` (same directory)
- `__tests__/Button.test.tsx` → `Button.tsx` (parent directory)
- `tests/api/user.test.ts` → `src/api/user.ts` (parallel structure)

## Per-Path Overrides

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
      "files": ["**/*.e2e.test.ts"],
      "skipSourceAnalysis": true
    }
  ]
}
```

| Option | Description |
|--------|-------------|
| `files` | Glob patterns this override applies to |
| `threshold` | Override threshold |
| `rules` | Override rule severities |
| `skipSourceAnalysis` | Skip source file analysis (useful for E2E) |

## Custom Test Patterns

Default patterns: `.test.ts`, `.test.tsx`, `.spec.ts`, `.spec.tsx`, `.cy.ts`, etc.

Override with:

```json
{
  "testPatterns": [".test.ts", ".spec.ts", ".integration.ts", "_test.ts"]
}
```

## CLI Overrides

CLI flags override config file values:

```bash
rigor src/ --threshold 80 --config ./custom-config.json
```

## Full Example

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
      "rules": { "weak-assertion": "off" }
    },
    {
      "files": ["**/*.e2e.test.ts"],
      "skipSourceAnalysis": true
    }
  ]
}
```

## Inline Ignores

Suppress issues in code:

```typescript
// rigor-ignore-next-line
expect(result).toBeDefined();

// rigor-ignore weak-assertion
expect(valid).toBeTruthy();

/* rigor-disable */
// ... block of code to ignore ...
/* rigor-enable */
```
