pub mod allowed_tools;
pub mod skill_content_equivalence;
pub mod skill_generation;
pub mod skill_paths;
pub mod tool_detection;

// Re-export key types for convenience.
pub use allowed_tools::{
    AllowedToolsConfig, default_allowed_tool_names, read_allowed_tools_config,
    resolve_allowed_tools,
};
pub use skill_generation::{
    GeneratedSkill, SkillMetadata, generate_changes_skill, generate_claude_skill,
    generate_skill_content, generate_specs_summary_skill, write_skill,
};
pub use skill_paths::{
    SkillPaths, claude_commands_dir, claude_instructions_path, claude_settings_path,
};
pub use tool_detection::{
    DetectedTool, detect_tool_ids, detect_tools, has_any_tool_marker, has_claude_md,
};
