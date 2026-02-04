//! Select a subset of mutations for fast mutation testing.

use super::Mutation;

/// Select up to `count` mutations, preferring boundary and boolean operators.
pub fn select_mutations(mutations: &[Mutation], count: usize) -> Vec<Mutation> {
    if mutations.is_empty() {
        return vec![];
    }
    if mutations.len() <= count {
        return mutations.to_vec();
    }

    // Prefer >=, <=, >, < (boundary), then true/false, then arithmetic
    let priority = |m: &Mutation| -> u8 {
        if m.description.starts_with(">=") || m.description.starts_with("<=")
            || m.description.starts_with("> to") || m.description.starts_with("< to")
        {
            3
        } else if m.description.contains("true") || m.description.contains("false") {
            2
        } else if m.description.starts_with("=== ") || m.description.starts_with("!= ") {
            2
        } else {
            1
        }
    };

    let mut indexed: Vec<(usize, u8)> = mutations
        .iter()
        .enumerate()
        .map(|(i, m)| (i, priority(m)))
        .collect();
    indexed.sort_by(|a, b| b.1.cmp(&a.1));

    let take: Vec<usize> = indexed.into_iter().take(count).map(|(i, _)| i).collect();
    take.into_iter()
        .map(|i| mutations[i].clone())
        .collect()
}
