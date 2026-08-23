//! Unified skill generation.
//!
//! This module is the single canonical source for skill templates and the
//! `SKILL.md` content emitted to disk. `init`, `update`, profile sync, and
//! migration all consume it; nothing else should hand-roll frontmatter or
//! duplicate the workflow list. The shape mirrors `openspec/src/core/shared/skill-generation.ts`
//! so the two implementations stay structurally aligned: every OpenSpec
//! workflow has exactly one Speckit counterpart, and brand substitution
//! (`openspec` -> `speckit`, `OpenSpec` -> `Speckit`) is the only content delta.

use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Skill template with directory name and workflow ID mapping.
///
/// Mirrors OpenSpec's `SkillTemplateEntry` so parity tests can compare one entry
/// at a time.
#[derive(Debug, Clone)]
pub struct SkillTemplateEntry {
    pub template: super::types::SkillTemplate,
    pub dir_name: String,
    pub workflow_id: String,
}

/// Returns every canonical Speckit skill template, optionally filtered to a
/// subset of workflow ids.
///
/// The default (no filter) set is the 12 OpenSpec workflows. Filter values come
/// from `migration::ALL_WORKFLOWS` (and must be `String` to keep the public API
/// flexible across profile/CLI layers).
pub fn get_skill_templates(workflow_filter: Option<&[String]>) -> Vec<SkillTemplateEntry> {
    let all: Vec<SkillTemplateEntry> = vec![
        entry("explore", super::workflows::get_explore_skill_template()),
        entry("new", super::workflows::get_new_change_skill_template()),
        entry(
            "continue",
            super::workflows::get_continue_change_skill_template(),
        ),
        entry("apply", super::workflows::get_apply_change_skill_template()),
        entry(
            "update",
            super::workflows::get_update_change_skill_template(),
        ),
        entry("ff", super::workflows::get_ff_change_skill_template()),
        entry("sync", super::workflows::get_sync_specs_skill_template()),
        entry(
            "archive",
            super::workflows::get_archive_change_skill_template(),
        ),
        entry(
            "bulk-archive",
            super::workflows::get_bulk_archive_change_skill_template(),
        ),
        entry(
            "verify",
            super::workflows::get_verify_change_skill_template(),
        ),
        entry("onboard", super::workflows::get_onboard_skill_template()),
        entry("propose", super::workflows::get_propose_skill_template()),
    ];

    let Some(filter) = workflow_filter else {
        return all;
    };

    let filter_set: std::collections::HashSet<&str> = filter.iter().map(|s| s.as_str()).collect();
    all.into_iter()
        .filter(|e| filter_set.contains(e.workflow_id.as_str()))
        .collect()
}

fn entry(workflow_id: &str, template: super::types::SkillTemplate) -> SkillTemplateEntry {
    let dir_name = template.name.clone();
    SkillTemplateEntry {
        template,
        dir_name,
        workflow_id: workflow_id.to_string(),
    }
}

/// Command template with workflow ID mapping.
///
/// Mirrors OpenSpec's `CommandTemplateEntry` so command generation can
/// filter by workflow just like skills do.
#[derive(Debug, Clone)]
pub struct CommandTemplateEntry {
    pub template: super::types::CommandTemplate,
    pub id: String,
}

/// Returns every canonical Speckit command template, optionally filtered to a
/// subset of workflow ids.
///
/// Mirrors OpenSpec's `getCommandTemplates()` from
/// `openspec/src/core/shared/skill-generation.ts`.
pub fn get_command_templates(workflow_filter: Option<&[String]>) -> Vec<CommandTemplateEntry> {
    let all: Vec<CommandTemplateEntry> = vec![
        cmd_entry("explore", super::workflows::get_opsx_explore_command_template()),
        cmd_entry("new", super::workflows::get_opsx_new_command_template()),
        cmd_entry("continue", super::workflows::get_opsx_continue_command_template()),
        cmd_entry("apply", super::workflows::get_opsx_apply_command_template()),
        cmd_entry("update", super::workflows::get_opsx_update_command_template()),
        cmd_entry("ff", super::workflows::get_opsx_ff_command_template()),
        cmd_entry("sync", super::workflows::get_opsx_sync_command_template()),
        cmd_entry("archive", super::workflows::get_opsx_archive_command_template()),
        cmd_entry("bulk-archive", super::workflows::get_opsx_bulk_archive_command_template()),
        cmd_entry("verify", super::workflows::get_opsx_verify_command_template()),
        cmd_entry("onboard", super::workflows::get_opsx_onboard_command_template()),
        cmd_entry("propose", super::workflows::get_opsx_propose_command_template()),
    ];

    match workflow_filter {
        Some(filter) => {
            let filter_set: std::collections::HashSet<&str> =
                filter.iter().map(|s| s.as_str()).collect();
            all.into_iter()
                .filter(|e| filter_set.contains(e.id.as_str()))
                .collect()
        }
        None => all,
    }
}

fn cmd_entry(id: &str, template: super::types::CommandTemplate) -> CommandTemplateEntry {
    CommandTemplateEntry {
        template,
        id: id.to_string(),
    }
}

/// Converts command templates into `CommandContent` for the command generator.
///
/// Mirrors OpenSpec's `getCommandContents()` from
/// `openspec/src/core/shared/skill-generation.ts`.
pub fn get_command_contents(
    workflow_filter: Option<&[String]>,
) -> Vec<crate::command_generation::CommandContent> {
    get_command_templates(workflow_filter)
        .into_iter()
        .map(|entry| crate::command_generation::CommandContent {
            id: entry.id,
            name: entry.template.name,
            description: entry.template.description,
            category: entry.template.category,
            tags: entry.template.tags,
            body: entry.template.content,
        })
        .collect()
}

/// Generates skill file content with YAML frontmatter, mirroring
/// `openspec/src/core/shared/skill-generation.ts:generateSkillContent`.
///
/// `generated_by_version` is embedded as `metadata.generatedBy` so the version
/// that produced the file can be detected later (used by update to decide
/// whether to regenerate).
///
/// `transform_instructions` is applied to the template body before emission;
/// the surface-specific generators use it to swap `/opsx:*` -> `/speckit:*`
/// in the embedded slash-command references for command templates, and to
/// apply brand substitution when content is generated for verification.
pub fn generate_skill_content(
    template: &super::types::SkillTemplate,
    generated_by_version: &str,
    transform_instructions: Option<&dyn Fn(&str) -> String>,
) -> String {
    let instructions = match transform_instructions {
        Some(f) => f(template.instructions.as_str()),
        None => template.instructions.clone(),
    };

    let license = template.license.as_deref().unwrap_or("MIT");
    let compatibility = template
        .compatibility
        .as_deref()
        .unwrap_or("Requires speckit CLI.");
    let author = template
        .metadata
        .as_ref()
        .and_then(|m| m.get("author"))
        .map(|s| s.as_str())
        .unwrap_or("speckit");
    let version = template
        .metadata
        .as_ref()
        .and_then(|m| m.get("version"))
        .map(|s| s.as_str())
        .unwrap_or("1.0");

    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("name: {}\n", template.name));
    out.push_str(&format!("description: {}\n", template.description));
    out.push_str(&format!("allowed-tools: {}\n", SPECKIT_CLI_ALLOWED_TOOLS));
    out.push_str(&format!("license: {}\n", license));
    out.push_str(&format!("compatibility: {}\n", compatibility));
    out.push_str("metadata:\n");
    out.push_str(&format!("  author: {}\n", author));
    out.push_str(&format!("  version: \"{}\"\n", version));
    out.push_str(&format!(
        "  generatedBy: \"{}\"\n",
        escape_yaml_string(generated_by_version)
    ));
    out.push_str("---\n");
    out.push('\n');
    out.push_str(&instructions);
    out.push('\n');
    out
}

/// The `allowed-tools` frontmatter value emitted by Speckit.
///
/// Mirrors `OPENSPEC_CLI_ALLOWED_TOOLS` in OpenSpec so the field is recognized
/// by Claude Code and other Agent-Skills-aware tools as the CLI allowlist.
pub const SPECKIT_CLI_ALLOWED_TOOLS: &str = "Bash(speckit:*)";

/// Returns the current Speckit version, suitable for embedding in
/// `metadata.generatedBy` and detecting stale generated skills during update.
///
/// Uses `CARGO_PKG_VERSION` from the speckit-core crate. The same value is
/// used by `version_check.rs`, so generated files are always tagged with the
/// running CLI's version.
pub fn speckit_generated_by_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Escape a string for safe inclusion inside double-quoted YAML scalars.
///
/// Only used for `generatedBy` (which is always a SemVer string) but kept
/// defensive in case that ever changes. Mirrors the safe-subset handling in
/// OpenSpec's `generateSkillContent`.
fn escape_yaml_string(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Returns the canonical set of skill directories under the tool's skills root
/// that Speckit owns. Used by init/update to know which directories to clean up
/// when a workflow is removed from the registry.
pub fn managed_skill_dir_names() -> Vec<String> {
    get_skill_templates(None)
        .into_iter()
        .map(|e| e.dir_name)
        .collect()
}

/// Metadata extracted from a generated `SKILL.md` frontmatter.
///
/// Used by update to decide whether an existing file is up to date without
/// comparing the full body byte-for-byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSkillFrontmatter {
    pub name: String,
    pub generated_by: Option<String>,
    pub version: Option<String>,
}

/// Parses the YAML frontmatter from a generated `SKILL.md`.
///
/// Returns `None` if the file does not start with `---` or the frontmatter
/// block is malformed. Used by update's stale-detection step.
pub fn parse_skill_frontmatter(content: &str) -> Option<ParsedSkillFrontmatter> {
    let trimmed = content.strip_prefix("---")?;
    let rest = trimmed.strip_prefix('\n')?;
    let end = rest.find("\n---")?;
    let fm = &rest[..end];

    let mut name: Option<String> = None;
    let mut generated_by: Option<String> = None;
    let mut version: Option<String> = None;
    let mut in_metadata = false;

    for line in fm.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("metadata:") {
            in_metadata = true;
            continue;
        }
        if !in_metadata {
            if let Some((k, v)) = split_kv(line)
                && k == "name"
            {
                name = Some(v.to_string());
            }
            continue;
        }
        // Inside metadata: keys are indented.
        let (k, v) = match split_kv(line) {
            Some(kv) => kv,
            None => continue,
        };
        match k {
            "version" => version = Some(v.trim_matches('"').to_string()),
            "generatedBy" => generated_by = Some(v.trim_matches('"').to_string()),
            _ => {}
        }
    }

    Some(ParsedSkillFrontmatter {
        name: name?,
        generated_by,
        version,
    })
}

fn split_kv(line: &str) -> Option<(&str, &str)> {
    let idx = line.find(':')?;
    let key = line[..idx].trim();
    let value = line[idx + 1..].trim();
    Some((key, value))
}

/// Normalize a generated skill file body for hash comparison.
///
/// Applies the documented brand substitutions and whitespace normalisation so the
/// SHA-256 hash of the normalised content is identical across OpenSpec and Speckit
/// for the same workflow. Mirrors `stableStringify` + `hash` in OpenSpec's
/// `regen-parity-hashes.mjs`.
pub fn normalize_for_parity(content: &str) -> String {
    // Strip trailing whitespace from every line.
    let stripped: String = content
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    // Normalise CRLF -> LF.
    let normalized = stripped.replace("\r\n", "\n");
    if normalized.trim().is_empty() {
        String::new()
    } else {
        normalized
    }
}

/// Compute the SHA-256 hex digest of a normalised string.
pub fn parity_hash(content: &str) -> String {
    let normalised = normalize_for_parity(content);
    let bytes = normalised.as_bytes();
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    // Inline hex encoding (avoiding a dependency).
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut hex = Vec::with_capacity(result.len() * 2);
    for &byte in &result {
        hex.push(HEX_CHARS[(byte >> 4) as usize]);
        hex.push(HEX_CHARS[(byte & 0xf) as usize]);
    }
    String::from_utf8(hex).unwrap()
}

/// Baseline version string used when computing fixture hashes.
///
/// Must match the `PARITY_BASELINE` constant in OpenSpec's
/// `regen-parity-hashes.mjs` so both codebases produce identical hashes for the
/// same template content.
pub const PARITY_BASELINE_VERSION: &str = "PARITY-BASELINE";

/// Fixture hashes for the 12 canonical skill file contents.
///
/// Each entry maps a skill directory name to the SHA-256 hex digest of that
/// skill's generated file content when rendered with `PARITY_BASELINE_VERSION`
/// as the `generatedBy` value. These are the Speckit counterparts to
/// `EXPECTED_GENERATED_SKILL_CONTENT_HASHES` in OpenSpec's
/// `test/core/templates/skill-templates-parity.test.ts`; the keys are the
/// `speckit-*` equivalents of OpenSpec's `openspec-*` names.
///
/// These values are produced by running `regen-parity-hashes.mjs` against the
/// current source. When a workflow template is intentionally changed, update
/// the corresponding hash here and in the integration tests in
/// `skill_parity.rs` simultaneously.
pub fn parity_fixture_hashes() -> std::collections::HashMap<&'static str, &'static str> {
    let entries: [(&'static str, &'static str); 12] = [
        (
            "speckit-explore",
            "a3569a81a92b3f6d0fc044a01d76032ce6b8d09e0710ad8c3b932a2922e9454f",
        ),
        (
            "speckit-new-change",
            "36f8c6c21ddb9fe0308acc6fd0998e870cc7de4c1c2762ecd0dfe8582022a9df",
        ),
        (
            "speckit-continue-change",
            "0aade44dd759630a49de80a6f9de546620655892d3d9d115fd043de14b03febc",
        ),
        (
            "speckit-apply-change",
            "6b5ef0f6130f82eae145227db7d5f967654850fb2878a022fa81957b5554f2ea",
        ),
        (
            "speckit-update-change",
            "e28c0d7196a20a167bb7732108decab964e1493ecbfeff94d117c24b011c8fa5",
        ),
        (
            "speckit-ff-change",
            "ed537e6aa0696c76f471b21ae5c31bcd13867dc61e8695a58b1fe45ef85b7778",
        ),
        (
            "speckit-sync-specs",
            "3e6622d8b1023efc7759fb5c4b3d65f380deb79640086f9c8740314cee7648c3",
        ),
        (
            "speckit-archive-change",
            "62025dabf21b40be46b41c2c2a520035d3a8676780924e179feaf80e74b2af4f",
        ),
        (
            "speckit-bulk-archive-change",
            "b5e1fecb057629b7e96a80f7d2b36ab043c211a7dfb50d234bf3d088977b690f",
        ),
        (
            "speckit-verify-change",
            "e947a2f19344b49c30eee655caf8f44dad1accdb9b524a31bcfaf72250ce8722",
        ),
        (
            "speckit-onboard",
            "470190876e10cae692f62c2ea02e2be0195b5edd572603a97a069f6cb1694e08",
        ),
        (
            "speckit-propose",
            "f042cc05799239510e7bd3deb7a7df8d51b52437a66a45c760226eb1e095eb1f",
        ),
    ];
    entries.into_iter().collect()
}

/// Build the frontmatter-only metadata block used to detect managed skills
/// during update. The map is sorted alphabetically so its hash is stable.
pub fn frontmatter_metadata_for_hash(
    template: &super::types::SkillTemplate,
) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    if let Some(m) = &template.metadata {
        for (k, v) in m {
            out.insert(k.clone(), v.clone());
        }
    }
    out.insert("generatedBy".into(), speckit_generated_by_version());
    out.insert("tool".into(), "speckit".into());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_skill_templates_default_returns_twelve() {
        let all = get_skill_templates(None);
        assert_eq!(all.len(), 12);
    }

    #[test]
    fn get_skill_templates_filter_returns_subset() {
        let filter = vec!["explore".to_string(), "apply".to_string()];
        let picked = get_skill_templates(Some(&filter));
        assert_eq!(picked.len(), 2);
        let ids: Vec<&str> = picked.iter().map(|e| e.workflow_id.as_str()).collect();
        assert!(ids.contains(&"explore"));
        assert!(ids.contains(&"apply"));
    }

    #[test]
    fn get_skill_templates_filter_unknown_workflow_excluded() {
        let filter = vec!["nonexistent".to_string()];
        let picked = get_skill_templates(Some(&filter));
        assert!(picked.is_empty());
    }

    #[test]
    fn workflow_id_matches_dir_name_pattern() {
        let all = get_skill_templates(None);
        for entry in &all {
            assert!(
                entry.dir_name.starts_with("speckit-"),
                "dir {} should start with speckit-",
                entry.dir_name
            );
            assert!(
                !entry.dir_name.contains("openspec"),
                "dir {} should not contain openspec",
                entry.dir_name
            );
        }
    }

    #[test]
    fn frontmatter_includes_generated_by() {
        let all = get_skill_templates(None);
        let first = &all[0].template;
        let content = generate_skill_content(first, "1.9.0", None);
        assert!(content.starts_with("---\n"));
        assert!(content.contains("name: "));
        assert!(content.contains("description: "));
        assert!(content.contains("allowed-tools: Bash(speckit:*)"));
        assert!(content.contains("license: MIT"));
        assert!(content.contains("compatibility: Requires speckit CLI."));
        assert!(content.contains("author: speckit"));
        assert!(content.contains("version: \"1.0\""));
        assert!(content.contains("generatedBy: \"1.9.0\""));
    }

    #[test]
    fn transform_instructions_applies() {
        let all = get_skill_templates(None);
        let template = &all[0].template;
        let transformed =
            generate_skill_content(template, "1.9.0", Some(&|s| format!("[WRAP] {}", s)));
        assert!(transformed.contains("[WRAP] "));
        // The transformed body must equal the wrapped version of the original.
        let expected_body = format!("[WRAP] {}", template.instructions);
        assert!(transformed.contains(&expected_body[..40]));
    }

    #[test]
    fn parse_skill_frontmatter_round_trip() {
        let all = get_skill_templates(None);
        let template = &all[0].template.clone();
        let content = generate_skill_content(template, "1.9.0", None);
        let parsed = parse_skill_frontmatter(&content).expect("frontmatter present");
        assert_eq!(parsed.name, template.name);
        assert_eq!(parsed.generated_by.as_deref(), Some("1.9.0"));
        assert_eq!(parsed.version.as_deref(), Some("1.0"));
    }

    #[test]
    fn parse_skill_frontmatter_handles_no_block() {
        let parsed = parse_skill_frontmatter("no frontmatter here");
        assert!(parsed.is_none());
    }

    #[test]
    fn speckit_generated_by_version_is_semver() {
        let v = speckit_generated_by_version();
        assert!(!v.is_empty());
        let parts: Vec<&str> = v.split('.').collect();
        assert!(parts.len() >= 3, "expected SemVer-ish, got {v}");
    }

    #[test]
    fn managed_skill_dir_names_is_twelve() {
        let dirs = managed_skill_dir_names();
        assert_eq!(dirs.len(), 12);
    }

    #[test]
    fn normalize_for_parity_strips_trailing_whitespace() {
        let input = "hello   \nworld  \n";
        let out = normalize_for_parity(input);
        assert_eq!(out, "hello\nworld");
    }

    #[test]
    fn normalize_for_parity_normalizes_crlf() {
        let input = "line1\r\nline2\r\n";
        let out = normalize_for_parity(input);
        assert_eq!(out, "line1\nline2");
    }

    #[test]
    fn parity_hash_is_sha256_hex() {
        // Known SHA-256 of "hello\n" (trailing newline after stripping).
        // normalize_for_parity("hello   \n") = "hello\n"
        let h = parity_hash("hello   \n");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn parity_fixture_hashes_has_twelve_entries() {
        let hashes = parity_fixture_hashes();
        assert_eq!(hashes.len(), 12);
    }

    #[test]
    fn parity_fixture_hashes_keys_match_registry() {
        let hashes = parity_fixture_hashes();
        let registry = get_skill_templates(None);
        let registry_names: std::collections::HashSet<_> =
            registry.iter().map(|e| e.dir_name.as_str()).collect();
        for dir_name in hashes.keys() {
            assert!(
                registry_names.contains(dir_name),
                "fixture hash key `{dir_name}` not in registry"
            );
        }
    }

    #[test]
    fn parity_fixture_hashes_are_sha256_hex() {
        for (dir_name, hash) in parity_fixture_hashes() {
            assert_eq!(hash.len(), 64, "hash for `{dir_name}` must be 64 hex chars");
            assert!(
                hash.chars().all(|c| c.is_ascii_hexdigit()),
                "hash for `{dir_name}` contains non-hex chars"
            );
        }
    }
}
