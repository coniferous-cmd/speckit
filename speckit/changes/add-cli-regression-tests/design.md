---
create-time: 2026-08-25 21:05:21
---

## Context

`crates/speckit-cli/tests/cross_implementation.rs` already runs the compiled
binary in a temporary project and isolates configuration and data homes. Its
three scenarios establish the test harness pattern, but cover only `context`
and `workset`. Core command behavior is otherwise tested mainly inside
`speckit-core`, which cannot verify the binary's argument parsing, output
serialization, or command-to-core wiring.

This is a test-only change. `skip_specs: true` declares that no product
requirements are added or modified.

## Goals / Non-Goals

**Goals:**

- Exercise representative CLI workflows through the built executable using
  isolated filesystem and environment fixtures.
- Assert stable, machine-readable JSON shape and essential filesystem effects
  at workflow boundaries.
- Lock down safety behavior: failed archive operations must not relocate or
  partially apply a change.
- Keep generated-skill parity tests focused on observable file ownership and
  byte-level output contracts.

**Non-Goals:**

- Exhaustively test every CLI subcommand or duplicate core unit tests.
- Change command semantics, JSON schemas, or error messages beyond assertions
  needed for stable public contracts.
- Add network, global-home, or interactive-prompt dependencies to CI tests.

## Decisions

### Extend the existing black-box CLI integration suite

Add focused scenarios to `crates/speckit-cli/tests/cross_implementation.rs`
or a sibling integration-test module. Reuse its `CARGO_BIN_EXE_speckit` helper
and temporary XDG/HOME isolation so tests exercise the released command surface
without relying on a developer's real configuration.

Alternative considered: command-module tests. They are faster and can inspect
internal types, but would miss CLI parsing, process environment behavior, and
stdout purity—the regression boundaries this change targets.

### Assert contracts, not incidental presentation

For successful JSON commands, parse stdout as exactly one JSON document and
assert required fields, status codes, and persisted artifacts. For failures,
assert non-success exit status, structured JSON/error diagnostics where the
command promises them, and that protected paths remain unchanged. Avoid exact
assertions on timestamps, absolute temporary paths, ANSI text, or unrelated
diagnostic ordering.

### Cover the lifecycle at critical safety boundaries

The workflow scenarios cover:

1. `new change`, followed by `status --json` and `instructions --json`.
2. Invocation from a nested directory and invalid/missing change identifiers.
3. A `skip_specs: true` change whose status reports the specs artifact as
   skipped, matching validation and artifact-graph behavior.
4. Successful archive plus archive validation/spec-application failure atomicity.
5. `init`/`update` skill output parity and non-overwrite behavior for unmanaged
   files.

This gives high regression value across creation, discovery, generation, and
destructive workflow boundaries while keeping fixtures maintainable.

## Risks / Trade-offs

- Process-level tests cost more than unit tests. The suite will use only a few
  compact fixtures and no external services to keep runtime predictable.
- JSON assertions can become brittle if they cover implementation details.
  Each case should assert only documented or relied-on contract fields.
- Current `status` implementation derives artifacts from filesystem presence
  alone, so the new `skip_specs` assertion will initially expose a mismatch.
  The implementation must route or augment status calculation with the same
  metadata-aware artifact graph used by instructions rather than weaken the
  regression assertion.
- Archive failures have multiple causes. Separate fixtures should isolate an
  incomplete-task failure from a spec-application failure so an unrelated
  validation rule cannot mask the safety guarantee.
