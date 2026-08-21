use regex::Regex;
/// Shared fenced-code-block detection for the Markdown parsers.
///
/// Several parsers need to ignore Markdown structure (headers, requirement
/// blocks, scenarios, delta sections) that appears inside fenced code blocks.
/// Keeping this logic in one place avoids the drift that previously left
/// `requirement-blocks.ts` treating fenced `### Requirement:` lines as real
/// requirements during validation and archiving.
use std::sync::LazyLock;

struct ActiveFence {
    marker: char, // '`' or '~'
    length: usize,
}

/// Regex that matches a potential opening fence: optional leading whitespace
/// followed by 3+ backticks or tildes (the info string, if any, is ignored).
static FENCE_OPEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(`{3,}|~{3,})").unwrap());

/// Regex that matches a closing fence: the entire line (apart from optional
/// leading/trailing whitespace) must consist solely of 3+ backticks or tildes.
static FENCE_CLOSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(`{3,}|~{3,})\s*$").unwrap());

/// Extract the fence marker character and its run-length from a line, or
/// `None` if the line is not a fence opening.
fn get_fence_marker(line: &str) -> Option<ActiveFence> {
    let caps = FENCE_OPEN.captures(line)?;
    let matched = &caps[1];
    let marker = matched.chars().next()?;
    let length = matched.len();
    Some(ActiveFence { marker, length })
}

/// Determine whether `line` closes the given `active_fence`.  A closing fence
/// must use the same marker character and have at least the same run-length;
/// the line must contain nothing but the fence markers (no info string).
fn is_closing_fence(line: &str, active_fence: &ActiveFence) -> bool {
    FENCE_CLOSE.captures(line).map_or(false, |caps| {
        let matched = &caps[1];
        matched
            .chars()
            .next()
            .map_or(false, |c| c == active_fence.marker)
            && matched.len() >= active_fence.length
    })
}

/// Build a per-line mask where `true` marks a line that is part of a fenced
/// code block (including the opening and closing fence lines themselves).
///
/// `lines` are the already-normalized (BOM-stripped, `\n`-split) document lines.
pub fn build_code_fence_mask(lines: &[String]) -> Vec<bool> {
    let mut mask = vec![false; lines.len()];
    let mut active_fence: Option<ActiveFence> = None;

    for (i, line) in lines.iter().enumerate() {
        match active_fence {
            None => {
                if let Some(fence) = get_fence_marker(line) {
                    mask[i] = true;
                    active_fence = Some(fence);
                }
            }
            Some(ref fence) => {
                mask[i] = true;
                if is_closing_fence(line, fence) {
                    active_fence = None;
                }
            }
        }
    }

    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_strings(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_input() {
        let mask = build_code_fence_mask(&[]);
        assert!(mask.is_empty());
    }

    #[test]
    fn no_fences() {
        let lines = to_strings(&["# Hello", "world", "## Sub"]);
        let mask = build_code_fence_mask(&lines);
        assert_eq!(mask, vec![false, false, false]);
    }

    #[test]
    fn backtick_fence() {
        let lines = to_strings(&["before", "```rust", "code", "```", "after"]);
        let mask = build_code_fence_mask(&lines);
        assert_eq!(mask, vec![false, true, true, true, false]);
    }

    #[test]
    fn tilde_fence() {
        let lines = to_strings(&["before", "~~~python", "code", "~~~", "after"]);
        let mask = build_code_fence_mask(&lines);
        assert_eq!(mask, vec![false, true, true, true, false]);
    }

    #[test]
    fn unclosed_fence() {
        let lines = to_strings(&["before", "```rust", "code", "more code"]);
        let mask = build_code_fence_mask(&lines);
        assert_eq!(mask, vec![false, true, true, true]);
    }

    #[test]
    fn longer_closing_fence() {
        let lines = to_strings(&["````", "code", "`````"]);
        let mask = build_code_fence_mask(&lines);
        assert_eq!(mask, vec![true, true, true]);
    }

    #[test]
    fn shorter_closing_fence_does_not_close() {
        let lines = to_strings(&["````", "code", "```", "more", "````"]);
        let mask = build_code_fence_mask(&lines);
        assert_eq!(mask, vec![true, true, true, true, true]);
    }

    #[test]
    fn closing_fence_with_info_string_does_not_close() {
        let lines = to_strings(&["```rust", "code", "``` text", "still fenced", "```"]);
        let mask = build_code_fence_mask(&lines);
        assert_eq!(mask, vec![true, true, true, true, true]);
    }

    #[test]
    fn nested_fences_not_supported() {
        let lines = to_strings(&["```", "```inner", "code", "```", "```"]);
        let mask = build_code_fence_mask(&lines);
        // The first ``` opens, the second ``` closes, rest is unmasked.
        assert_eq!(mask, vec![true, true, true, true, false]);
    }

    #[test]
    fn indented_fence() {
        let lines = to_strings(&["  ```rust", "code", "  ```"]);
        let mask = build_code_fence_mask(&lines);
        assert_eq!(mask, vec![true, true, true]);
    }
}
