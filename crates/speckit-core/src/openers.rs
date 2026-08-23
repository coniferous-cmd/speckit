//! Workset openers: tools that open a workset in an IDE or agent.
//!
//! Every tool is an instance of one of two launch styles:
//! - 'workspace-file': invoke with a generated .code-workspace
//! - 'attach-dirs': pre-args plus one attach flag per member

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::worksets::WorksetMember;

/// The launch style of an opener.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OpenerStyle {
    WorkspaceFile,
    AttachDirs,
}

/// Definition of a tool opener.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenerDefinition {
    pub id: String,
    pub label: String,
    pub style: OpenerStyle,
    pub command: String,
    pub args: Vec<String>,
    pub attach_flag: String,
}

const DEFAULT_ATTACH_FLAG: &str = "--add-dir";

/// Whether CLI agent openers are enabled.
pub fn is_cli_agent_openers_enabled() -> bool {
    std::env::var("SPECKIT_ENABLE_CLI_AGENT_OPENERS").as_deref() == Ok("1")
}

/// Whether an opener is enabled right now.
pub fn is_opener_enabled(opener: &OpenerDefinition) -> bool {
    is_cli_agent_openers_enabled() || opener.style != OpenerStyle::AttachDirs
}

/// Built-in opener definitions.
pub fn builtin_openers() -> Vec<OpenerDefinition> {
    vec![
        OpenerDefinition {
            id: "code".to_string(),
            label: "VS Code".to_string(),
            style: OpenerStyle::WorkspaceFile,
            command: "code".to_string(),
            args: Vec::new(),
            attach_flag: DEFAULT_ATTACH_FLAG.to_string(),
        },
        OpenerDefinition {
            id: "cursor".to_string(),
            label: "Cursor".to_string(),
            style: OpenerStyle::WorkspaceFile,
            command: "cursor".to_string(),
            args: Vec::new(),
            attach_flag: DEFAULT_ATTACH_FLAG.to_string(),
        },
        OpenerDefinition {
            id: "claude".to_string(),
            label: "Claude Code".to_string(),
            style: OpenerStyle::AttachDirs,
            command: "claude".to_string(),
            args: Vec::new(),
            attach_flag: DEFAULT_ATTACH_FLAG.to_string(),
        },
        OpenerDefinition {
            id: "codex".to_string(),
            label: "codex".to_string(),
            style: OpenerStyle::AttachDirs,
            command: "codex".to_string(),
            args: vec!["--sandbox".to_string(), "workspace-write".to_string()],
            attach_flag: DEFAULT_ATTACH_FLAG.to_string(),
        },
    ]
}

/// Check if a command is available on PATH.
pub fn is_opener_command_available(command: &str) -> bool {
    if let Ok(path) = std::env::var("PATH") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        for dir in path.split(sep) {
            let full = PathBuf::from(dir).join(command);
            if full.exists() {
                return true;
            }
            // On Windows, also check with common extensions
            if cfg!(windows) {
                for ext in &[".exe", ".cmd", ".bat"] {
                    if full.with_extension(ext.trim_start_matches('.')).exists() {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// An opener choice with availability status.
#[derive(Debug, Clone)]
pub struct OpenerChoice {
    pub opener: OpenerDefinition,
    pub available: bool,
    pub note: Option<String>,
}

/// List opener choices with availability status.
pub fn list_opener_choices(table: &[OpenerDefinition]) -> Vec<OpenerChoice> {
    let mut choices: Vec<OpenerChoice> = table
        .iter()
        .filter(|opener| is_opener_enabled(opener))
        .map(|opener| {
            let available = is_opener_command_available(&opener.command);
            OpenerChoice {
                opener: opener.clone(),
                available,
                note: if available {
                    None
                } else {
                    Some(format!("({} not found on PATH)", opener.command))
                },
            }
        })
        .collect();

    // Sort: available first, preserving order within each group
    choices.sort_by(|a, b| {
        if a.available == b.available {
            std::cmp::Ordering::Equal
        } else if a.available {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });

    choices
}

/// Find an opener by id.
pub fn find_opener<'a>(table: &'a [OpenerDefinition], id: &str) -> Option<&'a OpenerDefinition> {
    table.iter().find(|opener| opener.id == id)
}

/// A launch command ready to execute.
#[derive(Debug, Clone)]
pub struct LaunchCommand {
    pub executable: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub label: String,
    pub style: OpenerStyle,
}

/// Build the argv for launching a workset.
pub fn build_launch_command(
    opener: &OpenerDefinition,
    members: &[WorksetMember],
    code_workspace_path: &Path,
) -> Result<LaunchCommand, String> {
    if members.is_empty() {
        return Err("build_launch_command requires at least one member.".to_string());
    }

    if !code_workspace_path.is_absolute() {
        return Err(format!(
            "build_launch_command requires an absolute workspace-file path (got '{}').",
            code_workspace_path.display()
        ));
    }

    let cwd = members[0].path.clone();

    if opener.style == OpenerStyle::WorkspaceFile {
        let mut args = opener.args.clone();
        args.push(code_workspace_path.to_string_lossy().to_string());
        return Ok(LaunchCommand {
            executable: opener.command.clone(),
            args,
            cwd,
            label: opener.label.clone(),
            style: opener.style.clone(),
        });
    }

    // attach-dirs style
    let mut args = opener.args.clone();
    for member in members {
        args.push(opener.attach_flag.clone());
        args.push(member.path.clone());
    }

    Ok(LaunchCommand {
        executable: opener.command.clone(),
        args,
        cwd,
        label: opener.label.clone(),
        style: opener.style.clone(),
    })
}

/// Merge user openers config over built-in table.
pub fn merge_opener_table(raw_openers: Option<&serde_json::Value>) -> Vec<OpenerDefinition> {
    let mut table = builtin_openers();

    let openers = match raw_openers {
        Some(v) => v,
        None => return table,
    };

    let obj = match openers.as_object() {
        Some(o) => o,
        None => return table,
    };

    for (id, row) in obj {
        let row_obj = match row.as_object() {
            Some(o) => o,
            None => continue,
        };

        let builtin_index = table.iter().position(|o| o.id == *id);

        if let Some(idx) = builtin_index {
            let builtin = &mut table[idx];
            if let Some(style) = row_obj.get("style").and_then(|v| v.as_str()) {
                builtin.style = match style {
                    "workspace-file" => OpenerStyle::WorkspaceFile,
                    "attach-dirs" => OpenerStyle::AttachDirs,
                    _ => continue,
                };
            }
            if let Some(label) = row_obj.get("label").and_then(|v| v.as_str()) {
                builtin.label = label.to_string();
            }
            if let Some(command) = row_obj.get("command").and_then(|v| v.as_str()) {
                builtin.command = command.to_string();
            }
            if let Some(args) = row_obj.get("args").and_then(|v| v.as_array()) {
                builtin.args = args
                    .iter()
                    .filter_map(|a| a.as_str().map(String::from))
                    .collect();
            }
            if let Some(flag) = row_obj.get("attach_flag").and_then(|v| v.as_str()) {
                builtin.attach_flag = flag.to_string();
            }
        } else {
            let style_str = match row_obj.get("style").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => continue, // New tool must specify style
            };
            let style = match style_str {
                "workspace-file" => OpenerStyle::WorkspaceFile,
                "attach-dirs" => OpenerStyle::AttachDirs,
                _ => continue,
            };
            table.push(OpenerDefinition {
                id: id.clone(),
                label: row_obj
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or(id)
                    .to_string(),
                style,
                command: row_obj
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or(id)
                    .to_string(),
                args: row_obj
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                attach_flag: row_obj
                    .get("attach_flag")
                    .and_then(|v| v.as_str())
                    .unwrap_or(DEFAULT_ATTACH_FLAG)
                    .to_string(),
            });
        }
    }

    table
}
