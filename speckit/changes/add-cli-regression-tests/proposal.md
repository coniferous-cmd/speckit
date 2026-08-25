---
create-time: 2026-08-25 21:05:01
---

## Why

The CLI's primary workflows are exercised mostly by unit tests. The existing
cross-layer suite covers root discovery and workset persistence, but does not
protect the user-facing contracts for creating, inspecting, and archiving a
change. Regressions in argument parsing, filesystem layout, error handling, or
JSON output can therefore ship even while the affected core functions pass in
isolation.

## What Changes

- Add hermetic CLI integration tests for the change lifecycle: creating a
  change, reading its JSON status, and retrieving artifact instructions.
- Add CLI regression scenarios for invalid or missing change names and for
  nested-directory root resolution.
- Lock down `skip_specs` handling so status output consistently represents a
  test-only or tooling-only change as having its specification artifact skipped.
- Add archive lifecycle coverage for successful archival and the safety rule
  that validation or spec-application failures leave the active change intact.
- Extend generated-skill regression coverage to protect init/update parity and
  preservation of unmanaged user files.

## Capabilities

### New Capabilities

None. This change adds test coverage only and does not introduce user-visible
product behavior.

### Modified Capabilities

None.

## Impact

- Adds integration tests under `crates/speckit-cli/tests/` and may add focused
  test helpers shared by those tests.
- May extend existing parity test fixtures in `crates/speckit-core/tests/`.
- Does not change CLI commands, persisted formats, dependencies, or release
  behavior.
