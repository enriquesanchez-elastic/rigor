# CI Integration

## GitHub Actions

### Basic Setup

```yaml
name: Test Quality
on: [push, pull_request]

jobs:
  rigor:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Check Test Quality
        run: npx rigor-cli src/ --threshold 75
```

### With Artifacts

```yaml
- name: Run Rigor
  run: npx rigor-cli src/ --json > test-quality.json

- name: Upload Report
  uses: actions/upload-artifact@v4
  with:
    name: test-quality-report
    path: test-quality.json
```

### GitHub Code Scanning (SARIF)

```yaml
- name: Run Rigor
  run: npx rigor-cli src/ --sarif > rigor.sarif

- name: Upload SARIF
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: rigor.sarif
```

Issues appear in the Security tab and as PR annotations.

### Official Action

```yaml
- uses: rigor-dev/rigor-action@v1
  with:
    path: src
    threshold: 75
    comment: true        # Comment on PR
    upload-sarif: true   # Upload to Code Scanning
```

## Pre-commit Hooks

### With pre-commit (pre-commit.com)

Add to `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: local
    hooks:
      - id: rigor
        name: rigor
        entry: npx rigor-cli . --staged --threshold 70
        language: system
        files: \.(test|spec)\.(ts|tsx|js|jsx)$
        pass_filenames: false
```

### With Husky

Install:
```bash
npm install -D husky
npx husky init
```

Add to `.husky/pre-commit`:
```bash
#!/bin/sh
npx rigor-cli . --staged --threshold 70 || exit 1
```

### With lint-staged

```json
{
  "lint-staged": {
    "*.test.{ts,tsx}": "rigor --threshold 70"
  }
}
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | All files pass threshold |
| 1 | One or more files below threshold |
| 2 | Error (file not found, parse error, etc.) |

## Caching in CI

Rigor caches results in `.rigor-cache.json`. To speed up CI:

```yaml
- uses: actions/cache@v4
  with:
    path: .rigor-cache.json
    key: rigor-${{ hashFiles('**/*.test.ts') }}
```

## GitLab CI

```yaml
test-quality:
  image: node:20
  script:
    - npx rigor-cli src/ --threshold 75 --json > report.json
  artifacts:
    reports:
      codequality: report.json
```

## CircleCI

```yaml
jobs:
  test-quality:
    docker:
      - image: cimg/node:20.0
    steps:
      - checkout
      - run: npx rigor-cli src/ --threshold 75
```

## Azure Pipelines

```yaml
- task: Npm@1
  inputs:
    command: custom
    customCommand: 'exec rigor-cli src/ --threshold 75'
```

## Tips

1. **Start with a low threshold** (e.g., 60) and increase over time
2. **Use `--staged`** in pre-commit to only check changed files
3. **Use `--quiet`** in CI for concise output
4. **Save JSON reports** for trend tracking
5. **Upload SARIF** to see issues directly in GitHub
