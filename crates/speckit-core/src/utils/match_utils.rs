/// Fuzzy matching utilities.

/// Check if a string matches a pattern with fuzzy matching.
pub fn fuzzy_match(pattern: &str, text: &str) -> bool {
    let pattern_lower = pattern.to_lowercase();
    let text_lower = text.to_lowercase();

    if pattern_lower.is_empty() {
        return true;
    }

    let mut pattern_chars = pattern_lower.chars().peekable();

    for text_char in text_lower.chars() {
        if let Some(&pattern_char) = pattern_chars.peek()
            && text_char == pattern_char {
                pattern_chars.next();
            }
    }

    pattern_chars.next().is_none()
}

/// Calculate a fuzzy match score (higher is better).
///
/// The score combines per-character match counts with bonuses for
/// consecutive runs, prefix matches, and exact equality, plus a small
/// penalty for characters left over after the pattern has been fully
/// consumed.  Exact matches short-circuit with the maximum score so the
/// caller can rank candidates deterministically.
pub fn fuzzy_score(pattern: &str, text: &str) -> usize {
    let pattern_lower = pattern.to_lowercase();
    let text_lower = text.to_lowercase();

    if pattern_lower.is_empty() {
        return 0;
    }

    // Exact equality wins outright -- tie-breaker for identical candidates.
    if pattern_lower == text_lower {
        return usize::MAX / 2;
    }

    let pattern_chars: Vec<char> = pattern_lower.chars().collect();
    let text_chars: Vec<char> = text_lower.chars().collect();

    let mut score: usize = 0;
    let mut pattern_idx: usize = 0;
    let mut last_match: Option<usize> = None;

    for (text_idx, &text_char) in text_chars.iter().enumerate() {
        if pattern_idx < pattern_chars.len() && text_char == pattern_chars[pattern_idx] {
            score += 1;
            // Bonus for consecutive matches (pattern[i] == pattern[i-1] AND
            // text[j] == text[j-1]).
            if pattern_idx > 0 && text_idx > 0 {
                let prev_text_char = text_chars[text_idx - 1];
                let prev_pattern_char = pattern_chars[pattern_idx - 1];
                if prev_text_char == prev_pattern_char {
                    score += 2;
                }
            }
            pattern_idx += 1;
            last_match = Some(text_idx);
        }
    }

    // Pattern must be fully consumed for a meaningful score.
    if pattern_idx < pattern_chars.len() {
        return 0;
    }

    // Prefix bonus when the pattern matches starting at position 0.
    if let Some(0) = last_match {
        // The pattern always matches starting at 0 by construction once
        // pattern_idx reached `pattern_chars.len()` with `text_idx == 0`
        // still in `last_match`.
        score += pattern_chars.len() * 4;
    }

    // Penalize leftover characters after the last pattern character.
    if let Some(matched_at) = last_match {
        let leftover = text_chars.len().saturating_sub(matched_at + 1);
        score = score.saturating_sub(leftover * 3);
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match() {
        assert!(fuzzy_match("abc", "aabbcc"));
        assert!(fuzzy_match("abc", "abc"));
        assert!(!fuzzy_match("abc", "ab"));
        assert!(fuzzy_match("", "anything"));
    }

    #[test]
    fn test_fuzzy_score() {
        assert!(fuzzy_score("abc", "abc") > fuzzy_score("abc", "aabbcc"));
    }
}
