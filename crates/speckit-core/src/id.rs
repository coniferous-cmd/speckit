use regex::Regex;
use std::sync::LazyLock;

/// The one kebab id grammar. Store ids, change ids, and legacy initiative ids
/// all share it.
pub static KEBAB_ID_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9]+(?:-[a-z0-9]+)*$").unwrap());

/// Human rendering of the grammar, shared so the wording never forks.
pub const KEBAB_ID_DESCRIPTION: &str =
    "must be kebab-case with lowercase letters, numbers, and single hyphen separators";

/// The fix-line twin of KEBAB_ID_DESCRIPTION, shared for the same reason.
pub const KEBAB_ID_FIX: &str =
    "Use kebab-case with lowercase letters, numbers, and single hyphen separators.";

/// Returns `true` when `value` satisfies the kebab-case id grammar.
pub fn is_kebab_id(value: &str) -> bool {
    KEBAB_ID_REGEX.is_match(value)
}

/// The folder-safe-name grammar (store ids layer the kebab grammar on top of
/// it; workset member labels use it alone). Returns a problem description, or
/// `None` when valid.
pub fn folder_style_name_problem(value: &str, label: &str) -> Option<String> {
    if value.is_empty() {
        return Some(format!("{label} must not be empty"));
    }

    if value == "." || value == ".." {
        return Some(format!("{label} must not be '{value}'"));
    }

    if value.contains('/') || value.contains('\\') {
        return Some(format!("{label} must not contain path separators"));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_kebab_ids() {
        assert!(is_kebab_id("foo"));
        assert!(is_kebab_id("foo-bar"));
        assert!(is_kebab_id("a-1-b-2"));
        assert!(is_kebab_id("abc123"));
    }

    #[test]
    fn invalid_kebab_ids() {
        assert!(!is_kebab_id(""));
        assert!(!is_kebab_id("Foo"));
        assert!(!is_kebab_id("foo_bar"));
        assert!(!is_kebab_id("-foo"));
        assert!(!is_kebab_id("foo-"));
        assert!(!is_kebab_id("foo--bar"));
    }

    #[test]
    fn folder_name_valid() {
        assert_eq!(folder_style_name_problem("hello", "name"), None);
    }

    #[test]
    fn folder_name_empty() {
        assert!(folder_style_name_problem("", "name").is_some());
    }

    #[test]
    fn folder_name_dot() {
        assert!(folder_style_name_problem(".", "name").is_some());
        assert!(folder_style_name_problem("..", "name").is_some());
    }

    #[test]
    fn folder_name_separators() {
        assert!(folder_style_name_problem("foo/bar", "name").is_some());
        assert!(folder_style_name_problem("foo\\bar", "name").is_some());
    }
}
