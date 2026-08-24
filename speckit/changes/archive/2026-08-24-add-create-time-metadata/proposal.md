## Why

Currently, generated workflow artifacts (`proposal.md`, `design.md`, `tasks.md`, `specs/**/*.md`) are written by the AI tool as plain markdown with no frontmatter. There is no machine-readable record of when an artifact was produced. Users cannot tell, from the file alone, whether an artifact is fresh or stale, or sort artifacts by creation time.

This change adds a `create-time` field to the YAML frontmatter of every workflow artifact, stamped by the CLI at the moment it returns the template to the AI tool.

## What Changes

- `build_artifact_instructions()` in `crates/speckit-commands/src/workflow/instructions.rs` prepends a YAML frontmatter block to the `template` string before returning it.
- The prepended block contains a single top-level field `create-time`, formatted as `YYYY-MM-DD HH:MM:SS` in local time (no timezone suffix).
- The timestamp is computed via `chrono::Local::now().format("%Y-%m-%d %H:%M:%S")` at call time. `chrono` is already a dependency of `speckit-core`.
- All four artifacts (`proposal`, `specs`, `design`, `tasks`) receive the same frontmatter treatment uniformly.
- No new modules, no new commands, no new schema fields, no frontmatter parsing.

## Capabilities

### New Capabilities

- `artifact-metadata`: every workflow artifact (`proposal.md`, `design.md`, `tasks.md`, and files under `specs/`) carries a top-level YAML `create-time` field stamped at the CLI's instruction-emission time in `YYYY-MM-DD HH:MM:SS` local-time format. The capability-path `artifact-metadata` is reserved for any future per-capability specs under `speckit/specs/`; this change ships only a delta spec at `speckit/changes/add-create-time-metadata/specs/spec.md`.

### Modified Capabilities

None.

## Impact

- **Code surface**: one file modified (`crates/speckit-commands/src/workflow/instructions.rs`), approximately 5 lines added.
- **Build**: no new dependencies; `chrono` already in `speckit-core/Cargo.toml`.
- **AI tools**: they will see a YAML frontmatter block at the top of each template. The block contains exactly one key. Most AI tools preserve frontmatter when copying the template into a file; if any tool strips it, that artifact simply lacks `create-time`, which is acceptable per the "good-enough precision" requirement.
- **Backwards compatibility**: existing artifacts written before this change have no frontmatter and remain valid markdown. New artifacts gain frontmatter; consumers that don't understand frontmatter (markdown renderers) ignore it.
- **Validation**: `speckit validate` and `speckit status` do not currently parse artifact frontmatter, so neither needs updating.