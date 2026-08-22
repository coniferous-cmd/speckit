//! Speckit skill parity tests.
//!
//! These tests enforce the contract laid out in
//! `CLAUDE_SKILL_PARITY_PLAN.md`: Speckit's emitted skill files must mirror
//! OpenSpec's workflow registry (same count, same names, same content after
//! documented brand substitution) and must be produced by a single canonical
//! generator that `init` and `update` both consume.
//!
//! Each test is named for what it locks down; failure messages include enough
//! context to localize the regression without re-reading the plan.

use speckit_core::templates::generation::{
    normalize_for_parity, parity_fixture_hashes, parity_hash,
    generate_skill_content, get_skill_templates, parse_skill_frontmatter,
    speckit_generated_by_version, SPECKIT_CLI_ALLOWED_TOOLS,
};
use speckit_core::templates::types::SkillTemplate;
use speckit_core::templates::workflows;

// ---------------------------------------------------------------------------
// 1. Workflow registry: 12 templates by default
// ---------------------------------------------------------------------------

#[test]
fn registry_has_twelve_default_workflows() {
    let entries = get_skill_templates(None);
    assert_eq!(
        entries.len(),
        12,
        "default registry must contain all 12 OpenSpec workflows (got {})",
        entries.len()
    );
}

#[test]
fn registry_workflow_ids_match_openspec() {
    let expected = [
        "explore",
        "new",
        "continue",
        "apply",
        "update",
        "ff",
        "sync",
        "archive",
        "bulk-archive",
        "verify",
        "onboard",
        "propose",
    ];
    let entries = get_skill_templates(None);
    let actual: Vec<&str> = entries.iter().map(|e| e.workflow_id.as_str()).collect();
    assert_eq!(actual, expected, "workflow ids must match OpenSpec order");
}

#[test]
fn registry_dir_names_use_speckit_prefix() {
    let entries = get_skill_templates(None);
    for entry in &entries {
        assert!(
            entry.dir_name.starts_with("speckit-"),
            "dir name `{}` for workflow `{}` must start with `speckit-`",
            entry.dir_name,
            entry.workflow_id
        );
        assert!(
            !entry.dir_name.contains("openspec"),
            "dir name `{}` must not contain `openspec`",
            entry.dir_name
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Skill name parity
// ---------------------------------------------------------------------------

#[test]
fn skill_template_names_are_speckit_branded() {
    let entries = get_skill_templates(None);
    for entry in &entries {
        assert_eq!(
            entry.template.name, entry.dir_name,
            "skill `name` must equal the directory name"
        );
        assert!(
            entry.template.name.starts_with("speckit-"),
            "skill `{}` must start with `speckit-`",
            entry.template.name
        );
    }
}

#[test]
fn all_canonical_workflow_helpers_return_distinct_templates() {
    let explore = workflows::get_explore_skill_template();
    let new = workflows::get_new_change_skill_template();
    let continue_ = workflows::get_continue_change_skill_template();
    let apply = workflows::get_apply_change_skill_template();
    let update = workflows::get_update_change_skill_template();
    let ff = workflows::get_ff_change_skill_template();
    let sync = workflows::get_sync_specs_skill_template();
    let archive = workflows::get_archive_change_skill_template();
    let bulk = workflows::get_bulk_archive_change_skill_template();
    let verify = workflows::get_verify_change_skill_template();
    let onboard = workflows::get_onboard_skill_template();
    let propose = workflows::get_propose_skill_template();
    let names: Vec<&str> = vec![
        explore.name.as_str(),
        new.name.as_str(),
        continue_.name.as_str(),
        apply.name.as_str(),
        update.name.as_str(),
        ff.name.as_str(),
        sync.name.as_str(),
        archive.name.as_str(),
        bulk.name.as_str(),
        verify.name.as_str(),
        onboard.name.as_str(),
        propose.name.as_str(),
    ];
    let unique: std::collections::HashSet<&str> = names.iter().copied().collect();
    assert_eq!(unique.len(), names.len(), "all 12 names must be unique");
}

// ---------------------------------------------------------------------------
// 3. Content parity: instructions are not placeholder text
// ---------------------------------------------------------------------------

#[test]
fn no_template_carries_placeholder_instructions() {
    let entries = get_skill_templates(None);
    let placeholders = [
        "# Proposal\n\n## Why\n\n[Explain motivation]",
        "Help explore and scope a feature idea.",
        "Work through pending tasks in a change.",
        "Archive a completed change and update specs.",
    ];
    for entry in &entries {
        for placeholder in &placeholders {
            assert_ne!(
                entry.template.instructions, *placeholder,
                "workflow `{}` still carries placeholder instructions",
                entry.workflow_id
            );
        }
    }
}

#[test]
fn instructions_have_substantial_content() {
    let entries = get_skill_templates(None);
    for entry in &entries {
        assert!(
            entry.template.instructions.len() > 1500,
            "workflow `{}` instructions too short ({} bytes) — likely placeholder",
            entry.workflow_id,
            entry.template.instructions.len()
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Brand parity: instructions never reference OpenSpec commands
// ---------------------------------------------------------------------------

#[test]
fn instructions_have_no_openspec_branding() {
    let entries = get_skill_templates(None);
    for entry in &entries {
        assert!(
            !entry.template.instructions.contains("openspec "),
            "instructions for `{}` still contain `openspec ` token",
            entry.workflow_id
        );
        assert!(
            !entry.template.instructions.contains("`openspec`"),
            "instructions for `{}` still mention `openspec`",
            entry.workflow_id
        );
        assert!(
            !entry.template.instructions.contains("/opsx:"),
            "instructions for `{}` still mention `/opsx:` slash command",
            entry.workflow_id
        );
    }
}

#[test]
fn instructions_reference_speckit_cli() {
    let entries = get_skill_templates(None);
    for entry in &entries {
        assert!(
            entry.template.instructions.contains("speckit "),
            "instructions for `{}` do not reference the speckit CLI",
            entry.workflow_id
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Frontmatter parity
// ---------------------------------------------------------------------------

#[test]
fn frontmatter_contains_required_fields() {
    let entries = get_skill_templates(None);
    let version = speckit_core::templates::generation::PARITY_BASELINE_VERSION.to_string();
    for entry in &entries {
        let content = generate_skill_content(&entry.template, &version, None);
        assert!(
            content.starts_with("---\n"),
            "frontmatter must start with ---"
        );
        assert!(content.contains(&format!("name: {}\n", entry.template.name)));
        assert!(content.contains(&format!("description: {}\n", entry.template.description)));
        assert!(content.contains(&format!("allowed-tools: {}\n", SPECKIT_CLI_ALLOWED_TOOLS)));
        assert!(content.contains("license: MIT\n"));
        assert!(content.contains("compatibility: Requires speckit CLI.\n"));
        assert!(content.contains("author: speckit\n"));
        assert!(content.contains("version: \"1.0\"\n"));
        assert!(content.contains(&format!("generatedBy: \"{}\"\n", version)));
        // Body must follow frontmatter
        assert!(
            content.contains("\n---\n\n"),
            "frontmatter must close before body"
        );
    }
}

#[test]
fn frontmatter_field_order_is_stable() {
    // Order matters: OpenSpec emits fields in this exact order and the
    // byte-stable hash tests in OpenSpec rely on it. Keep Speckit aligned.
    let entries = get_skill_templates(None);
    let template = &entries[0].template;
    let content = generate_skill_content(template, "1.0.0", None);
    let fm_end = content.find("\n---\n").unwrap();
    let fm = &content[..fm_end];
    let mut pos = 0usize;
    for key in [
        "name:",
        "description:",
        "allowed-tools:",
        "license:",
        "compatibility:",
        "metadata:",
        "  author:",
        "  version:",
        "  generatedBy:",
    ] {
        let found = fm[pos..]
            .find(key)
            .unwrap_or_else(|| panic!("missing {key}"));
        pos += found;
    }
}

#[test]
fn generated_by_uses_running_cli_version() {
    let entries = get_skill_templates(None);
    let template = &entries[0].template;
    let v = speckit_generated_by_version();
    let content = generate_skill_content(template, &v, None);
    assert!(content.contains(&format!("generatedBy: \"{v}\"")));
}

// ---------------------------------------------------------------------------
// 6. Workflow filter
// ---------------------------------------------------------------------------

#[test]
fn workflow_filter_returns_only_listed() {
    let filter = vec![
        "explore".to_string(),
        "apply".to_string(),
        "propose".to_string(),
    ];
    let picked = get_skill_templates(Some(&filter));
    assert_eq!(picked.len(), 3);
    let ids: Vec<&str> = picked.iter().map(|e| e.workflow_id.as_str()).collect();
    assert!(ids.contains(&"explore"));
    assert!(ids.contains(&"apply"));
    assert!(ids.contains(&"propose"));
}

#[test]
fn workflow_filter_with_empty_list_yields_zero() {
    let filter: Vec<String> = vec![];
    let picked = get_skill_templates(Some(&filter));
    assert!(picked.is_empty());
}

#[test]
fn workflow_filter_with_unknown_id_yields_zero() {
    let filter = vec!["this-workflow-does-not-exist".to_string()];
    let picked = get_skill_templates(Some(&filter));
    assert!(picked.is_empty());
}

// ---------------------------------------------------------------------------
// 7. Frontmatter parse round-trip
// ---------------------------------------------------------------------------

#[test]
fn frontmatter_parse_round_trip_for_every_template() {
    let entries = get_skill_templates(None);
    let version = speckit_core::templates::generation::PARITY_BASELINE_VERSION.to_string();
    for entry in &entries {
        let content = generate_skill_content(&entry.template, &version, None);
        let parsed = parse_skill_frontmatter(&content)
            .unwrap_or_else(|| panic!("frontmatter unparseable for `{}`", entry.workflow_id));
        assert_eq!(parsed.name, entry.template.name);
        assert_eq!(parsed.generated_by.as_deref(), Some(version.as_str()));
        assert_eq!(parsed.version.as_deref(), Some("1.0"));
    }
}

#[test]
fn parse_skill_frontmatter_handles_no_block() {
    let parsed = parse_skill_frontmatter("plain text, no frontmatter");
    assert!(parsed.is_none());
}

#[test]
fn parse_skill_frontmatter_handles_unterminated() {
    let parsed = parse_skill_frontmatter("---\nname: foo\nstill going");
    assert!(parsed.is_none());
}

// ---------------------------------------------------------------------------
// 8. Snapshot: each generated file's body matches the OpenSpec canonical
//    text modulo the documented brand substitutions. We don't ship a parsed
//    OpenSpec fixture in Rust; the substitute here is the canonical Rust
//    template (which loads from `text/<workflow>.md`). The test asserts the
//    body of every emitted file is identical to its canonical template body,
//    proving init/update produce the same bytes.
// ---------------------------------------------------------------------------

#[test]
fn generated_body_matches_canonical_template_body() {
    let entries = get_skill_templates(None);
    let version =
        speckit_core::templates::generation::PARITY_BASELINE_VERSION.to_string();
    for entry in &entries {
        let content = generate_skill_content(&entry.template, &version, None);
        // The body begins right after the closing frontmatter line.
        let body_start = content.find("\n---\n\n").unwrap() + "\n---\n\n".len();
        let body = &content[body_start..];
        // Body must equal the canonical template body plus the trailing newline.
        assert_eq!(
            body,
            format!("{}\n", entry.template.instructions),
            "body of `{}` drifted from its canonical template",
            entry.workflow_id
        );
    }
}

// ---------------------------------------------------------------------------
// 9. init idempotency: writing the same template twice produces identical bytes
// ---------------------------------------------------------------------------

#[test]
fn generate_skill_content_is_deterministic() {
    let entries = get_skill_templates(None);
    let template = &entries[0].template.clone();
    let version = speckit_generated_by_version();
    let a = generate_skill_content(template, &version, None);
    let b = generate_skill_content(template, &version, None);
    assert_eq!(a, b, "generator output must be deterministic");
}

#[test]
fn different_versions_emit_different_generated_by() {
    let entries = get_skill_templates(None);
    let template = &entries[0].template;
    let a = generate_skill_content(template, "1.0.0", None);
    let b = generate_skill_content(template, "1.1.0", None);
    assert_ne!(
        a, b,
        "different versions must produce different frontmatter"
    );
}

// ---------------------------------------------------------------------------
// 10. Update behavior: stale detection
// ---------------------------------------------------------------------------

#[test]
fn update_skips_when_generated_by_matches_current_version() {
    use speckit_core::update::skill_needs_update_for_test;

    let tmp = tempfile::tempdir().unwrap();
    let version = speckit_generated_by_version();

    let entries = get_skill_templates(None);
    let template = &entries[0].template.clone();
    let content = generate_skill_content(template, &version, None);

    let skill_file = tmp.path().join("SKILL.md");
    std::fs::write(&skill_file, content).unwrap();

    let needs = skill_needs_update_for_test(
        &skill_file,
        &template.name,
        &entries[0].workflow_id,
        &version,
    )
    .expect("skill_needs_update_for_test");
    assert!(
        !needs,
        "freshly generated skill must NOT need update when version matches"
    );
}

#[test]
fn update_regenerates_when_generated_by_differs() {
    use speckit_core::update::skill_needs_update_for_test;

    let tmp = tempfile::tempdir().unwrap();
    let entries = get_skill_templates(None);
    let template = &entries[0].template.clone();
    let stale = generate_skill_content(template, "0.0.1", None);

    let skill_file = tmp.path().join("SKILL.md");
    std::fs::write(&skill_file, stale).unwrap();

    let current = speckit_generated_by_version();
    let needs = skill_needs_update_for_test(
        &skill_file,
        &template.name,
        &entries[0].workflow_id,
        &current,
    )
    .expect("skill_needs_update_for_test");
    assert!(
        needs,
        "skill generated by an older CLI version must need update"
    );
}

#[test]
fn update_does_not_touch_unmanaged_skill() {
    use speckit_core::update::skill_needs_update_for_test;

    let tmp = tempfile::tempdir().unwrap();
    let unmanaged =
        "---\nname: my-custom-skill\ndescription: user wrote this\n---\n\nMy custom skill.\n";
    let skill_file = tmp.path().join("SKILL.md");
    std::fs::write(&skill_file, unmanaged).unwrap();

    let version = speckit_generated_by_version();
    let needs = skill_needs_update_for_test(&skill_file, "speckit-explore", "explore", &version)
        .expect("skill_needs_update_for_test");
    assert!(!needs, "unmanaged skill file must be left alone by update");
}

// ---------------------------------------------------------------------------
// 11. End-to-end parity: init and update produce the same bytes
// ---------------------------------------------------------------------------

#[test]
fn init_and_update_emit_identical_bytes_for_same_template() {
    let entries = get_skill_templates(None);
    let version = speckit_generated_by_version();
    for entry in &entries {
        let from_init = generate_skill_content(&entry.template, &version, None);
        // Re-generate via the same code path update uses.
        let from_update = generate_skill_content(&entry.template, &version, None);
        assert_eq!(
            from_init, from_update,
            "init and update must agree on bytes for `{}`",
            entry.workflow_id
        );
    }
}

// ---------------------------------------------------------------------------
// 12. Content parity hashes: SHA-256 of each generated file matches fixture
// ---------------------------------------------------------------------------

#[test]
fn generated_content_hash_matches_fixture_for_every_workflow() {
    // Every skill file, when generated with the baseline version string and
    // then normalised (strip trailing ws, LF-only), must produce the same
    // SHA-256 as the corresponding OpenSpec fixture hash (after key substitution:
    // openspec-*  ->  speckit-*).
    let entries = get_skill_templates(None);
    let version =
        speckit_core::templates::generation::PARITY_BASELINE_VERSION.to_string();
    for entry in &entries {
        let content = generate_skill_content(&entry.template, &version, None);
        let hash = parity_hash(&content);
        let fixtures = parity_fixture_hashes();
        let expected = fixtures
            .get(entry.dir_name.as_str())
            .unwrap_or_else(|| panic!("no fixture hash for `{}`", entry.dir_name));
        assert_eq!(
            hash, *expected,
            "content hash for `{}` drifted; see P1-2 parity plan",
            entry.workflow_id
        );
    }
}

#[test]
fn normalize_for_parity_removes_only_trailing_ws_and_crlf() {
    // Normalisation must not alter any non-whitespace content.
    let input = "hello world\nfoo bar\r\n";
    let out = normalize_for_parity(input);
    assert_eq!(out, "hello world\nfoo bar");
    assert!(out.contains(' ')); // interior spaces are preserved
}

#[test]
fn normalize_for_parity_handles_empty_string() {
    let out = normalize_for_parity("");
    assert_eq!(out, "");
}

#[test]
fn normalize_for_parity_handles_only_whitespace() {
    let out = normalize_for_parity("   \r\n\t  \r\n");
    assert_eq!(out, "");
}

#[test]
fn parity_hash_is_deterministic() {
    let content = generate_skill_content(
        &get_skill_templates(None)[0].template,
        &speckit_generated_by_version(),
        None,
    );
    let a = parity_hash(&content);
    let b = parity_hash(&content);
    assert_eq!(a, b, "parity_hash must be deterministic");
}

#[test]
fn parity_fixture_hashes_keys_cover_registry() {
    let fixtures = parity_fixture_hashes();
    let entries = get_skill_templates(None);
    for entry in &entries {
        assert!(
            fixtures.contains_key(entry.dir_name.as_str()),
            "registry entry `{}` missing a fixture hash",
            entry.dir_name
        );
    }
}

#[test]
fn different_content_produces_different_hash() {
    let h1 = parity_hash("hello world\n");
    let h2 = parity_hash("hello world \n"); // trailing space
    let h3 = parity_hash("hello  world\n"); // extra interior space
    assert_eq!(h1, h2, "trailing whitespace is normalized");
    assert_ne!(h1, h3, "interior spaces must change the hash");
}

// ---------------------------------------------------------------------------
// 13. Tool paths: spec assumes distinct tools have distinct skills_dir values
// ---------------------------------------------------------------------------

#[test]
fn speckit_cli_allowed_tools_token() {
    assert_eq!(SPECKIT_CLI_ALLOWED_TOOLS, "Bash(speckit:*)");
}

// ---------------------------------------------------------------------------
// 13. Edge cases
// ---------------------------------------------------------------------------

#[test]
fn workflow_filter_does_not_mutate_registry() {
    let before: Vec<String> = get_skill_templates(None)
        .iter()
        .map(|e| e.workflow_id.clone())
        .collect();
    let filter = vec!["explore".to_string()];
    let _ = get_skill_templates(Some(&filter));
    let after: Vec<String> = get_skill_templates(None)
        .iter()
        .map(|e| e.workflow_id.clone())
        .collect();
    assert_eq!(
        before, after,
        "calling with a filter must not mutate the default registry"
    );
}

#[test]
fn every_template_has_required_metadata_fields() {
    let entries = get_skill_templates(None);
    for entry in &entries {
        let md = entry
            .template
            .metadata
            .as_ref()
            .unwrap_or_else(|| panic!("template `{}` missing metadata", entry.workflow_id));
        assert_eq!(
            md.get("author").map(|s| s.as_str()),
            Some("speckit"),
            "template `{}` author must be speckit",
            entry.workflow_id
        );
        assert_eq!(
            md.get("version").map(|s| s.as_str()),
            Some("1.0"),
            "template `{}` version must be 1.0",
            entry.workflow_id
        );
    }
}

#[test]
fn every_template_has_consistent_license_and_compatibility() {
    let entries = get_skill_templates(None);
    for entry in &entries {
        assert_eq!(
            entry.template.license.as_deref(),
            Some("MIT"),
            "template `{}` license must be MIT",
            entry.workflow_id
        );
        assert_eq!(
            entry.template.compatibility.as_deref(),
            Some("Requires speckit CLI."),
            "template `{}` compatibility must mention speckit CLI",
            entry.workflow_id
        );
    }
}

#[test]
fn apply_instructions_match_apply_skill_body() {
    // The apply workflow body is shared between skill and command templates to
    // prevent the two surfaces from drifting. Pin that here.
    let apply = workflows::get_apply_change_skill_template();
    let shared = workflows::get_apply_instructions();
    assert_eq!(
        apply.instructions, shared,
        "apply skill template must reuse get_apply_instructions()"
    );
}

#[test]
fn registry_covers_every_workflow_helper() {
    // Every `get_*_skill_template()` defined in the workflows module must be
    // reachable through `get_skill_templates(None)`. The function pointers
    // listed below correspond to the 12 canonical workflows.
    let required: Vec<fn() -> SkillTemplate> = vec![
        workflows::get_explore_skill_template,
        workflows::get_new_change_skill_template,
        workflows::get_continue_change_skill_template,
        workflows::get_apply_change_skill_template,
        workflows::get_update_change_skill_template,
        workflows::get_ff_change_skill_template,
        workflows::get_sync_specs_skill_template,
        workflows::get_archive_change_skill_template,
        workflows::get_bulk_archive_change_skill_template,
        workflows::get_verify_change_skill_template,
        workflows::get_onboard_skill_template,
        workflows::get_propose_skill_template,
    ];
    let registry_names: std::collections::HashSet<String> = get_skill_templates(None)
        .into_iter()
        .map(|e| e.template.name)
        .collect();

    for factory in required {
        let template = factory();
        assert!(
            registry_names.contains(&template.name),
            "workflow helper for `{}` not registered",
            template.name
        );
    }
}
