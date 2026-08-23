//! Init Command: sets up Speckit with agent skills and slash commands.
//!
//! This is the unified setup command that configures tools, generates
//! skill files and commands, handles legacy cleanup, and creates config.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::command_generation::{self, CommandAdapterRegistry, Delivery as CommandDelivery};
use crate::config::{self, SPECKIT_DIR_NAME};
use crate::global_config::{self, Profile};
use crate::legacy_cleanup::{self, LegacyDetectionResult};
use crate::planning_home;
use crate::profiles;

/// Options for the init command.
#[derive(Debug, Clone, Default)]
pub struct InitCommandOptions {
    pub tools: Option<String>,
    pub force: bool,
    pub interactive: Option<bool>,
    pub profile: Option<String>,
    pub animation: bool,
    pub copilot_cloud: Option<bool>,
}

/// A validated tool selected for init.
#[derive(Debug, Clone)]
pub struct ValidatedInitTool {
    pub value: String,
    pub name: String,
    pub skills_dir: Option<String>,
    pub skills_path: PathBuf,
    pub skills_root: PathBuf,
    pub is_global_skill_target: bool,
    pub was_configured: bool,
    pub requires_ide_restart: bool,
}

/// Holds deferred legacy cleanup data.
#[derive(Debug, Clone)]
struct DeferredLegacyCleanup {
    detection: LegacyDetectionResult,
}

/// Skills generation result.
#[derive(Debug, Default)]
pub struct GenerationResults {
    pub created_tools: Vec<ValidatedInitTool>,
    pub refreshed_tools: Vec<ValidatedInitTool>,
    pub failed_tools: Vec<(String, String)>,
    pub commands_skipped: Vec<String>,
    pub skills_invocable_command_skips: Vec<String>,
    pub removed_command_count: usize,
    pub removed_skill_count: usize,
}

/// The init command implementation.
pub struct InitCommand {
    tools_arg: Option<String>,
    force: bool,
    interactive_option: Option<bool>,
    profile_override: Option<String>,
    animation: bool,
    copilot_cloud_option: Option<bool>,
}

impl InitCommand {
    /// Create a new init command with the given options.
    pub fn new(options: InitCommandOptions) -> Self {
        Self {
            tools_arg: options.tools,
            force: options.force,
            interactive_option: options.interactive,
            profile_override: options.profile,
            animation: options.animation,
            copilot_cloud_option: options.copilot_cloud,
        }
    }

    /// Execute the init command on the given project path.
    pub fn execute(&self, target_path: &Path) -> Result<()> {
        let project_path =
            dunce::canonicalize(target_path).unwrap_or_else(|_| target_path.to_path_buf());
        let speckit_path = project_path.join(SPECKIT_DIR_NAME);

        // Validate permissions
        let extend_mode = self.validate(&project_path, &speckit_path)?;

        // Pointer guard: check for externalized planning
        if let Some(guard_root) = planning_home::find_repo_planning_root_sync(Some(&project_path)) {
            let config_path = guard_root.join(SPECKIT_DIR_NAME).join("config.yaml");
            let config_yml_path = guard_root.join(SPECKIT_DIR_NAME).join("config.yml");
            if !config_path.exists() && !config_yml_path.exists() {
                let store_path = guard_root.join(SPECKIT_DIR_NAME).join("store");
                if store_path.exists() {
                    return Err(anyhow::anyhow!(
                        "This repo's planning is externalized. Remove the store file first to \
                         convert this repo to a local Speckit root."
                    ));
                }
            }
        }

        // Handle legacy cleanup
        let deferred_legacy_cleanup = self.handle_legacy_cleanup(&project_path, extend_mode)?;

        // Create directory structure
        self.create_directory_structure(&speckit_path, extend_mode)?;

        // Determine which tools to set up
        let selected_tools = self.resolve_tools_arg().unwrap_or_default();
        let validated_tools = self.validate_tools(&selected_tools, &project_path)?;

        let (_profile, workflow_filter) = self.resolve_workflow_filter(&project_path)?;
        let workflows = workflow_filter
            .clone()
            .unwrap_or_else(profiles::all_workflow_strings);

        // The welcome screen is intentionally limited to interactive init. In
        // particular, `--tools` and CI/non-TTY invocations must never wait for
        // input. `animation` still controls the interactive rendering branch.
        if self.can_prompt_interactively() {
            crate::ui::welcome_screen::show_welcome_screen(&workflows, Some(self.animation))
                .context("Failed to render welcome screen")?;
        }

        // Generate skills and commands
        let results = self.generate_skills_and_commands(
            &project_path,
            &validated_tools,
            workflow_filter.as_deref(),
        )?;

        // Finalize deferred legacy cleanup
        if let Some(ref deferred) = deferred_legacy_cleanup {
            self.finalize_deferred_legacy_cleanup(&project_path, deferred)?;
        }

        // Create config.yaml if needed
        let config_status = self.create_config(&speckit_path, extend_mode)?;

        if validated_tools
            .iter()
            .any(|tool| tool.value == "github-copilot")
        {
            let opt_in = self
                .copilot_cloud_option
                .or_else(|| crate::github_copilot::read_copilot_cloud_opt_in(&project_path))
                .or_else(|| {
                    crate::github_copilot::has_existing_managed_cloud_files(&project_path)
                        .then_some(true)
                });
            if let Some(value) = opt_in {
                crate::github_copilot::write_copilot_cloud_files(&project_path, Some(value))?;
                crate::github_copilot::persist_copilot_cloud_opt_in(&project_path, value)?;
            }
        } else if self.copilot_cloud_option.is_some() {
            println!("GitHub Copilot cloud setup ignored: github-copilot is not selected.");
        }

        // Display success
        self.display_success_message(&project_path, &validated_tools, &results, &config_status);

        if !results.failed_tools.is_empty() {
            let failed_names: Vec<&str> = results
                .failed_tools
                .iter()
                .map(|(name, _)| name.as_str())
                .collect();
            return Err(anyhow::anyhow!(
                "Speckit setup failed for: {}",
                failed_names.join(", ")
            ));
        }

        Ok(())
    }

    /// Validate the project path and check permissions.
    fn validate(&self, project_path: &Path, speckit_path: &Path) -> Result<bool> {
        let extend_mode = speckit_path.exists();

        if !project_path.exists() {
            return Err(anyhow::anyhow!(
                "Project path does not exist: {}",
                project_path.display()
            ));
        }

        let test_file = project_path.join(".speckit-write-test");
        match fs::write(&test_file, "") {
            Ok(_) => {
                let _ = fs::remove_file(&test_file);
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Insufficient permissions to write to {}: {}",
                    project_path.display(),
                    e
                ));
            }
        }

        Ok(extend_mode)
    }

    /// Check if the command can prompt interactively.
    fn can_prompt_interactively(&self) -> bool {
        if self.interactive_option == Some(false) {
            return false;
        }
        if self.tools_arg.is_some() {
            return false;
        }
        self.interactive_option
            .unwrap_or_else(|| std::io::IsTerminal::is_terminal(&std::io::stdin()))
    }

    /// Handle legacy cleanup detection and execution.
    fn handle_legacy_cleanup(
        &self,
        project_path: &Path,
        _extend_mode: bool,
    ) -> Result<Option<DeferredLegacyCleanup>> {
        let detection = legacy_cleanup::detect_legacy_artifacts(project_path)?;

        if !detection.has_legacy_artifacts {
            return Ok(None);
        }

        let immediate_detection = legacy_cleanup::omit_global_legacy_prompt_files(&detection);

        let immediate_summary = legacy_cleanup::format_detection_summary(&immediate_detection);
        if !immediate_summary.is_empty() {
            println!();
            println!("{}", immediate_summary);
            println!();
        }

        let deferred_summary = legacy_cleanup::format_deferred_global_prompt_summary(&detection);
        if !deferred_summary.is_empty() {
            println!("{}", deferred_summary);
            println!();
        }

        if self.force || !self.can_prompt_interactively() {
            self.perform_immediate_legacy_cleanup(project_path, &immediate_detection)?;
            if !detection.global_slash_command_files.is_empty() {
                return Ok(Some(DeferredLegacyCleanup { detection }));
            }
            return Ok(None);
        }

        // Interactive prompt
        println!("Upgrade and clean up legacy files? [Y/n]");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let should_cleanup = input.trim().is_empty() || input.trim().to_lowercase() == "y";

        if !should_cleanup {
            println!("Initialization cancelled.");
            println!("Run with --force to skip this prompt, or manually remove legacy files.");
            std::process::exit(0);
        }

        self.perform_immediate_legacy_cleanup(project_path, &immediate_detection)?;
        if !detection.global_slash_command_files.is_empty() {
            return Ok(Some(DeferredLegacyCleanup { detection }));
        }
        Ok(None)
    }

    /// Perform immediate (non-deferred) legacy cleanup.
    fn perform_immediate_legacy_cleanup(
        &self,
        project_path: &Path,
        detection: &LegacyDetectionResult,
    ) -> Result<()> {
        if !detection.has_legacy_artifacts {
            return Ok(());
        }
        let result = legacy_cleanup::cleanup_legacy_artifacts(project_path, detection)?;
        let summary = legacy_cleanup::format_cleanup_summary(&result);
        if !summary.is_empty() {
            println!();
            println!("{}", summary);
        }
        println!();
        Ok(())
    }

    /// Finalize deferred legacy cleanup after skill generation.
    fn finalize_deferred_legacy_cleanup(
        &self,
        project_path: &Path,
        deferred: &DeferredLegacyCleanup,
    ) -> Result<()> {
        let removable_matches =
            legacy_cleanup::get_legacy_global_prompt_matches(&deferred.detection);

        if !removable_matches.is_empty() {
            let pick_paths: Vec<&str> = removable_matches.iter().map(|m| m.path.as_str()).collect();
            let picked =
                legacy_cleanup::pick_global_legacy_prompt_files(&deferred.detection, &pick_paths);
            self.perform_immediate_legacy_cleanup(project_path, &picked)?;
        }

        Ok(())
    }

    /// Resolve the --tools argument into a list of tool ids.
    fn resolve_tools_arg(&self) -> Option<Vec<String>> {
        let raw = self.tools_arg.as_ref()?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Some(vec![]);
        }

        let lower = trimmed.to_lowercase();
        if lower == "all" {
            return Some(config::all_tool_ids());
        }
        if lower == "none" {
            return Some(vec![]);
        }

        let tokens: Vec<String> = trimmed
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();

        if tokens.is_empty() {
            return Some(vec![]);
        }

        let mut seen = HashSet::new();
        let mut deduped = Vec::new();
        for token in tokens {
            let resolved = config::resolve_tool_id_alias(&token.to_lowercase()).to_string();
            if !seen.contains(&resolved) {
                seen.insert(resolved.clone());
                deduped.push(resolved);
            }
        }

        Some(deduped)
    }

    /// Validate and resolve tool ids into ValidatedInitTool structs.
    fn validate_tools(
        &self,
        tool_ids: &[String],
        project_path: &Path,
    ) -> Result<Vec<ValidatedInitTool>> {
        let mut validated = Vec::new();

        for tool_id in tool_ids {
            let tool = config::find_tool(tool_id)
                .ok_or_else(|| anyhow::anyhow!("Unknown tool '{}'.", tool_id))?;

            let skills_dir = tool.skills_dir.clone().unwrap_or_default();
            let skills_path = project_path.join(&skills_dir).join("skills");
            validated.push(ValidatedInitTool {
                value: tool.value.clone(),
                name: tool.name.clone(),
                skills_dir: tool.skills_dir.clone(),
                skills_path: skills_path.clone(),
                skills_root: project_path.to_path_buf(),
                is_global_skill_target: tool.global_skills_dir.is_some(),
                was_configured: false,
                requires_ide_restart: tool.requires_ide_restart.unwrap_or(false),
            });
        }

        Ok(validated)
    }

    /// Create the Speckit directory structure.
    fn create_directory_structure(&self, speckit_path: &Path, _extend_mode: bool) -> Result<()> {
        let dirs = [
            speckit_path.to_path_buf(),
            speckit_path.join("specs"),
            speckit_path.join("changes"),
            speckit_path.join("changes").join("archive"),
        ];

        for dir in &dirs {
            fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create directory: {}", dir.display()))?;
        }

        Ok(())
    }

    /// Generate skills and commands for validated tools.
    ///
    /// Mirrors OpenSpec's `generateSkillsAndCommands` from
    /// `openspec/src/core/init.ts:852-982`. Skills are always generated when
    /// delivery includes them; commands are generated for adapter-backed tools
    /// when delivery is `Both` or `Commands`.
    fn generate_skills_and_commands(
        &self,
        project_path: &Path,
        tools: &[ValidatedInitTool],
        workflow_filter: Option<&[String]>,
    ) -> Result<GenerationResults> {
        let mut results = GenerationResults::default();

        // Resolve delivery from global config (mirrors OpenSpec line 877)
        let global_cfg = global_config::get_global_config();
        let delivery = match global_cfg.delivery {
            global_config::Delivery::Both => CommandDelivery::Both,
            global_config::Delivery::Skills => CommandDelivery::Skills,
            global_config::Delivery::Commands => CommandDelivery::Commands,
        };
        let delivery_includes_commands = delivery != CommandDelivery::Skills;

        // Pre-fetch command contents once for all tools (mirrors OpenSpec line 882)
        let command_contents =
            if delivery_includes_commands {
                crate::templates::generation::get_command_contents(workflow_filter)
            } else {
                Vec::new()
            };

        for tool in tools {
            println!("Setting up {}...", tool.name);

            let should_generate_skills =
                command_generation::should_generate_skills_for_tool(&tool.value, delivery);
            let should_generate_commands =
                command_generation::should_generate_commands_for_tool(&tool.value, delivery);

            // --- Skills ---
            if should_generate_skills {
                fs::create_dir_all(&tool.skills_path).with_context(|| {
                    format!(
                        "Failed to create skills directory: {}",
                        tool.skills_path.display()
                    )
                })?;

                let skill_entries =
                    crate::templates::generation::get_skill_templates(workflow_filter);
                let generated_by_version =
                    crate::templates::generation::speckit_generated_by_version();
                for entry in &skill_entries {
                    let skill_dir = tool.skills_path.join(&entry.dir_name);
                    fs::create_dir_all(&skill_dir)?;

                    let skill_file = skill_dir.join("SKILL.md");
                    let content = crate::templates::generation::generate_skill_content(
                        &entry.template,
                        &generated_by_version,
                        None,
                    );
                    if !skill_file.exists() || self.force {
                        fs::write(&skill_file, content)?;
                    }
                }
            }

            // --- Commands (mirrors OpenSpec lines 922-938) ---
            if should_generate_commands {
                if let Some(adapter) = CommandAdapterRegistry::global().get(&tool.value) {
                    let generated_commands =
                        command_generation::generate_commands(&command_contents, adapter);
                    for cmd in &generated_commands {
                        let command_file = project_path.join(&cmd.path);
                        if let Some(parent) = command_file.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        if !command_file.exists() || self.force {
                            fs::write(&command_file, &cmd.file_content)?;
                        }
                    }
                }
            } else if delivery_includes_commands {
                // Track skipped tools (mirrors OpenSpec lines 933-939)
                let capability =
                    command_generation::resolve_command_surface_capability(&tool.value);
                if capability == command_generation::CommandSurfaceCapability::SkillsInvocable {
                    results.skills_invocable_command_skips.push(tool.value.clone());
                } else {
                    results.commands_skipped.push(tool.value.clone());
                }
            }

            // Reconcile: remove stale command files when delivery is skills-only
            if command_generation::should_reconcile_command_files_for_tool(&tool.value, delivery) {
                if let Some(adapter) = CommandAdapterRegistry::global().get(&tool.value) {
                    let command_dir_name = adapter.get_file_path("").rsplit_once('/').map(|(dir, _)| dir.to_string());
                    if let Some(dir) = command_dir_name {
                        let dir_path = project_path.join(&dir);
                        if dir_path.exists() {
                            // Remove command files that are no longer in the workflow set
                            let active_ids: HashSet<&str> = command_contents.iter().map(|c| c.id.as_str()).collect();
                            if let Ok(rd) = fs::read_dir(&dir_path) {
                                for entry in rd.flatten() {
                                    let path = entry.path();
                                    if !path.is_file() { continue; }
                                    let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                                    if !active_ids.contains(file_stem) {
                                        let _ = fs::remove_file(&path);
                                        results.removed_command_count += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if tool.was_configured {
                results.refreshed_tools.push(tool.clone());
            } else {
                results.created_tools.push(tool.clone());
            }

            println!("Setup complete for {}", tool.name);
        }

        Ok(results)
    }

    /// Resolve the profile once for all init outputs. CLI values are validated
    /// here so an unknown profile cannot silently fall back to all workflows.
    fn resolve_workflow_filter(
        &self,
        project_path: &Path,
    ) -> Result<(Profile, Option<Vec<String>>)> {
        let override_profile = match self.profile_override.as_deref() {
            None => None,
            Some("core") => Some(Profile::Core),
            Some("custom") | Some("expanded") => Some(Profile::Custom),
            Some(other) => {
                return Err(anyhow::anyhow!(
                    "Unknown profile '{other}'. Supported profiles: core, expanded, custom."
                ));
            }
        };

        let (profile, filter) = profiles::resolve_profile_and_workflow_filter(
            override_profile.as_ref(),
            Some(project_path),
        );
        if self.profile_override.as_deref() == Some("expanded") {
            return Ok((profile, None));
        }

        // A custom profile may declare workflows in the project config. The
        // shared resolver already handles global workflows; project values
        // take precedence for init as OpenSpec does.
        if override_profile == Some(Profile::Custom)
            && let Ok(value) = std::fs::read_to_string(project_path.join("speckit/config.yaml"))
            && let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(&value)
            && let Some(workflows) = yaml.get("workflows").and_then(|v| v.as_sequence())
        {
            let selected = workflows
                .iter()
                .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>();
            let unknown = selected
                .iter()
                .filter(|wf| !profiles::ALL_WORKFLOWS.contains(&wf.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            if !unknown.is_empty() {
                return Err(anyhow::anyhow!(
                    "Unknown workflow(s) in custom profile: {}",
                    unknown.join(", ")
                ));
            }
            return Ok((Profile::Custom, Some(selected)));
        }
        Ok((profile, filter))
    }

    /// Create the config.yaml file if it does not exist.
    fn create_config(&self, speckit_path: &Path, _extend_mode: bool) -> Result<&'static str> {
        let config_path = speckit_path.join("config.yaml");
        let config_yml_path = speckit_path.join("config.yml");

        if config_path.exists() || config_yml_path.exists() {
            return Ok("exists");
        }

        let yaml_content = "schema: spec-driven\n";
        match fs::write(&config_path, yaml_content) {
            Ok(_) => Ok("created"),
            Err(_) => Ok("skipped"),
        }
    }

    /// Display the success message after init.
    fn display_success_message(
        &self,
        _project_path: &Path,
        _tools: &[ValidatedInitTool],
        results: &GenerationResults,
        config_status: &str,
    ) {
        println!();
        if results.failed_tools.is_empty() {
            println!("Speckit Setup Complete");
        } else {
            println!("Speckit Setup Incomplete");
        }
        println!();

        if !results.created_tools.is_empty() {
            let names: Vec<&str> = results
                .created_tools
                .iter()
                .map(|t| t.name.as_str())
                .collect();
            println!("Created: {}", names.join(", "));
        }
        if !results.refreshed_tools.is_empty() {
            let names: Vec<&str> = results
                .refreshed_tools
                .iter()
                .map(|t| t.name.as_str())
                .collect();
            println!("Refreshed: {}", names.join(", "));
        }

        if !results.failed_tools.is_empty() {
            let failures: Vec<String> = results
                .failed_tools
                .iter()
                .map(|(name, error)| format!("{} ({})", name, error))
                .collect();
            println!("Failed: {}", failures.join(", "));
        }

        match config_status {
            "created" => println!("Config: speckit/config.yaml (schema: spec-driven)"),
            "exists" => println!("Config: speckit/config.yaml (exists)"),
            _ => println!("Config: skipped"),
        }

        println!();
    }
}

// Add helper to config module
use crate::config::AI_TOOLS;

/// Find a tool by id (exposed from config).
pub fn find_tool(tool_id: &str) -> Option<&'static config::AiToolOption> {
    AI_TOOLS.iter().find(|t| t.value == tool_id)
}

/// Return all tool ids.
pub fn all_tool_ids() -> Vec<String> {
    AI_TOOLS.iter().map(|t| t.value.clone()).collect()
}
