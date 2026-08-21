/// Archive change workflow template.

pub const ARCHIVE_CHANGE_TEMPLATE: &str = r#"# Archive Change

Archive a completed change and update main specs.

## Steps

1. Validate the change is complete
2. Update main specs with delta changes
3. Move change to archive directory
4. Update project metadata
"#;
