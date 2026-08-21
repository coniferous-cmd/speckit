use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json;

use crate::parsers::{ChangeParser, MarkdownParser};
use crate::schemas::{Change, Spec};

/// Converts Speckit markdown files to JSON format.
pub struct JsonConverter;

impl JsonConverter {
    /// Convert a spec markdown file to JSON.
    pub fn convert_spec_to_json(&self, file_path: &Path) -> Result<String> {
        let content = fs::read_to_string(file_path)?;
        let parser = MarkdownParser::new(&content);
        let spec_name = Self::extract_name_from_path(file_path);

        let spec = parser.parse_spec(&spec_name)?;

        let json_spec = serde_json::json!({
            "name": spec.name,
            "overview": spec.overview,
            "requirements": spec.requirements,
            "metadata": {
                "version": spec.metadata.as_ref().map(|m| m.version.as_str()).unwrap_or("1.0.0"),
                "format": "speckit",
                "sourcePath": file_path.to_string_lossy(),
            }
        });

        Ok(serde_json::to_string_pretty(&json_spec)?)
    }

    /// Convert a change markdown file to JSON.
    pub async fn convert_change_to_json(&self, file_path: &Path) -> Result<String> {
        let content = fs::read_to_string(file_path)?;
        let change_name = Self::extract_name_from_path(file_path);
        let change_dir = file_path.parent().unwrap_or(Path::new("."));
        let parser = ChangeParser::new(&content, change_dir);

        let change = parser.parse_change_with_deltas(&change_name).await?;

        let json_change = serde_json::json!({
            "name": change.name,
            "why": change.why,
            "whatChanges": change.what_changes,
            "deltas": change.deltas,
            "metadata": {
                "version": change.metadata.as_ref().map(|m| m.version.as_str()).unwrap_or("1.0.0"),
                "format": "speckit-change",
                "sourcePath": file_path.to_string_lossy(),
            }
        });

        Ok(serde_json::to_string_pretty(&json_change)?)
    }

    /// Extract the name from a file path by looking for 'specs' or 'changes' directories.
    fn extract_name_from_path(file_path: &Path) -> String {
        let components: Vec<&str> = file_path
            .components()
            .map(|c| c.as_os_str().to_str().unwrap_or(""))
            .collect();

        // Look for 'specs' or 'changes' directory
        for i in 0..components.len() {
            if components[i] == "specs" || components[i] == "changes" {
                if i + 1 < components.len() {
                    return components[i + 1].to_string();
                }
            }
        }

        // Fallback: use filename without extension
        let file_name = components.last().unwrap_or(&"");
        match file_name.rfind('.') {
            Some(idx) => file_name[..idx].to_string(),
            None => file_name.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_extract_name_from_path_specs() {
        let path = PathBuf::from("speckit/specs/user-auth/spec.md");
        assert_eq!(JsonConverter::extract_name_from_path(&path), "user-auth");
    }

    #[test]
    fn test_extract_name_from_path_changes() {
        let path = PathBuf::from("speckit/changes/add-dark-mode/proposal.md");
        assert_eq!(JsonConverter::extract_name_from_path(&path), "add-dark-mode");
    }

    #[test]
    fn test_extract_name_from_path_fallback() {
        let path = PathBuf::from("some/random/file.md");
        assert_eq!(JsonConverter::extract_name_from_path(&path), "file");
    }
}
