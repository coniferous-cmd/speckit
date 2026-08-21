use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Standard skill directory names used by Speckit.
const SKILL_DIR_NAME: &str = "skills";
const SPECKIT_DIR_NAME: &str = "speckit";

/// Resolved paths for skill generation output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPaths {
    /// Root directory for skills (e.g. `<project>/.claude/skills`).
    pub skills_dir: PathBuf,
    /// Path to the Speckit specs directory.
    pub specs_dir: PathBuf,
    /// Path to the Speckit changes directory.
    pub changes_dir: PathBuf,
    /// Path to the Speckit archive directory.
    pub archive_dir: PathBuf,
}

impl SkillPaths {
    /// Resolves skill paths for a given project root and tool.
    ///
    /// `tool_id` determines the base directory:
    /// - `"claude"` → `.claude/skills/`
    /// - others → `.speckit/skills/<tool_id>/`
    pub fn resolve(project_root: &Path, tool_id: &str) -> Self {
        let speckit_dir = project_root.join(SPECKIT_DIR_NAME);

        let skills_dir = match tool_id {
            "claude" => project_root.join(".claude").join(SKILL_DIR_NAME),
            _ => speckit_dir.join(SKILL_DIR_NAME).join(tool_id),
        };

        Self {
            skills_dir,
            specs_dir: speckit_dir.join("specs"),
            changes_dir: speckit_dir.join("changes"),
            archive_dir: speckit_dir.join("changes").join("archive"),
        }
    }

    /// Returns the path to a specific skill file within the skills directory.
    pub fn skill_file(&self, skill_name: &str) -> PathBuf {
        self.skills_dir.join(format!("{skill_name}.md"))
    }

    /// Returns the path to a specific spec file within the specs directory.
    pub fn spec_file(&self, spec_name: &str) -> PathBuf {
        self.specs_dir.join(format!("{spec_name}.md"))
    }

    /// Returns the path to a specific change file within the changes directory.
    pub fn change_file(&self, change_name: &str) -> PathBuf {
        self.changes_dir.join(format!("{change_name}.md"))
    }
}

/// Resolves the CLAUDE.md instructions file path for a project.
pub fn claude_instructions_path(project_root: &Path) -> PathBuf {
    project_root.join("CLAUDE.md")
}

/// Resolves the `.claude/settings.json` path for a project.
pub fn claude_settings_path(project_root: &Path) -> PathBuf {
    project_root.join(".claude").join("settings.json")
}

/// Resolves the `.claude/commands/` directory for custom slash commands.
pub fn claude_commands_dir(project_root: &Path) -> PathBuf {
    project_root.join(".claude").join("commands")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_paths_resolve_claude() {
        let paths = SkillPaths::resolve(Path::new("/project"), "claude");
        assert_eq!(paths.skills_dir, PathBuf::from("/project/.claude/skills"));
        assert_eq!(paths.specs_dir, PathBuf::from("/project/speckit/specs"));
        assert_eq!(paths.changes_dir, PathBuf::from("/project/speckit/changes"));
        assert_eq!(
            paths.archive_dir,
            PathBuf::from("/project/speckit/changes/archive")
        );
    }

    #[test]
    fn skill_paths_resolve_other_tool() {
        let paths = SkillPaths::resolve(Path::new("/project"), "cursor");
        assert_eq!(
            paths.skills_dir,
            PathBuf::from("/project/speckit/skills/cursor")
        );
    }

    #[test]
    fn skill_file_path() {
        let paths = SkillPaths::resolve(Path::new("/project"), "claude");
        assert_eq!(
            paths.skill_file("my-skill"),
            PathBuf::from("/project/.claude/skills/my-skill.md")
        );
    }

    #[test]
    fn claude_paths() {
        assert_eq!(
            claude_instructions_path(Path::new("/project")),
            PathBuf::from("/project/CLAUDE.md")
        );
        assert_eq!(
            claude_settings_path(Path::new("/project")),
            PathBuf::from("/project/.claude/settings.json")
        );
        assert_eq!(
            claude_commands_dir(Path::new("/project")),
            PathBuf::from("/project/.claude/commands")
        );
    }
}
