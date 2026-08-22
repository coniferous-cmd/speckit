//! Spec Application Logic
//!
//! Applies delta specs from a change to main specs without archiving.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Spec update information.
#[derive(Debug, Clone)]
pub struct SpecUpdate {
    pub id: String,
    pub source_root: PathBuf,
    pub source: PathBuf,
    pub target_root: PathBuf,
    pub target: PathBuf,
    pub exists: bool,
}

/// Result of building an updated spec.
#[derive(Debug, Clone)]
pub struct BuildResult {
    pub rebuilt: String,
    pub counts: SpecCounts,
    pub warnings: Vec<String>,
    pub no_requirement_blocks: bool,
    pub unaccounted_content: Vec<String>,
}

/// Counts of spec operations.
#[derive(Debug, Clone, Default)]
pub struct SpecCounts {
    pub added: usize,
    pub modified: usize,
    pub removed: usize,
    pub renamed: usize,
}

/// Result of retiring a spec.
#[derive(Debug, Clone)]
pub struct RetireResult {
    pub retired: bool,
    pub resolved_path: Option<PathBuf>,
    pub displaced_path: Option<PathBuf>,
}

/// Find all delta spec files that need to be applied from a change.
pub fn find_spec_updates(change_dir: &Path, main_specs_dir: &Path) -> Result<Vec<SpecUpdate>> {
    let mut updates = Vec::new();
    let change_specs_dir = change_dir.join("specs");

    if !change_specs_dir.exists() {
        return Ok(updates);
    }

    // Discover spec files recursively
    discover_spec_files(&change_specs_dir, &mut |spec_id, spec_file| {
        let target_file = main_specs_dir
            .join(spec_id.replace('/', std::path::MAIN_SEPARATOR_STR).as_str())
            .join("spec.md");

        let exists = target_file.exists();

        updates.push(SpecUpdate {
            id: spec_id,
            source_root: change_specs_dir.clone(),
            source: spec_file,
            target_root: main_specs_dir.to_path_buf(),
            target: target_file,
            exists,
        });
    })?;

    Ok(updates)
}

/// Build an updated spec by applying delta operations.
pub fn build_updated_spec(
    update: &SpecUpdate,
    change_name: &str,
    silent: bool,
) -> Result<BuildResult> {
    let mut warnings = Vec::new();

    // Read change spec content
    let change_content = fs::read_to_string(&update.source)?;

    // Parse deltas from the change spec file
    let plan = parse_delta_spec(&change_content);

    // Load or create base target content
    let (target_content, is_new_spec) = if update.target.exists() {
        (fs::read_to_string(&update.target)?, false)
    } else {
        (build_spec_skeleton(&update.id, change_name, None), true)
    };

    // Extract requirements sections and apply deltas
    let mut requirements = parse_requirements_from_content(&target_content);

    // Apply REMOVED
    let mut removed_applied = 0;
    for name in &plan.removed {
        let key = normalize_requirement_name(name);
        if requirements.remove(&key).is_some() {
            removed_applied += 1;
        } else if !is_new_spec && !silent {
            warnings.push(format!(
                "{} - REMOVED requirement '{}' is not in the current spec; treating as already removed.",
                update.id, name
            ));
        }
    }

    // Apply MODIFIED
    let mut modified_applied = 0;
    for mod_req in &plan.modified {
        let key = normalize_requirement_name(&mod_req.name);
        if let std::collections::hash_map::Entry::Occupied(mut e) = requirements.entry(key) {
            e.insert(mod_req.raw.clone());
            modified_applied += 1;
        } else {
            return Err(anyhow::anyhow!(
                "{} MODIFIED failed for requirement '{}' - not found",
                update.id,
                mod_req.name
            ));
        }
    }

    // Apply ADDED
    let mut added_applied = 0;
    for add in &plan.added {
        let key = normalize_requirement_name(&add.name);
        if requirements.contains_key(&key) {
            return Err(anyhow::anyhow!(
                "{} ADDED failed for requirement '{}' - already exists",
                update.id,
                add.name
            ));
        }
        requirements.insert(key, add.raw.clone());
        added_applied += 1;
    }

    // Rebuild the spec
    let rebuilt = rebuild_spec_from_requirements(&update.id, &requirements, change_name);

    Ok(BuildResult {
        rebuilt,
        counts: SpecCounts {
            added: added_applied,
            modified: modified_applied,
            removed: removed_applied,
            renamed: 0,
        },
        warnings,
        no_requirement_blocks: requirements.is_empty(),
        unaccounted_content: Vec::new(),
    })
}

/// Write an updated spec to disk.
pub fn write_updated_spec(
    update: &SpecUpdate,
    rebuilt: &str,
    counts: &SpecCounts,
    silent: bool,
) -> Result<()> {
    // Create target directory if needed
    if let Some(parent) = update.target.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&update.target, rebuilt)?;

    if !silent {
        println!("Applying changes to speckit/specs/{}/spec.md:", update.id);
        if counts.added > 0 {
            println!("  + {} added", counts.added);
        }
        if counts.modified > 0 {
            println!("  ~ {} modified", counts.modified);
        }
        if counts.removed > 0 {
            println!("  - {} removed", counts.removed);
        }
        if counts.renamed > 0 {
            println!("  -> {} renamed", counts.renamed);
        }
    }

    Ok(())
}

/// Retire a capability: delete its main spec and prune empty directories.
pub fn retire_spec(
    update: &SpecUpdate,
    main_specs_dir: &Path,
    silent: bool,
) -> Result<RetireResult> {
    if !update.target.exists() {
        return Ok(RetireResult {
            retired: false,
            resolved_path: None,
            displaced_path: None,
        });
    }

    // Resolve real path before deletion
    let real_source = if update.target.is_symlink() {
        None
    } else {
        Some(dunce::canonicalize(&update.target).unwrap_or_else(|_| update.target.clone()))
    };

    // Verify the target is inside the specs directory
    if let Some(ref real) = real_source {
        let real_specs =
            dunce::canonicalize(main_specs_dir).unwrap_or_else(|_| main_specs_dir.to_path_buf());
        if !real.starts_with(&real_specs) {
            return Err(anyhow::anyhow!(
                "Could not retire capability '{}': {} resolves outside {}.",
                update.id,
                update.target.display(),
                main_specs_dir.display()
            ));
        }
    }

    fs::remove_file(&update.target)?;

    // Prune empty directories
    if let Some(parent) = update.target.parent() {
        let _ = prune_empty_dirs(parent, main_specs_dir);
    }

    let nominal = format!("speckit/specs/{}/spec.md", update.id);
    if !silent {
        println!("Retiring {}: all requirements removed.", nominal);
    }

    Ok(RetireResult {
        retired: true,
        resolved_path: real_source,
        displaced_path: None,
    })
}

/// Finalize a retired spec by removing its displaced file and pruning.
pub fn finalize_retired_spec(
    target: &Path,
    displaced_path: &Path,
    main_specs_dir: &Path,
) -> Result<()> {
    fs::remove_file(displaced_path)?;
    if let Some(parent) = target.parent() {
        let _ = prune_empty_dirs(parent, main_specs_dir);
    }
    Ok(())
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Parsed delta specification.
struct DeltaSpec {
    added: Vec<DeltaRequirement>,
    modified: Vec<DeltaRequirement>,
    removed: Vec<String>,
    renamed: Vec<(String, String)>,
}

/// A single requirement from a delta.
struct DeltaRequirement {
    name: String,
    raw: String,
}

/// Parse a delta spec file into its component operations.
fn parse_delta_spec(content: &str) -> DeltaSpec {
    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut removed = Vec::new();
    let mut renamed = Vec::new();

    let mut current_section: Option<String> = None;
    let mut current_block = String::new();
    let mut current_name = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Detect section headers
        if trimmed.eq_ignore_ascii_case("## ADDED Requirements")
            || trimmed.eq_ignore_ascii_case("## ADDED Requirement")
        {
            flush_current(
                &current_section,
                &current_name,
                &current_block,
                &mut added,
                &mut modified,
            );
            current_section = Some("ADDED".to_string());
            current_block.clear();
            current_name.clear();
            continue;
        }
        if trimmed.eq_ignore_ascii_case("## MODIFIED Requirements")
            || trimmed.eq_ignore_ascii_case("## MODIFIED Requirement")
        {
            flush_current(
                &current_section,
                &current_name,
                &current_block,
                &mut added,
                &mut modified,
            );
            current_section = Some("MODIFIED".to_string());
            current_block.clear();
            current_name.clear();
            continue;
        }
        if trimmed.eq_ignore_ascii_case("## REMOVED Requirements")
            || trimmed.eq_ignore_ascii_case("## REMOVED Requirement")
        {
            flush_current(
                &current_section,
                &current_name,
                &current_block,
                &mut added,
                &mut modified,
            );
            current_section = Some("REMOVED".to_string());
            current_block.clear();
            current_name.clear();
            continue;
        }
        if trimmed.eq_ignore_ascii_case("## RENAMED Requirements")
            || trimmed.eq_ignore_ascii_case("## RENAMED Requirement")
        {
            flush_current(
                &current_section,
                &current_name,
                &current_block,
                &mut added,
                &mut modified,
            );
            current_section = Some("RENAMED".to_string());
            current_block.clear();
            current_name.clear();
            continue;
        }

        // Parse requirement headers within sections
        if let Some(ref section) = current_section {
            if trimmed.starts_with("### Requirement:") {
                // Flush previous requirement
                flush_current(
                    &current_section,
                    &current_name,
                    &current_block,
                    &mut added,
                    &mut modified,
                );

                current_name = trimmed
                    .strip_prefix("### Requirement:")
                    .unwrap_or(trimmed)
                    .trim()
                    .to_string();
                current_block = format!("{}\n", line);
            } else if section == "REMOVED" && trimmed.starts_with("- ") {
                let name = trimmed
                    .strip_prefix("- ")
                    .unwrap_or(trimmed)
                    .trim()
                    .to_string();
                if !name.is_empty() {
                    removed.push(name);
                }
            } else if section == "RENAMED" && trimmed.starts_with("- ") {
                // Parse "old -> new" format
                let parts: Vec<&str> = trimmed
                    .strip_prefix("- ")
                    .unwrap_or(trimmed)
                    .split(" -> ")
                    .collect();
                if parts.len() == 2 {
                    renamed.push((parts[0].trim().to_string(), parts[1].trim().to_string()));
                }
            } else if !current_name.is_empty() {
                current_block.push_str(line);
                current_block.push('\n');
            }
        }
    }

    // Flush the last requirement
    flush_current(
        &current_section,
        &current_name,
        &current_block,
        &mut added,
        &mut modified,
    );

    DeltaSpec {
        added,
        modified,
        removed,
        renamed,
    }
}

fn flush_current(
    section: &Option<String>,
    name: &str,
    block: &str,
    added: &mut Vec<DeltaRequirement>,
    modified: &mut Vec<DeltaRequirement>,
) {
    if name.is_empty() || block.is_empty() {
        return;
    }
    match section.as_deref() {
        Some("ADDED") => added.push(DeltaRequirement {
            name: name.to_string(),
            raw: block.to_string(),
        }),
        Some("MODIFIED") => modified.push(DeltaRequirement {
            name: name.to_string(),
            raw: block.to_string(),
        }),
        _ => {}
    }
}

/// Normalize a requirement name for comparison.
fn normalize_requirement_name(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Parse requirements from spec content.
fn parse_requirements_from_content(content: &str) -> HashMap<String, String> {
    let mut requirements = HashMap::new();
    let mut current_name = String::new();
    let mut current_block = String::new();
    let mut in_requirements = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.eq_ignore_ascii_case("## Requirements") {
            in_requirements = true;
            continue;
        }

        if in_requirements && trimmed.starts_with("## ") && !trimmed.starts_with("### ") {
            // Hit next section
            break;
        }

        if in_requirements && trimmed.starts_with("### Requirement:") {
            if !current_name.is_empty() {
                requirements.insert(
                    normalize_requirement_name(&current_name),
                    current_block.clone(),
                );
            }
            current_name = trimmed
                .strip_prefix("### Requirement:")
                .unwrap_or(trimmed)
                .trim()
                .to_string();
            current_block = format!("{}\n", line);
        } else if !current_name.is_empty() {
            current_block.push_str(line);
            current_block.push('\n');
        }
    }

    if !current_name.is_empty() {
        requirements.insert(normalize_requirement_name(&current_name), current_block);
    }

    requirements
}

/// Rebuild a spec from its requirements.
fn rebuild_spec_from_requirements(
    spec_name: &str,
    requirements: &HashMap<String, String>,
    change_name: &str,
) -> String {
    let mut result = format!("# {} Specification\n\n", spec_name);
    result.push_str("## Purpose\n");
    result.push_str(&format!(
        "TBD - created by archiving change {}. Update Purpose after archive.\n",
        change_name
    ));
    result.push_str("\n## Requirements\n");

    let mut sorted: Vec<_> = requirements.iter().collect();
    sorted.sort_by_key(|(name, _)| name.to_string());

    for (_, raw) in sorted {
        result.push('\n');
        result.push_str(raw.trim());
        result.push('\n');
    }

    result
}

/// Build a skeleton spec for new capabilities.
pub fn build_spec_skeleton(
    spec_folder_name: &str,
    change_name: &str,
    purpose: Option<&str>,
) -> String {
    let purpose_body = purpose
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| {
            format!(
                "TBD - created by archiving change {}. Update Purpose after archive.",
                change_name
            )
        });
    format!(
        "# {} Specification\n\n## Purpose\n{}\n\n## Requirements\n",
        spec_folder_name, purpose_body
    )
}

/// Discover spec files recursively.
fn discover_spec_files(dir: &Path, callback: &mut dyn FnMut(String, PathBuf)) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    let spec_file = dir.join("spec.md");
    if spec_file.exists() {
        // Determine spec id from directory name
        let id = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        callback(id, spec_file);
    }

    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                discover_spec_files(&path, callback)?;
            }
        }
    }

    Ok(())
}

/// Remove empty directories from start_dir upward, never leaving boundary_dir.
fn prune_empty_dirs(start_dir: &Path, boundary_dir: &Path) -> Result<()> {
    let boundary = dunce::canonicalize(boundary_dir).unwrap_or_else(|_| boundary_dir.to_path_buf());

    let mut dir = start_dir.to_path_buf();
    loop {
        let real_dir = match dunce::canonicalize(&dir) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };

        if real_dir == boundary || !real_dir.starts_with(&boundary) {
            return Ok(());
        }

        match fs::read_dir(&dir) {
            Ok(entries) => {
                if entries.count() > 0 {
                    return Ok(());
                }
            }
            Err(_) => return Ok(()),
        }

        match fs::remove_dir(&dir) {
            Ok(_) => {}
            Err(_) => return Ok(()),
        }

        dir = match dir.parent() {
            Some(p) => p.to_path_buf(),
            None => return Ok(()),
        };
    }
}
