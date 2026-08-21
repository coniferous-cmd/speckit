use clap::{Args, Parser, Subcommand};
use std::process;

use speckit_commands::{
    archive, change, completion, config, context, doctor, feedback, init, schema, shared_output,
    show, spec, store, update, validate, view, workflow, workset,
};

/// Speckit - AI-native system for spec-driven development
#[derive(Parser)]
#[command(
    name = "speckit",
    version,
    about = "AI-native system for spec-driven development"
)]
struct Cli {
    /// Disable color output
    #[arg(long = "no-color", global = true)]
    no_color: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize Speckit in your project
    Init {
        /// Target path
        path: Option<String>,
        /// Configure AI tools non-interactively
        #[arg(long)]
        tools: Option<String>,
        /// Auto-cleanup legacy files without prompting
        #[arg(long)]
        force: bool,
        /// Override global config profile
        #[arg(long)]
        profile: Option<String>,
        /// Show a static welcome screen instead of the animated one
        #[arg(long = "no-animation")]
        no_animation: bool,
        /// Set up GitHub Copilot cloud coding-agent files without prompting
        #[arg(long = "copilot-cloud")]
        copilot_cloud: bool,
        /// Skip GitHub Copilot cloud coding-agent files without prompting
        #[arg(long = "no-copilot-cloud")]
        no_copilot_cloud: bool,
    },

    /// Update Speckit instruction files
    Update {
        /// Target path
        path: Option<String>,
        /// Force update even when tools are up to date
        #[arg(long)]
        force: bool,
    },

    /// List items (changes by default). Use --specs to list specs.
    List {
        /// List specs instead of changes
        #[arg(long)]
        specs: bool,
        /// List changes explicitly (default)
        #[arg(long)]
        changes: bool,
        /// Sort order: "recent" (default) or "name"
        #[arg(long, default_value = "recent")]
        sort: String,
        /// Output as JSON (for programmatic use)
        #[arg(long)]
        json: bool,
        /// Store ID
        #[arg(long)]
        store: Option<String>,
    },

    /// Display an interactive dashboard of specs and changes
    View {
        /// Store ID
        #[arg(long)]
        store: Option<String>,
    },

    /// Manage Speckit change proposals (deprecated)
    Change {
        #[command(subcommand)]
        command: ChangeCommands,
    },

    /// Archive a completed change and update main specs
    Archive {
        /// Change name
        change_name: Option<String>,
        /// Skip confirmation prompts
        #[arg(short = 'y', long = "yes")]
        yes: bool,
        /// Skip spec update operations
        #[arg(long = "skip-specs")]
        skip_specs: bool,
        /// Skip validation (not recommended)
        #[arg(long = "no-validate")]
        no_validate: bool,
        /// Output as JSON (non-interactive)
        #[arg(long)]
        json: bool,
        /// Store ID
        #[arg(long)]
        store: Option<String>,
    },

    /// Manage and view Speckit specifications (deprecated)
    Spec {
        #[command(subcommand)]
        command: SpecCommands,
    },

    /// View and modify global Speckit configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// Manage workflow schemas [experimental]
    Schema {
        #[command(subcommand)]
        command: SchemaCommands,
    },

    /// Create and manage stores - standalone Speckit repos
    Store {
        #[command(subcommand)]
        command: Option<StoreCommands>,
        /// Unknown args catch-all
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Report relationship health for the resolved Speckit root
    Doctor {
        /// Store ID
        #[arg(long)]
        store: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Print the working context for the resolved Speckit root
    Context {
        /// Store ID
        #[arg(long)]
        store: Option<String>,
        /// Output the agent brief as JSON
        #[arg(long)]
        json: bool,
        /// Also write a VS Code workspace file for the set
        #[arg(long = "code-workspace")]
        code_workspace: Option<String>,
        /// Overwrite an existing --code-workspace file
        #[arg(long)]
        force: bool,
    },

    /// Validate changes and specs
    Validate {
        /// Item name
        item_name: Option<String>,
        /// Validate all changes and specs
        #[arg(long)]
        all: bool,
        /// Validate all changes
        #[arg(long)]
        changes: bool,
        /// Validate all specs
        #[arg(long)]
        specs: bool,
        /// Validate archived changes have all tasks completed
        #[arg(long)]
        archived: bool,
        /// Specify item type when ambiguous: change|spec
        #[arg(long = "type")]
        item_type: Option<String>,
        /// Enable strict validation mode
        #[arg(long)]
        strict: bool,
        /// Output validation results as JSON
        #[arg(long)]
        json: bool,
        /// Max concurrent validations
        #[arg(long)]
        concurrency: Option<String>,
        /// Disable interactive prompts
        #[arg(long = "no-interactive")]
        no_interactive: bool,
        /// Store ID
        #[arg(long)]
        store: Option<String>,
    },

    /// Show a change or spec
    Show {
        /// Item name
        item_name: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Specify item type when ambiguous: change|spec
        #[arg(long = "type")]
        item_type: Option<String>,
        /// Disable interactive prompts
        #[arg(long = "no-interactive")]
        no_interactive: bool,
        /// Show only deltas (JSON only, change)
        #[arg(long = "deltas-only")]
        deltas_only: bool,
        /// Alias for --deltas-only (deprecated, change)
        #[arg(long = "requirements-only")]
        requirements_only: bool,
        /// JSON only: Show only requirements (exclude scenarios)
        #[arg(long)]
        requirements: bool,
        /// JSON only: Exclude scenario content
        #[arg(long = "no-scenarios")]
        no_scenarios: bool,
        /// JSON only: Show specific requirement by ID (1-based)
        #[arg(short = 'r', long = "requirement")]
        requirement: Option<String>,
        /// Store ID
        #[arg(long)]
        store: Option<String>,
    },

    /// Submit feedback about Speckit
    Feedback {
        /// Feedback message
        message: String,
        /// Detailed description for the feedback
        #[arg(long)]
        body: Option<String>,
    },

    /// Manage shell completions for Speckit CLI
    Completion {
        #[command(subcommand)]
        command: CompletionCommands,
    },

    /// Display artifact completion status for a change
    Status {
        /// Change name to show status for
        #[arg(long)]
        change: Option<String>,
        /// Schema override
        #[arg(long)]
        schema: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Store ID
        #[arg(long)]
        store: Option<String>,
    },

    /// Output enriched instructions for artifacts
    Instructions {
        /// Artifact ID (or "apply" / "archive")
        artifact: Option<String>,
        /// Change name
        #[arg(long)]
        change: Option<String>,
        /// Schema override
        #[arg(long)]
        schema: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Store ID
        #[arg(long)]
        store: Option<String>,
    },

    /// Show resolved template paths for all artifacts in a schema
    Templates {
        /// Schema to use
        #[arg(long)]
        schema: Option<String>,
        /// Output as JSON mapping artifact IDs to template paths
        #[arg(long)]
        json: bool,
    },

    /// List available workflow schemas with descriptions
    Schemas {
        /// Output as JSON (for agent use)
        #[arg(long)]
        json: bool,
        /// Store ID
        #[arg(long)]
        store: Option<String>,
    },

    /// Create new items
    New {
        #[command(subcommand)]
        command: NewCommands,
    },

    /// Compose, keep, and open personal working views (purely local)
    Workset {
        #[command(subcommand)]
        command: Option<WorksetCommands>,
        /// JSON output flag at group level
        #[arg(long, hide = true)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ChangeCommands {
    /// Show a change proposal in JSON or markdown format
    Show {
        /// Change name
        change_name: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Show only deltas (JSON only)
        #[arg(long = "deltas-only")]
        deltas_only: bool,
        /// Alias for --deltas-only (deprecated)
        #[arg(long = "requirements-only")]
        requirements_only: bool,
        /// Disable interactive prompts
        #[arg(long = "no-interactive")]
        no_interactive: bool,
    },

    /// List all active changes (DEPRECATED: use "speckit list" instead)
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Show id and title with counts
        #[arg(long)]
        long: bool,
    },

    /// Validate a change proposal
    Validate {
        /// Change name
        change_name: Option<String>,
        /// Enable strict validation mode
        #[arg(long)]
        strict: bool,
        /// Output validation report as JSON
        #[arg(long)]
        json: bool,
        /// Disable interactive prompts
        #[arg(long = "no-interactive")]
        no_interactive: bool,
    },
}

#[derive(Subcommand)]
enum SpecCommands {
    /// Display a specific specification
    Show {
        /// Spec ID
        spec_id: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// JSON only: Show only requirements (exclude scenarios)
        #[arg(long)]
        requirements: bool,
        /// JSON only: Exclude scenario content
        #[arg(long = "no-scenarios")]
        no_scenarios: bool,
        /// JSON only: Show specific requirement by ID (1-based)
        #[arg(short = 'r', long = "requirement")]
        requirement: Option<String>,
        /// Disable interactive prompts
        #[arg(long = "no-interactive")]
        no_interactive: bool,
    },

    /// List all available specifications
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Show id and title with counts
        #[arg(long)]
        long: bool,
    },

    /// Validate a specification structure
    Validate {
        /// Spec ID
        spec_id: Option<String>,
        /// Enable strict validation mode
        #[arg(long)]
        strict: bool,
        /// Output validation report as JSON
        #[arg(long)]
        json: bool,
        /// Disable interactive prompts
        #[arg(long = "no-interactive")]
        no_interactive: bool,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show config file location
    Path,

    /// Show all current settings
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Get a specific value (raw, scriptable)
    Get {
        /// Configuration key (dot-separated)
        key: String,
    },

    /// Set a value (auto-coerce types)
    Set {
        /// Configuration key (dot-separated)
        key: String,
        /// Value to set
        value: String,
        /// Force value to be stored as string
        #[arg(long)]
        string: bool,
        /// Allow setting unknown keys
        #[arg(long = "allow-unknown")]
        allow_unknown: bool,
    },

    /// Remove a key (revert to default)
    Unset {
        /// Configuration key (dot-separated)
        key: String,
    },

    /// Reset configuration to defaults
    Reset {
        /// Reset all configuration (required)
        #[arg(long)]
        all: bool,
        /// Skip confirmation prompts
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },

    /// Open config in $EDITOR
    Edit,

    /// Configure workflow profile (interactive picker or preset shortcut)
    Profile {
        /// Profile preset (currently only "core")
        preset: Option<String>,
    },
}

#[derive(Subcommand)]
enum SchemaCommands {
    /// Show where a schema resolves from
    Which {
        /// Schema name
        name: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// List all schemas with their resolution sources
        #[arg(long)]
        all: bool,
    },

    /// Validate a schema structure and templates
    Validate {
        /// Schema name
        name: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Show detailed validation steps
        #[arg(long)]
        verbose: bool,
    },

    /// Copy an existing schema to project for customization
    Fork {
        /// Source schema name
        source: String,
        /// Destination schema name
        name: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Overwrite existing destination
        #[arg(long)]
        force: bool,
    },

    /// Create a new project-local schema
    Init {
        /// Schema name
        name: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Schema description
        #[arg(long)]
        description: Option<String>,
        /// Comma-separated artifact IDs
        #[arg(long)]
        artifacts: Option<String>,
        /// Set as project default schema
        #[arg(long = "default")]
        set_default: bool,
        /// Explicitly do not set as project default
        #[arg(long = "no-default")]
        no_default: bool,
        /// Overwrite existing schema
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum StoreCommands {
    /// Create and register a local store
    Setup {
        /// Store ID
        id: Option<String>,
        /// Folder where the store should live
        #[arg(long)]
        path: Option<String>,
        /// Initialize a Git repository
        #[arg(long = "init-git")]
        init_git: bool,
        /// Canonical clone source
        #[arg(long)]
        remote: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Register an existing local store
    Register {
        /// Path to existing store
        path: Option<String>,
        /// Store ID
        #[arg(long)]
        id: Option<String>,
        /// Confirm creating store identity metadata
        #[arg(long)]
        yes: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Forget a local store registration without deleting files
    Unregister {
        /// Store ID
        id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Forget a local store registration and delete its local folder
    Remove {
        /// Store ID
        id: String,
        /// Confirm local store folder deletion
        #[arg(long)]
        yes: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// List locally registered stores
    #[command(alias = "ls")]
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Check local store registration and metadata
    Doctor {
        /// Store ID
        id: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum CompletionCommands {
    /// Generate completion script for a shell (outputs to stdout)
    Generate {
        /// Shell (bash, zsh, fish)
        shell: Option<String>,
    },

    /// Install completion script for a shell
    Install {
        /// Shell (bash, zsh, fish)
        shell: Option<String>,
        /// Show detailed installation output
        #[arg(long)]
        verbose: bool,
    },

    /// Uninstall completion script for a shell
    Uninstall {
        /// Shell (bash, zsh, fish)
        shell: Option<String>,
        /// Skip confirmation prompts
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum NewCommands {
    /// Create a new change directory
    Change {
        /// Change name
        name: String,
        /// Description to add to README.md
        #[arg(long)]
        description: Option<String>,
        /// Optional goal metadata to store with the change
        #[arg(long)]
        goal: Option<String>,
        /// Workflow schema to use
        #[arg(long)]
        schema: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Store ID
        #[arg(long)]
        store: Option<String>,
        /// No longer supported (hidden)
        #[arg(long, hide = true)]
        initiative: Option<String>,
        /// No longer supported (hidden)
        #[arg(long, hide = true)]
        areas: Option<String>,
    },
}

#[derive(Subcommand)]
enum WorksetCommands {
    /// Compose and save a named working view of folders
    Create {
        /// Workset name
        name: Option<String>,
        /// Member folder as <path> or <name>=<path>; repeatable
        #[arg(long = "member", action = clap::ArgAction::Append)]
        member: Vec<String>,
        /// Preferred tool to open this workset with
        #[arg(long)]
        tool: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show saved worksets with their members
    #[command(alias = "ls")]
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Open a saved workset in your tool
    Open {
        /// Workset name
        name: String,
        /// Open with this tool just this once
        #[arg(long)]
        tool: Option<String>,
    },

    /// Delete a saved workset (member folders are never touched)
    Remove {
        /// Workset name
        name: String,
        /// Confirm removal non-interactively
        #[arg(long)]
        yes: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Handle --no-color
    if cli.no_color {
        // SAFETY: Setting NO_COLOR is a simple flag for downstream color output.
        unsafe {
            std::env::set_var("NO_COLOR", "1");
        }
    }

    let result = match cli.command {
        Commands::Init {
            path,
            tools,
            force,
            profile,
            no_animation,
            copilot_cloud,
            no_copilot_cloud,
        } => {
            let target_path = path.as_deref().unwrap_or(".");
            let resolved = std::path::Path::new(target_path)
                .canonicalize()
                .unwrap_or_else(|_| std::path::PathBuf::from(target_path));

            if resolved.exists() && !resolved.is_dir() {
                eprintln!("Error: Path \"{target_path}\" is not a directory");
                process::exit(1);
            }

            if !resolved.exists() {
                println!("Directory \"{target_path}\" doesn't exist, it will be created.");
                if let Err(e) = std::fs::create_dir_all(&resolved) {
                    eprintln!("Error: Cannot create path \"{target_path}\": {e}");
                    process::exit(1);
                }
            }

            let copilot_cloud_option = if copilot_cloud {
                Some(true)
            } else if no_copilot_cloud {
                Some(false)
            } else {
                None
            };

            init::execute(
                &resolved,
                tools,
                force,
                profile,
                !no_animation,
                copilot_cloud_option,
            )
        }

        Commands::Update { path, force } => {
            let target_path = path.as_deref().unwrap_or(".");
            let resolved = std::path::Path::new(target_path);
            update::execute(resolved, force)
        }

        Commands::List {
            specs,
            changes: _,
            sort,
            json,
            store,
        } => {
            if specs {
                let options = spec::SpecListOptions { json, long: false };
                spec::spec_list(options, store.as_deref()).await
            } else {
                let options = change::ChangeListOptions {
                    json,
                    long: false,
                    sort,
                };
                change::change_list(options, store.as_deref()).await
            }
        }

        Commands::View { store } => {
            let target_path =
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            view::execute(&target_path)
        }

        Commands::Change { command } => {
            eprintln!(
                "Warning: The \"speckit change ...\" commands are deprecated. Prefer verb-first commands (e.g., \"speckit list\", \"speckit validate --changes\")."
            );
            match command {
                ChangeCommands::Show {
                    change_name,
                    json,
                    deltas_only,
                    requirements_only,
                    no_interactive,
                } => {
                    change::change_show(
                        change_name.as_deref(),
                        change::ChangeShowOptions {
                            json,
                            deltas_only,
                            requirements_only,
                            no_interactive,
                        },
                        None,
                    )
                    .await
                }
                ChangeCommands::List { json, long } => {
                    eprintln!(
                        "Warning: \"speckit change list\" is deprecated. Use \"speckit list\"."
                    );
                    change::change_list(
                        change::ChangeListOptions {
                            json,
                            long,
                            sort: "recent".to_string(),
                        },
                        None,
                    )
                    .await
                }
                ChangeCommands::Validate {
                    change_name,
                    strict,
                    json,
                    no_interactive,
                } => {
                    change::change_validate(
                        change_name.as_deref(),
                        change::ChangeValidateOptions {
                            strict,
                            json,
                            no_interactive,
                        },
                    )
                    .await
                }
            }
        }

        Commands::Archive {
            change_name,
            yes,
            skip_specs,
            no_validate,
            json,
            store,
        } => {
            let project_root =
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            match archive::execute(
                change_name.as_deref(),
                yes,
                skip_specs,
                no_validate,
                json,
                store,
                &project_root,
            ) {
                Ok(Some(result)) => {
                    if json {
                        shared_output::print_json(
                            &serde_json::to_value(&result).unwrap_or_default(),
                        );
                    }
                    Ok(())
                }
                Ok(None) => Ok(()),
                Err(e) => {
                    if json {
                        shared_output::print_json(&serde_json::json!({
                            "status": [{
                                "severity": "error",
                                "code": "archive_failed",
                                "message": e.to_string(),
                            }]
                        }));
                    }
                    Err(e)
                }
            }
        }

        Commands::Spec { command } => {
            eprintln!(
                "Warning: The \"speckit spec ...\" commands are deprecated. Prefer verb-first commands (e.g., \"speckit show\", \"speckit validate --specs\")."
            );
            match command {
                SpecCommands::Show {
                    spec_id,
                    json,
                    requirements,
                    no_scenarios,
                    requirement,
                    no_interactive,
                } => {
                    spec::spec_show(
                        spec_id.as_deref(),
                        spec::SpecShowOptions {
                            json,
                            requirements,
                            no_scenarios,
                            requirement,
                            no_interactive,
                        },
                        None,
                    )
                    .await
                }
                SpecCommands::List { json, long } => {
                    spec::spec_list(spec::SpecListOptions { json, long }, None).await
                }
                SpecCommands::Validate {
                    spec_id,
                    strict,
                    json,
                    no_interactive,
                } => {
                    spec::spec_validate(
                        spec_id.as_deref(),
                        spec::SpecValidateOptions {
                            strict,
                            json,
                            no_interactive,
                        },
                    )
                    .await
                }
            }
        }

        Commands::Config { command } => match command {
            ConfigCommands::Path => {
                config::config_path();
                Ok(())
            }
            ConfigCommands::List { json } => {
                config::config_list(json);
                Ok(())
            }
            ConfigCommands::Get { key } => config::config_get(&key),
            ConfigCommands::Set {
                key,
                value,
                string,
                allow_unknown,
            } => config::config_set(&key, &value, string, allow_unknown),
            ConfigCommands::Unset { key } => config::config_unset(&key),
            ConfigCommands::Reset { all, yes } => config::config_reset(all, yes),
            ConfigCommands::Edit => config::config_edit(),
            ConfigCommands::Profile { preset } => config::config_profile(preset.as_deref()),
        },

        Commands::Schema { command } => match command {
            SchemaCommands::Which { name, json, all } => {
                schema::schema_which(name.as_deref(), json, all).await
            }
            SchemaCommands::Validate {
                name,
                json,
                verbose,
            } => schema::schema_validate(name.as_deref(), json, verbose).await,
            SchemaCommands::Fork {
                source,
                name,
                json,
                force,
            } => schema::schema_fork(&source, name.as_deref(), json, force).await,
            SchemaCommands::Init {
                name,
                json,
                description,
                artifacts,
                set_default,
                no_default,
                force,
            } => {
                // --no-default explicitly overrides --default
                let effective_default = set_default && !no_default;
                schema::schema_init(
                    &name,
                    json,
                    description.as_deref(),
                    artifacts.as_deref(),
                    effective_default,
                    force,
                )
                .await
            }
        },

        Commands::Store { command, args } => {
            match command {
                Some(cmd) => match cmd {
                    StoreCommands::Setup {
                        id,
                        path,
                        init_git,
                        remote,
                        json,
                    } => {
                        store::store_setup(
                            id.as_deref(),
                            path.as_deref(),
                            Some(init_git),
                            remote,
                            json,
                        )
                        .await
                    }
                    StoreCommands::Register {
                        path,
                        id,
                        yes,
                        json,
                    } => store::store_register(path.as_deref(), id.as_deref(), yes, json).await,
                    StoreCommands::Unregister { id, json } => {
                        store::store_unregister(&id, json).await
                    }
                    StoreCommands::Remove { id, yes, json } => {
                        store::store_remove(&id, yes, json).await
                    }
                    StoreCommands::List { json } => store::store_list(json).await,
                    StoreCommands::Doctor { id, json } => {
                        store::store_doctor(id.as_deref(), json).await
                    }
                },
                None => {
                    // Bare `speckit store` with no subcommand
                    if args.contains(&"--json".to_string()) {
                        shared_output::print_json(&serde_json::json!({
                            "status": [{
                                "severity": "error",
                                "code": "unknown_store_subcommand",
                                "message": "Missing subcommand for 'speckit store'. Store subcommands: setup, register, unregister, remove, list (ls), doctor.",
                                "fix": "Run a store subcommand, or use the lifecycle command with --store <id>.",
                            }]
                        }));
                    } else {
                        eprintln!("Error: missing subcommand for 'speckit store'.");
                        eprintln!(
                            "Store subcommands manage store registration: setup, register, unregister, remove, list (ls), doctor."
                        );
                        eprintln!(
                            "To create or work on a change in a store, use the normal command with --store:"
                        );
                        eprintln!("  speckit new change <change-id> --store <id>");
                    }
                    process::exit(1);
                }
            }
        }

        Commands::Doctor { store, json } => doctor::doctor_command(store.as_deref(), json).await,

        Commands::Context {
            store,
            json,
            code_workspace,
            force,
        } => {
            context::context_command(store.as_deref(), json, code_workspace.as_deref(), force).await
        }

        Commands::Validate {
            item_name,
            all,
            changes,
            specs,
            archived,
            item_type,
            strict,
            json,
            concurrency,
            no_interactive,
            store,
        } => {
            validate::validate_command(
                item_name.as_deref(),
                validate::ValidateOptions {
                    all,
                    changes,
                    specs,
                    archived,
                    item_type,
                    strict,
                    json,
                    concurrency,
                    no_interactive,
                    store,
                },
            )
            .await
        }

        Commands::Show {
            item_name,
            json,
            item_type,
            no_interactive,
            deltas_only,
            requirements_only,
            requirements,
            no_scenarios,
            requirement,
            store,
        } => {
            show::show_command(
                item_name.as_deref(),
                show::ShowOptions {
                    json,
                    item_type,
                    no_interactive,
                    deltas_only,
                    requirements_only,
                    requirements,
                    no_scenarios,
                    requirement,
                    store,
                },
            )
            .await
        }

        Commands::Feedback { message, body } => {
            feedback::feedback_command(&message, body.as_deref()).await
        }

        Commands::Completion { command } => match command {
            CompletionCommands::Generate { shell } => {
                completion::completion_generate(shell.as_deref()).await
            }
            CompletionCommands::Install { shell, verbose } => {
                completion::completion_install(shell.as_deref(), verbose).await
            }
            CompletionCommands::Uninstall { shell, yes } => {
                completion::completion_uninstall(shell.as_deref(), yes).await
            }
        },

        Commands::Status {
            change,
            schema,
            json,
            store,
        } => {
            workflow::status_command(workflow::StatusOptions {
                change,
                schema,
                store,
                json,
            })
            .await
        }

        Commands::Instructions {
            artifact,
            change,
            schema,
            json,
            store,
        } => {
            let options = workflow::InstructionsOptions {
                change,
                schema,
                store,
                json,
            };
            // Handle reserved sub-surface keywords
            match artifact.as_deref() {
                Some("apply") => workflow::apply_instructions_command(options).await,
                Some("archive") => workflow::archive_instructions_command(options).await,
                _ => workflow::instructions_command(artifact.as_deref(), options).await,
            }
        }

        Commands::Templates { schema, json } => {
            workflow::templates_command(workflow::TemplatesOptions { schema, json }).await
        }

        Commands::Schemas { json, store } => {
            workflow::schemas_command(workflow::SchemasOptions { store, json }).await
        }

        Commands::New { command } => match command {
            NewCommands::Change {
                name,
                description,
                goal,
                schema,
                json,
                store,
                initiative: _,
                areas: _,
            } => {
                workflow::new_change_command(
                    Some(&name),
                    workflow::NewChangeOptions {
                        description,
                        goal,
                        schema,
                        store,
                        json,
                    },
                )
                .await
            }
        },

        Commands::Workset {
            command,
            json: group_json,
        } => match command {
            Some(cmd) => match cmd {
                WorksetCommands::Create {
                    name,
                    member,
                    tool,
                    json,
                } => {
                    workset::workset_create(
                        name.as_deref(),
                        workset::WorksetCreateOptions {
                            member,
                            tool,
                            json: json || group_json,
                        },
                    )
                    .await
                }
                WorksetCommands::List { json } => workset::workset_list(json || group_json).await,
                WorksetCommands::Open { name, tool } => {
                    workset::workset_open(
                        &name,
                        workset::WorksetOpenOptions {
                            tool,
                            json: group_json,
                        },
                    )
                    .await
                }
                WorksetCommands::Remove { name, yes, json } => {
                    workset::workset_remove(
                        &name,
                        workset::WorksetRemoveOptions {
                            yes,
                            json: json || group_json,
                        },
                    )
                    .await
                }
            },
            None => {
                let subcommands = "create, list (ls), open, remove";
                let message = if group_json {
                    format!(
                        "Missing subcommand for 'speckit workset'. Workset subcommands: {subcommands}."
                    )
                } else {
                    format!(
                        "Missing subcommand for 'speckit workset'. Workset subcommands: {subcommands}."
                    )
                };
                if group_json {
                    shared_output::print_json(&serde_json::json!({
                        "status": [{
                            "severity": "error",
                            "code": "unknown_workset_subcommand",
                            "message": message,
                            "fix": "Run one of the workset subcommands.",
                        }]
                    }));
                } else {
                    eprintln!("Error: {message}");
                }
                process::exit(1);
            }
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
