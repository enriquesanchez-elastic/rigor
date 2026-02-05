# Mutation Testing

Rigor includes lightweight mutation testing to verify your tests catch real bugs.

## Quick Start

```bash
rigor src/auth.test.ts --mutate
```

This:
1. Finds the source file (`auth.ts`)
2. Generates mutants (code changes)
3. Runs your tests for each mutant
4. Reports how many were killed vs survived

## Modes

| Mode | Mutants | Usage |
|------|---------|-------|
| `--mutate` or `--mutate quick` | 10 | Fast feedback |
| `--mutate medium` | 30 | Balanced |
| `--mutate full` | All | Thorough |

## Output

```
🧬 Mutation Testing: src/auth.ts
   Total: 15 | Killed: 12 | Survived: 3
   Kill Rate: 80%

   Survived Mutants:
   L23: >= to > (boundary)
   L45: return token to return null (return value)
   L67: true to false (boolean)

   Suggestions:
   - Add boundary test for line 23 comparison
   - Assert exact return value at line 45
```

## Mutation Operators

Rigor applies 28 mutation operators:

| Category | Mutations |
|----------|-----------|
| **Boundary** | `>=` → `>`, `<=` → `<`, `>` → `>=`, `<` → `<=` |
| **Boolean** | `true` → `false`, `false` → `true` |
| **Arithmetic** | `+` → `-`, `-` → `+`, `*` → `/` |
| **Equality** | `===` → `!=`, `!==` → `==` |
| **String** | `"string"` → `""`, `""` → `" "` |
| **Array** | `[a, b]` → `[]`, `[]` → `[0]`, `[0]` → `[1]` |
| **Return** | `return x` → `return null`, `return x` → `return undefined` |
| **Increment** | `++` → `--`, `--` → `++`, `+= 1` → `-= 1` |
| **TypeScript** | `?.` → `.`, `??` → `\|\|`, `!.` → `.` |

## Requirements

- Test file must map to a source file
- Source file must exist (auto-detected or configured)
- Test command must be available

## Custom Test Command

By default, Rigor runs `npm test`. Override with:

```bash
RIGOR_TEST_CMD="yarn test" rigor src/auth.test.ts --mutate
RIGOR_TEST_CMD="pnpm test" rigor src/auth.test.ts --mutate
RIGOR_TEST_CMD="npx vitest run" rigor src/auth.test.ts --mutate
```

## Source File Mapping

Rigor auto-detects source files:
- `auth.test.ts` → `auth.ts`
- `__tests__/Button.test.tsx` → `Button.tsx`
- `tests/api/user.test.ts` → `src/api/user.ts`

Or configure explicitly:

```json
{
  "sourceMapping": {
    "mode": "manual",
    "mappings": {
      "tests/**/*.test.ts": "src/**/*.ts"
    }
  }
}
```

## Batch Mutation Testing

Run on multiple files:

```bash
rigor src/ --mutate --parallel
```

## Interpreting Results

| Kill Rate | Meaning |
|-----------|---------|
| 90-100% | Excellent - Tests catch most changes |
| 70-89% | Good - Some gaps to address |
| 50-69% | Fair - Tests miss significant changes |
| <50% | Poor - Tests don't verify behavior |

**Survived mutants** indicate test gaps:
- Boundary mutant survived → Add boundary value tests
- Return value mutant survived → Assert exact return values
- Boolean mutant survived → Test both true/false paths

## Limitations

- Requires source file mapping
- E2E tests typically don't have single source files
- Slower than static analysis (runs actual tests)
- Some mutants may be equivalent (same behavior)

## When to Use

| Scenario | Tool |
|----------|------|
| Fast feedback, CI gates | Static analysis (`rigor src/`) |
| Deep validation | Mutation testing (`rigor src/ --mutate`) |
| Comprehensive | Both in sequence |

Use static analysis for quick feedback and mutation testing for periodic deep validation.
