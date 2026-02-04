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
        // Match " > " and " < " (space-surrounded) to avoid needing look-around; regex crate doesn't support it
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
}
