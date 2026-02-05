//! Mutation operators - generate mutants from source code.

use regex::Regex;

/// A single mutation: replace one span of text with another
#[derive(Debug, Clone)]
pub struct Mutation {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
    pub original: String,
    pub replacement: String,
    pub description: String,
}

/// Operator that produces replacement for a matched pattern
#[derive(Debug, Clone)]
pub struct MutationOperator {
    pub pattern: Regex,
    pub replacements: Vec<&'static str>,
    pub description: &'static str,
}

fn make_ops() -> Vec<MutationOperator> {
    vec![
        // --- Boundary / comparison (existing) ---
        MutationOperator {
            pattern: Regex::new(r">=").unwrap(),
            replacements: vec![">"],
            description: ">= to >",
        },
        MutationOperator {
            pattern: Regex::new(r"<=").unwrap(),
            replacements: vec!["<"],
            description: "<= to <",
        },
        MutationOperator {
            pattern: Regex::new(r" > ").unwrap(),
            replacements: vec![" >="],
            description: "> to >=",
        },
        MutationOperator {
            pattern: Regex::new(r" < ").unwrap(),
            replacements: vec![" <="],
            description: "< to <=",
        },
        // --- Boolean (existing) ---
        MutationOperator {
            pattern: Regex::new(r"\btrue\b").unwrap(),
            replacements: vec!["false"],
            description: "true to false",
        },
        MutationOperator {
            pattern: Regex::new(r"\bfalse\b").unwrap(),
            replacements: vec!["true"],
            description: "false to true",
        },
        // --- Arithmetic (existing) ---
        MutationOperator {
            pattern: Regex::new(r" \+ ").unwrap(),
            replacements: vec![" - "],
            description: "+ to -",
        },
        MutationOperator {
            pattern: Regex::new(r" - ").unwrap(),
            replacements: vec![" + "],
            description: "- to +",
        },
        MutationOperator {
            pattern: Regex::new(r" \* ").unwrap(),
            replacements: vec![" / "],
            description: "* to /",
        },
        // --- Equality (existing) ---
        MutationOperator {
            pattern: Regex::new(r" === ").unwrap(),
            replacements: vec![" != "],
            description: "=== to !=",
        },
        MutationOperator {
            pattern: Regex::new(r" !== ").unwrap(),
            replacements: vec![" == "],
            description: "!= to ==",
        },
        // --- 2. String mutations ---
        // Non-empty double-quoted string -> empty string
        MutationOperator {
            pattern: Regex::new(r#""[^"]+""#).unwrap(),
            replacements: vec![r#""""#],
            description: "string to empty string",
        },
        // Non-empty single-quoted string -> empty string
        MutationOperator {
            pattern: Regex::new(r"'[^']+'").unwrap(),
            replacements: vec!["''"],
            description: "string to empty string (single quote)",
        },
        // Empty double-quoted string -> one space (so mutant is still valid)
        MutationOperator {
            pattern: Regex::new(r#""""#).unwrap(),
            replacements: vec![r#"" ""#],
            description: "empty string to space",
        },
        // Empty single-quoted string -> one space
        MutationOperator {
            pattern: Regex::new(r"''").unwrap(),
            replacements: vec!["' '"],
            description: "empty string to space (single quote)",
        },
        // --- 4. Array mutations ---
        // Array literal with comma (2+ elements) -> empty array
        MutationOperator {
            pattern: Regex::new(r"\[[^\]]*,[^\]]*\]").unwrap(),
            replacements: vec!["[]"],
            description: "array literal to empty array",
        },
        // Empty array -> [0]
        MutationOperator {
            pattern: Regex::new(r"\[\s*\]").unwrap(),
            replacements: vec!["[0]"],
            description: "empty array to [0]",
        },
        // Index [0] -> [1] (off-by-one)
        MutationOperator {
            pattern: Regex::new(r"\[0\]").unwrap(),
            replacements: vec!["[1]"],
            description: "index 0 to 1",
        },
        // --- 6. Return value mutations ---
        MutationOperator {
            pattern: Regex::new(r"return\s+[^;]+;").unwrap(),
            replacements: vec!["return null;", "return undefined;"],
            description: "return to null/undefined",
        },
        // --- 7. Increment / decrement ---
        MutationOperator {
            pattern: Regex::new(r"\+\+").unwrap(),
            replacements: vec!["--"],
            description: "++ to --",
        },
        MutationOperator {
            pattern: Regex::new(r"--").unwrap(),
            replacements: vec!["++"],
            description: "-- to ++",
        },
        MutationOperator {
            pattern: Regex::new(r"\+= ?1\b").unwrap(),
            replacements: vec!["-= 1"],
            description: "+= 1 to -= 1",
        },
        MutationOperator {
            pattern: Regex::new(r"-= ?1\b").unwrap(),
            replacements: vec!["+= 1"],
            description: "-= 1 to += 1",
        },
        // --- TypeScript-specific operators ---
        // Optional chaining: ?. -> . (may cause runtime errors if value is null)
        MutationOperator {
            pattern: Regex::new(r"\?\.").unwrap(),
            replacements: vec!["."],
            description: "?. to . (optional chaining removed)",
        },
        // Nullish coalescing: ?? -> || (different falsy behavior)
        MutationOperator {
            pattern: Regex::new(r" \?\? ").unwrap(),
            replacements: vec![" || "],
            description: "?? to || (nullish to logical or)",
        },
        // Non-null assertion at end of expressions: )! -> )
        MutationOperator {
            pattern: Regex::new(r"\)!\.").unwrap(),
            replacements: vec![")."],
            description: ")! to ) (non-null assertion removed)",
        },
        // Non-null assertion: identifier! followed by . or ) or ; or ,
        MutationOperator {
            pattern: Regex::new(r"!\.").unwrap(),
            replacements: vec!["."],
            description: "!. to . (non-null assertion removed)",
        },
    ]
}

/// Generate all possible mutations for the given source content.
pub fn generate_mutations(source: &str) -> Vec<Mutation> {
    let ops = make_ops();
    let mut out = Vec::new();

    for op in ops {
        for cap in op.pattern.find_iter(source) {
            let start = cap.start();
            let end = cap.end();
            let original = source[start..end].to_string();
            let (line, column) = line_column(source, start);

            for &replacement in &op.replacements {
                if replacement == original {
                    continue;
                }
                out.push(Mutation {
                    start,
                    end,
                    line,
                    column,
                    original: original.clone(),
                    replacement: replacement.to_string(),
                    description: op.description.to_string(),
                });
            }
        }
    }

    out
}

fn line_column(source: &str, byte_offset: usize) -> (usize, usize) {
    let before = &source[..byte_offset];
    let line = before.lines().count().max(1);
    let last_newline = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let column = before[last_newline..].len() + 1;
    (line, column)
}

/// Apply a single mutation to source content.
pub fn apply_mutation(source: &str, mutation: &Mutation) -> String {
    if source.get(mutation.start..mutation.end) != Some(mutation.original.as_str()) {
        return source.to_string();
    }
    format!(
        "{}{}{}",
        &source[..mutation.start],
        mutation.replacement,
        &source[mutation.end..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_mutation() {
        let s = "if (x >= 0) return true;";
        let mutations = generate_mutations(s);
        assert!(!mutations.is_empty());
    }

    #[test]
    fn test_string_mutations() {
        let s = r#"const x = "hello"; const y = '';"#;
        let mutations = generate_mutations(s);
        let string_ops: Vec<_> = mutations
            .iter()
            .filter(|m| m.description.contains("string") || m.description.contains("empty"))
            .collect();
        assert!(!string_ops.is_empty(), "should have string mutations");
        let applied = apply_mutation(s, &mutations[0]);
        assert_ne!(applied, s);
    }

    #[test]
    fn test_array_mutations() {
        let s = "const a = [1, 2]; const b = []; const c = arr[0];";
        let mutations = generate_mutations(s);
        let array_ops: Vec<_> = mutations
            .iter()
            .filter(|m| m.description.contains("array") || m.description.contains("index"))
            .collect();
        assert!(!array_ops.is_empty());
    }

    #[test]
    fn test_return_mutations() {
        let s = "function f() { return 42; }";
        let mutations = generate_mutations(s);
        let return_ops: Vec<_> = mutations
            .iter()
            .filter(|m| m.description.contains("return"))
            .collect();
        assert!(!return_ops.is_empty());
        let null_mut = mutations.iter().find(|m| m.replacement == "return null;").unwrap();
        let applied = apply_mutation(s, null_mut);
        assert!(applied.contains("return null;"));
    }

    #[test]
    fn test_increment_decrement_mutations() {
        let s = "let i = 0; i++; i--; i += 1; i -= 1;";
        let mutations = generate_mutations(s);
        let inc_ops: Vec<_> = mutations
            .iter()
            .filter(|m| m.description.contains("++") || m.description.contains("--") || m.description.contains("+=") || m.description.contains("-="))
            .collect();
        assert!(!inc_ops.is_empty());
    }

    #[test]
    fn test_typescript_optional_chaining_mutations() {
        let s = "const name = user?.profile?.name;";
        let mutations = generate_mutations(s);
        let ts_ops: Vec<_> = mutations
            .iter()
            .filter(|m| m.description.contains("optional chaining"))
            .collect();
        assert!(!ts_ops.is_empty(), "Should have optional chaining mutations");
    }

    #[test]
    fn test_typescript_nullish_coalescing_mutations() {
        let s = "const value = input ?? 'default';";
        let mutations = generate_mutations(s);
        let ts_ops: Vec<_> = mutations
            .iter()
            .filter(|m| m.description.contains("nullish"))
            .collect();
        assert!(!ts_ops.is_empty(), "Should have nullish coalescing mutations");
    }

    #[test]
    fn test_typescript_non_null_assertion_mutations() {
        let s = "const element = document.getElementById('app')!.innerText;";
        let mutations = generate_mutations(s);
        let ts_ops: Vec<_> = mutations
            .iter()
            .filter(|m| m.description.contains("non-null"))
            .collect();
        assert!(!ts_ops.is_empty(), "Should have non-null assertion mutations");
    }
}
