# Scoring

Rigor calculates a score from 0-100 based on **six categories** minus issue penalties.

## Grade Scale

| Grade | Score | Description |
|-------|-------|-------------|
| A | 90-100 | Excellent - Well-structured tests with strong assertions |
| B | 80-89 | Good - Solid tests with room for improvement |
| C | 70-79 | Fair - Basic coverage but needs strengthening |
| D | 60-69 | Poor - Significant quality issues |
| F | 0-59 | Failing - Major improvements needed |

## Categories

Each category contributes 0-25 points (raw); the sum is normalized to 0-100, then weights and penalties are applied.

| Category | What It Measures |
|----------|------------------|
| **Assertion Quality** | Strong vs weak assertions, snapshot usage, trivial/tautological assertions |
| **Error Coverage** | Functions that throw have error tests |
| **Boundary Conditions** | Numeric comparisons tested at boundaries |
| **Test Isolation** | No shared mutable state, proper setup/teardown |
| **Input Variety** | Diverse test inputs, edge cases covered |
| **AI Smells** | Tautological assertions, over-mocking, shallow variety, happy-path-only, parrot names, boilerplate |

## How It's Calculated

```
Final Score = Weighted Category Score - Issue Penalties
```

### 1. Weighted Category Score

Categories are weighted differently based on test type (weights sum to 100):

| Category | Unit | E2E | Component | Integration |
|----------|------|-----|-----------|-------------|
| Assertion Quality | 20% | 30% | 25% | 22% |
| Error Coverage | 15% | 15% | 15% | 18% |
| Boundary Conditions | 20% | 5% | 15% | 15% |
| Test Isolation | 15% | 25% | 20% | 20% |
| Input Variety | 15% | 20% | 20% | 20% |
| AI Smells | 15% | 5% | 5% | 5% |

E2E tests de-emphasize boundary conditions and AI smells; unit tests weight all six. The weighted sum is capped at 100 before penalties.

### 2. Issue Penalties

Issues reduce your score:

| Severity | Penalty | Max Total |
|----------|---------|-----------|
| Error | 5 points each | 35 |
| Warning | 2 points each | 40 |
| Info | 1 point each | 15 |

Maximum total penalty is 90 points.

## Example

A unit test file with:
- Assertion Quality: 20/25, Error Coverage: 18/25, Boundary Conditions: 15/25, Test Isolation: 22/25, Input Variety: 17/25, AI Smells: 25/25
- Issues (penalty-only): 0 errors, 3 warnings, 2 info

**Step 1: Weighted score** (unit weights: 20, 15, 20, 15, 15, 15)
```
(20×20 + 18×15 + 15×20 + 22×15 + 17×15 + 25×15) / 25 = 78.2 → 78
```

**Step 2: Issue penalty**
```
0×5 + 3×2 + 2×1 = 8
```

**Step 3: Final score**
```
78 - 8 = 70 (Grade: C)
```

## Test Type Detection

Rigor automatically classifies tests:

| Type | How Detected |
|------|--------------|
| **E2E** | Path contains `e2e`, `.cy.`, `/cypress/`; or Playwright/Cypress detected |
| **Integration** | Path contains `integration` |
| **Component** | Path contains `component`; or `@testing-library`, `render()` found |
| **Unit** | Default |

## Assertion Quality Details

| Strength | Examples |
|----------|----------|
| **Strong** | `toBe()`, `toEqual()`, `toStrictEqual()`, `toThrow()`, `toHaveBeenCalledTimes()` |
| **Moderate** | `toContain()`, `toMatch()`, `toHaveLength()`, `toBeInstanceOf()` |
| **Weak** | `toBeDefined()`, `toBeTruthy()`, `toBeFalsy()`, `toMatchSnapshot()` |

Strong assertions verify specific values. Weak assertions only check existence or truthiness.

## Improving Your Score

1. **Replace weak assertions** with specific value checks
2. **Add error tests** for functions that throw
3. **Test boundary values** for numeric comparisons
4. **Reset state** in `beforeEach` hooks
5. **Add edge cases** (0, null, empty strings)
6. **Fix flagged issues** shown in output

---

## Score vs. finds (issues)

**How score and “finds” stay aligned**

- **Finds** = the list of issues reported (e.g. weak assertion, missing error test, debug code).
- **Score** is driven by those same finds in two ways:
  1. **Category scores** – Many rules feed into one of the six categories. When a rule fires, that category’s raw score (0–25) is reduced (or not increased) by that rule’s `calculate_score` logic. So more issues in a category → lower category score.
  2. **Penalties** – Issues that don’t affect a category (e.g. debug code, focused test, vague name) only add penalty points. So more of those → lower final score.

So in principle: **more or worse finds → lower score**. The exact number depends on which rules fire (category vs penalty) and severity (error/warning/info).

**Why some “bad” files still get A**

With six categories, each category has a smaller weight. The same number of issues therefore reduces the score less than with fewer categories. The regression and integration tests encode the current design: “good” fixtures score high, “bad” fixtures still report many issues and don’t get near-perfect (e.g. &lt; 95), and **worse files score lower than better files** (see below).

---

## How we validate alignment (great analysis)

We check that **score and finds are aligned** and that the analyzer behaves as intended in three ways:

| Test | What it checks | Where |
|------|----------------|--------|
| **Regression** | For each fixture in `test-repos/fake-project`, (score, issue count) equals a locked baseline. Prevents accidental drift when changing rules or scoring. | `tests/regression.rs` |
| **Integration – rules** | For each “bad” fixture, the **expected rule** fires (e.g. `TrivialAssertion` on trivial-assertions.test.ts, `NoAssertions` on no-assertions.test.ts). Ensures finds are correct. | `tests/integration.rs` |
| **Integration – score bands** | **Good** file (e.g. auth.test.ts) scores ≥ 80 (B or above). **Bad** files (weak-assertions, mixed-bad) have at least one issue and score &lt; 95. Ensures bad files don’t get near-perfect scores. | `tests/integration.rs` |
| **Integration – ordering** | Intentionally bad file (weak-assertions) scores **lower** than the good file (auth). Ensures “more/worse issues → lower score” in a concrete comparison. | `tests/integration.rs` |

**When to update baselines**

- After **intentional** scoring or rule changes (e.g. new category, new rule, weight change), run the test suite and update:
  - **Regression**: `tests/regression.rs` – set new (score, issue count) per file.
  - **Integration**: adjust score thresholds or assertions only if the product behavior (e.g. “bad files don’t get A”) is intentionally changed.

So we’re **aligned** when: regression and integration tests pass, good files score above the band, bad files score below the band and below the good file, and the right rules fire on the right fixtures. That is how we test that we’re “doing a great analysis.”
