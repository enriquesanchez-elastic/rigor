# Scoring

Rigor calculates a score from 0-100 based on five categories minus issue penalties.

## Grade Scale

| Grade | Score | Description |
|-------|-------|-------------|
| A | 90-100 | Excellent - Well-structured tests with strong assertions |
| B | 80-89 | Good - Solid tests with room for improvement |
| C | 70-79 | Fair - Basic coverage but needs strengthening |
| D | 60-69 | Poor - Significant quality issues |
| F | 0-59 | Failing - Major improvements needed |

## Categories

Each category contributes 0-25 points:

| Category | What It Measures |
|----------|------------------|
| **Assertion Quality** | Strong vs weak assertions, snapshot usage |
| **Error Coverage** | Functions that throw have error tests |
| **Boundary Conditions** | Numeric comparisons tested at boundaries |
| **Test Isolation** | No shared mutable state, proper setup/teardown |
| **Input Variety** | Diverse test inputs, edge cases covered |

## How It's Calculated

```
Final Score = Weighted Category Score - Issue Penalties
```

### 1. Weighted Category Score

Categories are weighted differently based on test type:

| Category | Unit | E2E | Component | Integration |
|----------|------|-----|-----------|-------------|
| Assertion Quality | 25% | 35% | 30% | 25% |
| Error Coverage | 20% | 15% | 15% | 20% |
| Boundary Conditions | 25% | 5% | 15% | 15% |
| Test Isolation | 15% | 25% | 20% | 20% |
| Input Variety | 15% | 20% | 20% | 20% |

E2E tests de-emphasize boundary conditions (less relevant) and emphasize isolation. Unit tests balance boundaries and assertions equally.

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
- Assertion Quality: 20/25
- Error Coverage: 18/25
- Boundary Conditions: 15/25
- Test Isolation: 22/25
- Input Variety: 17/25
- Issues: 0 errors, 3 warnings, 2 info

**Step 1: Weighted score** (unit test weights)
```
(20×25 + 18×20 + 15×25 + 22×15 + 17×15) / 25 = 72.8
```

**Step 2: Issue penalty**
```
0×5 + 3×2 + 2×1 = 8
```

**Step 3: Final score**
```
72.8 - 8 = 64.8 → 65 (Grade: D)
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
