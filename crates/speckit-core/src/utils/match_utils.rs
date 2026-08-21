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
        if let Some(&pattern_char) = pattern_chars.peek() {
            if text_char == pattern_char {
                pattern_chars.next();
            }
        }
    }

    pattern_chars.next().is_none()
}

/// Calculate a fuzzy match score (higher is better).
pub fn fuzzy_score(pattern: &str, text: &str) -> usize {
    let pattern_lower = pattern.to_lowercase();
    let text_lower = text.to_lowercase();

    if pattern_lower.is_empty() {
        return 0;
    }

    let mut score = 0;
    let mut pattern_idx = 0;
    let pattern_chars: Vec<char> = pattern_lower.chars().collect();
    let text_chars: Vec<char> = text_lower.chars().collect();

    for (text_idx, &text_char) in text_chars.iter().enumerate() {
        if pattern_idx < pattern_chars.len() && text_char == pattern_chars[pattern_idx] {
            score += 1;
            // Bonus for consecutive matches
            if pattern_idx > 0 && text_idx > 0 {
                let prev_text_char = text_chars[text_idx - 1];
                let prev_pattern_char = pattern_chars[pattern_idx - 1];
                if prev_text_char == prev_pattern_char {
                    score += 2;
                }
            }
            pattern_idx += 1;
        }
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
