//! F20 — narrowing the tree.
//!
//! Fuzzy: the typed characters must appear in order, not adjacently. Ranking
//! prefers matches whose characters sit close together and near the start of
//! the name, which is what "closest match" means in practice.

use crate::State;

/// How well `needle` matches `haystack`, or `None` if it does not. Higher is
/// closer.
pub fn score(needle: &str, haystack: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let hay: Vec<char> = haystack.to_lowercase().chars().collect();
    let mut points = 0;
    let mut at = 0;
    let mut previous: Option<usize> = None;
    for wanted in needle.to_lowercase().chars() {
        let found = hay[at..].iter().position(|c| *c == wanted)? + at;
        points += match previous {
            // Adjacent characters are worth more than scattered ones.
            Some(last) if found == last + 1 => 8,
            Some(last) => -((found - last) as i32).min(4),
            None => 0,
        };
        previous = Some(found);
        at = found + 1;
    }
    Some(points - (haystack.len() as i32 / 8))
}

/// The filename decides. Scoring the whole path lets an early letter in a
/// directory hijack the match — "tree" latching onto the t in "features".
pub fn rank(needle: &str, path: &str) -> Option<i32> {
    let name = path.rsplit('/').next().unwrap_or(path);
    match score(needle, name) {
        Some(points) => {
            let start = name.to_lowercase().starts_with(&needle.to_lowercase());
            Some(points + 12 + if start { 12 } else { 0 })
        }
        // Still findable by its directory, just never above a name match.
        None => score(needle, path),
    }
}

/// What the tree is showing: everything, some matches, or nothing at all.
pub fn view_state(state: &State) -> &'static str {
    if state.filter.is_empty() {
        "unfiltered"
    } else if matches(state).is_empty() {
        "no-matches"
    } else {
        "matches"
    }
}

/// Matching files, closest first. Only files match; `state.indexed` is the
/// project's file list, walked afresh whenever a filter is opened.
///
/// A literal match and a scattered one are different kinds of answer, not
/// degrees of one, so anything containing what was typed hides everything that
/// merely spells its letters in order. Typing "todo.md" used to find the file
/// and twenty-two `.scratch/` paths spelling t-o-d-o-.-m-d across their whole
/// length, and no ranking tweak fixes that — the tail is a different question
/// being answered. Fuzzy stands alone only when nothing is literal, which is
/// what keeps "ftr" reaching "file_tree.rs".
pub fn matches(state: &State) -> Vec<String> {
    let needle = &state.filter;
    if needle.is_empty() {
        return Vec::new();
    }
    let typed = needle.to_lowercase();
    let mut ranked: Vec<(bool, i32, &String)> = state
        .indexed
        .iter()
        .filter_map(|path| {
            let literal = path.to_lowercase().contains(&typed);
            rank(needle, path).map(|points| (literal, points, path))
        })
        .collect();
    if ranked.iter().any(|(literal, ..)| *literal) {
        ranked.retain(|(literal, ..)| *literal);
    }
    // Ties keep the index's order, which is alphabetical, so results are stable.
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.2.cmp(b.2)));
    ranked.into_iter().map(|(.., path)| path.clone()).collect()
}

pub fn best(state: &State) -> Option<String> {
    matches(state).into_iter().next()
}

#[cfg(test)]
mod tests {
    use crate::State;

    fn narrowed_by(needle: &str, files: &[&str]) -> Vec<String> {
        let state = State {
            filter: needle.to_string(),
            indexed: files.iter().map(|path| path.to_string()).collect(),
            ..State::default()
        };
        super::matches(&state)
    }

    /// Case must not decide the *class* of a match. Typed in the wrong case, a
    /// literal match still has to count as one — otherwise it is filed with the
    /// scattered matches and hidden by whichever file happened to be typed in
    /// the case the reader guessed.
    #[test]
    fn a_literal_match_is_literal_in_any_case() {
        assert_eq!(
            narrowed_by("TODO.md", &["docs/todo.md", "notes/the-old-story.md"]),
            vec!["docs/todo.md".to_string()]
        );
    }
}
