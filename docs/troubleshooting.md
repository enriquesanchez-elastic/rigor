# Troubleshooting

## "No source file found" for --mutate

Mutation testing requires a mapped source file.

**Solutions:**

1. Ensure source file exists with matching name:
   - `auth.test.ts` → `auth.ts`
   - `Button.test.tsx` → `Button.tsx`

2. Configure explicit mappings:
   ```json
   {
     "sourceMapping": {
       "mode": "manual",
       "mappings": { "tests/**/*.test.ts": "src/**/*.ts" }
     }
   }
   ```

3. E2E tests (Cypress/Playwright) typically don't have single source files — this warning is expected.

## Framework Detected Incorrectly

Force the framework in config:

```json
{ "framework": "vitest" }
```

Or ensure imports are explicit:
```typescript
import { vi, describe, it, expect } from 'vitest';
```

## Cache Issues

If you suspect stale results:

```bash
# Clear cache before running
rigor src/ --clear-cache

# Or disable cache entirely
rigor src/ --no-cache
```

Cache location: `.rigor-cache.json` in project root.

## Score Seems Wrong

1. **Use `--verbose`** to see all issues:
   ```bash
   rigor src/auth.test.ts --verbose
   ```

2. **Check test type** — E2E tests use different weights:
   - E2E de-emphasizes boundary conditions
   - Unit tests weight all categories equally

3. **Verify source file was found** — affects error/boundary coverage:
   ```bash
   rigor src/auth.test.ts --verbose | grep "Source:"
   ```

4. **Review issue penalties** — many warnings add up quickly.

## AI --apply Not Working

Ensure one of:

1. **Built-in Claude integration:**
   ```bash
   # Build with AI feature
   cargo build --release --features ai

   # Set API key
   export ANTHROPIC_API_KEY="sk-ant-..."

   # Run
   rigor src/auth.test.ts --fix --apply
   ```

2. **Custom command:**
   ```bash
   export RIGOR_APPLY_CMD="node my-ai-script.js"
   rigor src/auth.test.ts --fix --apply
   ```

## Watch Mode Not Working

Watch mode requires filesystem notifications. Issues:

1. **Too many files** — Some systems have limits on watched files
2. **Network drives** — May not support notifications
3. **Docker/containers** — May need polling mode

Try running from project root:
```bash
cd /path/to/project
rigor src/ --watch
```

## Slow Analysis

1. **Enable parallel processing:**
   ```bash
   rigor src/ --parallel
   ```

2. **Limit threads if needed:**
   ```bash
   rigor src/ --parallel --jobs 4
   ```

3. **Use caching** (enabled by default):
   - First run analyzes all files
   - Subsequent runs skip unchanged files

4. **Ignore unnecessary files:**
   ```json
   {
     "ignore": ["**/generated/**", "**/vendor/**"]
   }
   ```

## Parse Errors

Rigor uses tree-sitter which is lenient, but some edge cases fail:

1. **Check TypeScript syntax** — Ensure file compiles
2. **Check encoding** — Must be UTF-8
3. **Check for BOM** — Remove byte-order mark if present

## False Positives

If a rule incorrectly flags code:

1. **Suppress inline:**
   ```typescript
   // rigor-ignore-next-line
   expect(result).toBeDefined();
   ```

2. **Disable rule for file pattern:**
   ```json
   {
     "overrides": [{
       "files": ["**/special/**"],
       "rules": { "weak-assertion": "off" }
     }]
   }
   ```

3. **Disable rule globally:**
   ```json
   {
     "rules": { "problematic-rule": "off" }
   }
   ```

## MCP Server Issues

```bash
rigor mcp
```

The server communicates via stdin/stdout using JSON-RPC 2.0.

**Common issues:**

1. **Client not connecting** — Ensure client is configured to run `rigor mcp`
2. **Timeout** — Large files may take longer to analyze
3. **Path issues** — Use absolute paths when possible

## Getting Help

1. Run with `--verbose` for detailed output
2. Check the [GitHub Issues](https://github.com/rigor-dev/rigor/issues)
3. Include Rigor version, OS, and reproduction steps when reporting
