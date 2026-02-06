# Rules Reference

Rigor includes 34 active rules across several categories. Ten Phase 2.2 rules (test-complexity, implementation-coupling, vacuous-test, incomplete-mock-verification, async-error-mishandling, redundant-test, unreachable-test-code, excessive-setup, type-assertion-abuse, missing-cleanup) are planned but not yet implemented and are excluded from scoring.

## Assertion Quality

| Rule | Severity | Description |
|------|----------|-------------|
| `weak-assertion` | Warning | Assertion doesn't verify a specific value (`toBeDefined`, `toBeTruthy`, etc.) |
| `no-assertions` | Error | Test has no `expect()` calls |
| `empty-test` | Error | Test block has no body |
| `snapshot-overuse` | Warning | >50% snapshot assertions, or snapshot-only tests |
| `trivial-assertion` | Warning | Always-passing assertion (e.g., `expect(1).toBe(1)`) |

## Coverage

| Rule | Severity | Description |
|------|----------|-------------|
| `missing-error-test` | Warning | Function throws but no error test exists |
| `missing-boundary-test` | Warning | Numeric comparison (`>=`, `<`, etc.) not tested at boundary |
| `return-path-coverage` | Warning | Return paths in source not covered by tests |
| `behavioral-completeness` | Info | Test only verifies partial behavior |
| `side-effect-not-verified` | Info | Function has side effects but test doesn't verify them |

## Test Quality

| Rule | Severity | Description |
|------|----------|-------------|
| `shared-state` | Warning | Mutable state shared between tests |
| `duplicate-test` | Error | Multiple tests with same name |
| `skipped-test` | Info | Test marked with `.skip` or `.todo` |
| `limited-input-variety` | Info | Test inputs lack diversity |
| `hardcoded-values` | Info | Hardcoded data like emails |
| `vague-test-name` | Warning | Names like "test 1", "should work" |

## Debug & Focus

| Rule | Severity | Description |
|------|----------|-------------|
| `debug-code` | Warning | `console.log`, `debugger` in tests |
| `focused-test` | Warning | Test uses `.only` (`it.only`, `fit`, etc.) |

## Async

| Rule | Severity | Description |
|------|----------|-------------|
| `missing-await` | Warning | `expect().resolves`/`.rejects` without `await` |
| `flaky-pattern` | Warning | `Date.now()`, `Math.random()`, timers, unmocked fetch |

## Mocking

| Rule | Severity | Description |
|------|----------|-------------|
| `mock-abuse` | Warning | Too many mocks (>5), mocking standard library |

## React Testing Library

These rules only run when `@testing-library/react` or `@testing-library/dom` is imported.

| Rule | Severity | Description |
|------|----------|-------------|
| `rtl-prefer-screen` | Warning | `container.querySelector` instead of `screen`/`getByRole` |
| `rtl-prefer-semantic` | Info | `getByTestId` over semantic queries |
| `rtl-prefer-user-event` | Info | `fireEvent` instead of `userEvent` |

## Mutation Resistance

| Rule | Severity | Description |
|------|----------|-------------|
| `mutation-resistant` | Info | Assertion may let mutants survive (e.g., `toBeGreaterThan(0)` vs `toBe(3)`) |
| `boundary-specificity` | Info | Boundary test doesn't assert exact value |
| `state-verification` | Info | Test only checks return value, not state changes |
| `assertion-intent-mismatch` | Warning | Test name suggests outcome but no assertion verifies it |

## AI Smells

| Rule | Severity | Description |
|------|----------|-------------|
| `ai-smell-tautological-assertion` | Warning | Tautological assertion (e.g. `expect(x).toBe(x)`) |
| `ai-smell-over-mocking` | Info | Too many mocks, testing implementation |
| `ai-smell-shallow-variety` | Info | Narrow input range |
| `ai-smell-happy-path-only` | Info | No error or edge tests |
| `ai-smell-parrot-assertion` | Info | Repeats spec wording without real check |
| `ai-smell-boilerplate-padding` | Info | Generic setup, low signal |

## Configuring Rules

Set severity per rule in `.rigorrc.json`:

```json
{
  "rules": {
    "weak-assertion": "error",
    "snapshot-overuse": "off",
    "debug-code": "info"
  }
}
```

Severities:
- `error` - 5 points penalty each (max 35 total)
- `warning` - 2 points penalty each (max 40 total)
- `info` - 1 point penalty each (max 15 total)
- `off` - Disabled
