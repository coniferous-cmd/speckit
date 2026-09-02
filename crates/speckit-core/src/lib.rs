//! # speckit-core
//!
//! Core library for Speckit - AI-native spec-driven development.
//!
//! This crate provides the foundational types and operations for managing
//! Speckit stores, roots, and shared tool integrations.
//!
//! ## Modules
//!
//! - [`file_state`] — File locking, atomic writes, and path checks.
//! - [`speckit_root`] — Speckit root directory inspection and creation.
//! - [`root_selection`] — Root resolution for commands.
//! - [`store`] — Store registry, metadata, git backend, and CRUD operations.
//! - [`shared`] — Allowed tools, tool detection, skill paths and generation.

pub mod archive;
pub mod artifact_graph;
pub mod available_tools;
pub mod change_metadata;
pub mod change_status_policy;
pub mod command_generation;
pub mod command_surface;
pub mod completions;
pub mod config;
pub mod config_prompts;
pub mod config_schema;
pub mod file_state;
pub mod github_copilot;
pub mod global_config;
pub mod id;
pub mod init;
pub mod legacy_cleanup;
pub mod list;
pub mod migration;
pub mod onboarding_commands;
pub mod openers;
pub mod parsers;
pub mod planning_home;
pub mod profile_sync_drift;
pub mod profiles;
pub mod project_config;
pub mod references;
pub mod relationship_health;
pub mod root_selection;
pub mod schemas;
pub mod shared;
pub mod shared_skill_target;
pub mod speckit_root;
pub mod specs_apply;
pub mod store;
pub mod styles;
pub mod templates;
pub mod ui;
pub mod update;
pub mod utils;
pub mod validation;
pub mod version_check;
pub mod view;
pub mod working_set;
pub mod worksets;
