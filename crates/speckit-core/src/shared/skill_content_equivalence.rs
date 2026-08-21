/// Check if two skill contents are equivalent (ignoring whitespace differences).
pub fn are_skills_equivalent(content1: &str, content2: &str) -> bool {
    normalize_skill_content(content1) == normalize_skill_content(content2)
}

/// Normalize skill content for comparison.
fn normalize_skill_content(content: &str) -> String {
    content
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_are_skills_equivalent() {
        let content1 = "# Skill\n\nDo something.\n";
        let content2 = "# Skill\n\nDo something.\n\n";
        assert!(are_skills_equivalent(content1, content2));
    }

    #[test]
    fn test_are_skills_not_equivalent() {
        let content1 = "# Skill\n\nDo something.\n";
        let content2 = "# Skill\n\nDo something else.\n";
        assert!(!are_skills_equivalent(content1, content2));
    }
}
