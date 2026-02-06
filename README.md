# Rigor

**Fast test quality linting for TypeScript.** Analyzes your tests with static analysis and returns a score (0–100) with actionable issues. No test execution required.

## Why Rigor?

| | Rigor | Stryker / PIT / mutmut |
|--|-------|------------------------|
| **Speed** | ~50–100ms per file | Minutes (runs full test suite per mutant) |
| **Runs tests?** | No | Yes |
| **Best for** | Fast feedback, CI gates, pre-commit | Deep validation of test effectiveness |

Rigor is a **test linter**, not mutation testing. Use it for instant feedback; use Stryker when you need to verify tests actually kill mutants.

## Installation

```bash
npm install -g rigor-cli
# or
npx rigor-cli src/
```

**Pre-built binaries:**

Download from [GitHub Releases](https://github.com/enriquesanchez-elastic/rigor/releases):
- Linux (x86_64, ARM64)
- macOS (Intel, Apple Silicon)
- Windows (x86_64)

**From source:**
```bash
git clone https://github.com/enriquesanchez-elastic/rigor.git && cd rigor
cargo build --release
./target/release/rigor --help
```

## Quick Start

```bash
# Analyze a file
rigor src/auth.test.ts

# Analyze a directory
rigor src/

# Fail CI if score < 70
rigor src/ --threshold 70

# JSON output
rigor src/ --json

# Watch mode
rigor src/ --watch
```

## What It Checks

| Category | Examples |
|----------|----------|
| **Assertion Quality** | Flags `toBeDefined()`, `toBeTruthy()`, snapshot-only tests |
| **Error Coverage** | Functions that throw but lack error tests |
| **Boundary Conditions** | Numeric comparisons without boundary tests |
| **Test Isolation** | Shared mutable state, missing `beforeEach` |
| **Flaky Patterns** | `Date.now()`, `Math.random()`, unmocked fetch |
| **React Testing Library** | `querySelector` vs `getByRole`, `fireEvent` vs `userEvent` |

See [docs/rules.md](docs/rules.md) for the full list of 34 rules.

## Output

```
📊 Test Quality Analysis: src/auth.test.ts
   Framework: Jest | Tests: 8 | Assertions: 12

   Score: 82/100 (B)

   Issues Found:
   ⚠ L15:5 [weak-assertion] expect(result).toBeDefined() - use toBe() or toEqual()
   ⚠ L23:5 [missing-error-test] authenticate() throws but no error test found
```

## Scoring

| Grade | Score | Meaning |
|-------|-------|---------|
| A | 90-100 | Excellent |
| B | 80-89 | Good |
| C | 70-79 | Fair |
| D | 60-69 | Poor |
| F | 0-59 | Needs work |

Score is based on six categories (assertion quality, error coverage, boundary conditions, test isolation, input variety, AI smells) minus penalties for issues found. See [docs/scoring.md](docs/scoring.md) for details.

## Configuration

Create `.rigorrc.json`:

```json
{
  "threshold": 70,
  "rules": {
    "weak-assertion": "warning",
    "snapshot-overuse": "off"
  },
  "ignore": ["**/e2e/**", "**/legacy/**"]
}
```

Or run `rigor init` to generate one.

See [docs/configuration.md](docs/configuration.md) for all options.

## CLI Reference

```
rigor <path>              Analyze test file(s)
rigor init                Create .rigorrc.json
rigor mcp                 Run MCP server for AI assistants

Options:
  -j, --json              JSON output
  -q, --quiet             Minimal output (scores only)
  -v, --verbose           Show all issues
  -t, --threshold <N>     Exit 1 if score below N
  --fix                   Apply auto-fixes where possible
  --suggest               Generate AI improvement prompt
  --watch                 Re-analyze on file changes
  --sarif                 SARIF output for GitHub Code Scanning
  --staged                Only analyze git staged files
  --changed               Only analyze git changed files
  --mutate [MODE]         Run mutation testing (quick/medium/full)
  --parallel              Parallel analysis
  --no-cache              Skip cache
```

## CI Integration

```yaml
# GitHub Actions
- name: Check Test Quality
  run: npx rigor-cli src/ --threshold 75
```

```bash
# Pre-commit
rigor . --staged --threshold 70
```

See [docs/ci-integration.md](docs/ci-integration.md) for GitHub Actions, SARIF, and Husky setup.

## Mutation Testing

Rigor includes lightweight mutation testing:

```bash
rigor src/auth.test.ts --mutate
```

This mutates the source file (e.g., `>=` → `>`, `return x` → `return null`), runs your tests, and reports how many mutants were killed vs survived.

See [docs/mutation-testing.md](docs/mutation-testing.md) for operators and usage.

## Development

```bash
# Build
cargo build --release

# Run tests
cargo test --all-features

# Coverage (requires llvm-tools-preview and cargo-llvm-cov)
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov
# Generate coverage (runs test suite and writes .profraw into target/). Do this first.
cargo llvm-cov test --all-features --lcov --output-path lcov.info
# Then, in the same session (don't clear target/), view the report:
cargo llvm-cov report                    # summary in terminal
cargo llvm-cov report --html             # open target/llvm-cov/html/index.html
```

CI runs coverage on every push; the lcov report is uploaded as a workflow artifact.

## Documentation

- [Configuration](docs/configuration.md) - All config options, extends, overrides
- [Rules Reference](docs/rules.md) - All 34 rules with descriptions
- [Scoring](docs/scoring.md) - How scores are calculated
- [Mutation Testing](docs/mutation-testing.md) - Operators and usage
- [CI Integration](docs/ci-integration.md) - GitHub Actions, pre-commit, SARIF
- [Troubleshooting](docs/troubleshooting.md) - Common issues

## License

MIT
