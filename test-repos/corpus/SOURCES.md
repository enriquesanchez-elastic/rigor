# Validation Corpus — Sources & Attribution

Real-world TypeScript test files vendored from permissively-licensed (MIT) open
source projects. Used by `tests/corpus.rs` to detect **score drift**: unintended
changes in Rigor's scores/issues on real code when rules or scoring are edited.

Each vendored file is unmodified except, for `immer`, a `.test.ts` suffix was
added to the original `__tests__/<name>.ts` filename (content unchanged) so the
file is recognized as a test. Each project's `LICENSE` is preserved alongside its
files.

| Project | Framework | Commit (pinned) | License |
|---------|-----------|-----------------|---------|
| [zod](https://github.com/colinhacks/zod) | Vitest | `912f0f51b0ced654d0069741e7160834dca742ee` | MIT (`zod/LICENSE`) |
| [ts-pattern](https://github.com/gvergnaud/ts-pattern) | Vitest | `c92ca435c7e1827e0fd55c539080ef1bfd6fe3f0` | MIT (`ts-pattern/LICENSE`) |
| [immer](https://github.com/immerjs/immer) | Vitest | `bf2d15439259887f98f2737cf7ebde4234d5adea` | MIT (`immer/LICENSE`) |

## Why these

Real assertion-heavy suites with varied patterns (type assertions, async
refinements, error-path coverage, parametric/`it.each` style) — the kind of code
that surfaces scoring false positives that hand-written fixtures miss.

## Updating the corpus

1. Re-clone at a new pinned commit, copy files, update the table above.
2. Regenerate the baseline:
   ```
   UPDATE_CORPUS_BASELINE=1 cargo test --test corpus
   ```
3. Review the `corpus-baseline.json` diff. Score changes here are **intentional
   only** — an unexplained diff means a rule/scoring change moved real-world
   scores, which is exactly what this gate exists to catch.
