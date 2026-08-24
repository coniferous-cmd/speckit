## Context

`build_artifact_instructions()` in `crates/speckit-commands/src/workflow/instructions.rs` produces the `template` field of the returned JSON by reading a template file from the schema's `templates/` directory and assigning it to `ArtifactInstructions.template` at line 252. Today that string is the raw template body with no metadata header.

The four workflow artifacts (`proposal`, `specs`, `design`, `tasks`) all flow through this function, so a single change at this site covers every artifact.

## Goals / Non-Goals

**Goals:**

- Stamp `create-time` (local, `YYYY-MM-DD HH:MM:SS`, no timezone suffix) at the moment instructions are emitted.
- Apply the stamp uniformly to all four workflow artifacts via the existing `build_artifact_instructions()` path.
- Touch one file, add ~5 lines.

**Non-Goals:**

- A new `speckit stamp` command.
- A new module for frontmatter parsing.
- Backfilling `create-time` for artifacts written before this change.
- Updating `SKILL.md`, `.speckit.yaml`, `speckit/config.yaml`, or per-tool command adapters.
- Updating `speckit validate` or `speckit status` to parse or enforce frontmatter.
- Adding `update-time`, `author`, or any other frontmatter keys.

## Decisions

### Decision 1: Stamp at instruction time inside `build_artifact_instructions()`

**Rationale:** This is the single chokepoint where the template string exists for every workflow artifact. Prepending there gives uniform coverage with one edit.

**Alternatives considered:**

- *Stamp when file is detected by `speckit status`* — rejected: would require new frontmatter-parsing code, a mutation-on-read side effect, and is less precise ("first observed" can drift from "first written").
- *Stamp via a separate `speckit stamp` command* — rejected: adds a command surface and a manual step; user said "create-time doesn't need to be very accurate", so the simpler one-shot injection is sufficient.
- *Embed a placeholder in the markdown templates and let the AI fill it in* — rejected: AI tools do not generally substitute timestamps; this would be unreliable.

### Decision 2: Single-key frontmatter with exactly `create-time`

**Rationale:** User requirement. Keeps the diff minimal and avoids scope creep into "what should be in the frontmatter".

**Alternatives considered:**

- *Mirror SKILL.md style with nested `metadata.create-time`* — rejected: user specified a top-level field, and nesting adds characters without adding information.

### Decision 3: Use `chrono::Local::now().format("%Y-%m-%d %H:%M:%S")`

**Rationale:** `chrono` is already a dependency of `speckit-core` (Cargo.toml line 21), `chrono::Local` is already used in `speckit-core/src/list.rs`, `utils/date.rs`, and `archive.rs`. No new dependency, no new import to negotiate.

**Alternatives considered:**

- *`std::time::SystemTime` formatted manually* — would require a hand-rolled formatter and re-implementing local-timezone logic.
- *`time` crate* — not a current dependency; would add one.

### Decision 4: Prepend directly to the `template` string at line 252

**Rationale:** Localized change, no new helper function needed. The variable `template` already holds the body; we prepend the header right before assignment.

```rust
// before line 252:
template: template.to_string(),
```

becomes:

```rust
let stamped_header = format!(
    "---\ncreate-time: {}\n---\n\n",
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
);
template: format!("{stamped_header}{template}"),
```

The format string is fixed-width and locale-independent (no `%c` or `%x`), so output is deterministic across machines modulo the wall-clock and timezone.

## Risks / Trade-offs

- **AI tool drops frontmatter**: if the AI tool chosen by the user strips YAML frontmatter when copying the template into a file, the artifact will not carry `create-time`. Per user direction ("doesn't need to be very accurate"), this is acceptable. No attempt is made to detect or recover from this case.
- **Timestamp skew**: `create-time` reflects when instructions were emitted, not when the file was written. Empirically the gap is seconds to minutes — well within the user's stated tolerance.
- **No backfill**: pre-existing artifacts written before this change have no frontmatter. They remain valid markdown; they simply lack `create-time`. No migration.
- **Local time without zone**: a single string cannot be unambiguously converted back to an instant without knowing the originating zone. Documented as a tradeoff; the user accepted this.