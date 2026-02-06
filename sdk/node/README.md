# @rigor/sdk

Node.js wrapper for [Rigor](https://github.com/your-org/rigor) test quality analysis. Uses the stable JSON API (see [docs/api.md](../../docs/api.md)).

## Requirements

- Node.js 16+
- `rigor` binary on PATH, or set `RIGOR_BIN` to the binary path

## Install

From the repo root after building rigor:

```bash
cd sdk/node && npm pack
npm install /path/to/rigor-sdk-1.0.0.tgz
```

Or link for development: `npm link` in `sdk/node`, then `npm link @rigor/sdk` in your app.

## Usage

```js
const { analyze, analyzeSource } = require('@rigor/sdk');

// Analyze a file
const result = await analyze('tests/auth.test.ts');
console.log(result.score.value, result.issues.length);

// Analyze in-memory source (e.g. from AI-generated test)
const result2 = await analyzeSource(`describe('foo', () => { it('works', () => { expect(1).toBe(1); }); });`);
console.log(result2.score.grade);

// With options
const result3 = await analyze('tests/app.test.ts', { threshold: 80, config: '.rigorrc.json' });
```

## API

- **`analyze(input, options?)`**  
  - `input`: file path (string) or `{ stdin: string, filename?: string }`  
  - `options`: `{ config?: string, threshold?: number }`  
  - Returns: `Promise<AnalysisResult | AnalysisResult[]>`

- **`analyzeSource(source, options?)`**  
  - Same as `analyze({ stdin: source, filename: options?.filename ?? 'stdin.test.ts' }, options)`.
