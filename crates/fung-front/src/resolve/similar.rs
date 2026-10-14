//! "A similar name exists" suggestions for undefined names.

/// Returns the candidate closest to `name` within an edit budget that grows
/// with the name's length (short names would otherwise match almost
/// anything), preferring the alphabetically first on ties, or `None`.
pub(super) fn similar(name: &str, candidates: &[String]) -> Option<String> {
    let budget = edit_budget(name.chars().count());
    if budget == 0 {
        return None;
    }
    let mut sorted: Vec<&String> = candidates.iter().collect();
    sorted.sort();
    let mut best = None;
    let mut best_dist = budget + 1;
    for c in sorted {
        if c == name {
            continue;
        }
        let d = edit_distance(name, c);
        if d < best_dist {
            best = Some(c.clone());
            best_dist = d;
        }
    }
    best
}

/// How many edits may separate a typo from its intended name.
fn edit_budget(n: usize) -> usize {
    match n {
        0..=2 => 0,
        3..=4 => 1,
        _ => 2,
    }
}

/// The optimal string alignment distance in characters: like Levenshtein,
/// but a transposition of adjacent characters (fiels → files) counts as
/// one edit.
fn edit_distance(a: &str, b: &str) -> usize {
    let ra: Vec<char> = a.chars().collect();
    let rb: Vec<char> = b.chars().collect();
    let mut d = vec![vec![0usize; rb.len() + 1]; ra.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=ra.len() {
        for j in 1..=rb.len() {
            let cost = usize::from(ra[i - 1] != rb[j - 1]);
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && ra[i - 1] == rb[j - 2] && ra[i - 2] == rb[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[ra.len()][rb.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances() {
        assert_eq!(edit_distance("fiels", "files"), 1);
        assert_eq!(edit_distance("abcd", "abxy"), 2);
        assert_eq!(edit_distance("없는", "없다"), 1);
        assert_eq!(edit_distance("", "abc"), 3);
    }
}
