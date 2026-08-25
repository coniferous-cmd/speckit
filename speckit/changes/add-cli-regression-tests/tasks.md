---
create-time: 2026-08-25 21:05:21
---

## 1. CLI lifecycle regression coverage

- [x] 1.1 Extend the hermetic CLI test fixture with helpers for creating
  changes and asserting a single JSON stdout document.
- [x] 1.2 Add an end-to-end `new change` → `status --json` →
  `instructions --json` scenario that verifies scaffolded artifacts and
  required output fields.
- [x] 1.3 Add nested-directory and invalid/missing change scenarios that
  verify root selection and failure contracts without reading user config.
- [x] 1.4 Add a `skip_specs: true` status regression; update status artifact
  calculation as needed so specs is reported as skipped rather than ready.

## 2. Archive safety regression coverage

- [x] 2.1 Add a successful archive scenario that verifies the active change is
  moved to the archive destination and reports the expected JSON result.
- [x] 2.2 Add an incomplete-task archive rejection scenario that verifies the
  active change remains in place.
- [x] 2.3 Add a spec-application failure scenario that verifies no change
  directory is moved and main specs are not partially modified.

## 3. Generated skill ownership and parity coverage

- [x] 3.1 Add regression assertions that `init` and `update` produce identical
  managed skill bytes for the same workflow/version inputs.
- [x] 3.2 Add a scenario ensuring update leaves an unmanaged user skill file
  unchanged while refreshing managed files when appropriate.

## 4. Verification

- [x] 4.1 Run the focused CLI and skill-parity integration tests.
- [x] 4.2 Run `cargo test --workspace` and record any pre-existing warnings
  separately from test failures.
