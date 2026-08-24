## 1. Prepend create-time frontmatter in instructions.rs

- [x] 1.1 In `crates/speckit-commands/src/workflow/instructions.rs`, locate the `template: template.to_string(),` assignment (around line 252 inside `build_artifact_instructions`) and replace it with a stamped header:
      ```rust
      let stamped_header = format!(
          "---\ncreate-time: {}\n---\n\n",
          chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
      );
      template: format!("{stamped_header}{template}"),
      ```
- [x] 1.2 Confirm `chrono` is reachable from `speckit-commands` (it is re-exported transitively from `speckit-core`, already used by `list.rs`, `utils/date.rs`, and `archive.rs`). If the build complains, add `chrono = { workspace = true }` to `crates/speckit-commands/Cargo.toml`.

## 2. Add a unit test

- [x] 2.1 In `crates/speckit-commands/src/workflow/instructions.rs` (or a new `instructions_tests.rs` next to it), add a test that calls a small helper extracting just the frontmatter-stamping logic from `build_artifact_instructions`. The test must assert:
      - the result begins with `---\n`
      - the second line matches `^create-time: \d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\n$`
      - the third line is `---\n`
      - the original `template` body follows immediately after a blank line
- [x] 2.2 Add a test asserting the timestamp value equals `chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()` modulo a one-second slack (capture both before/after to bound drift).

## 3. Verify

- [x] 3.1 `cargo fmt --all -- --check`
- [x] 3.2 `cargo clippy --workspace --all-targets --all-features -- -A warnings -A clippy::invalid_regex`
- [x] 3.3 `cargo test --workspace --all-features`
- [x] 3.4 `cargo test -p speckit-commands` (focused run for the new test)
- [x] 3.5 Manually run `cargo run -p speckit-cli -- instructions proposal --change <existing-change> --json` against any existing change directory (e.g. one from `archive/`) and confirm the returned `template` field begins with `---\ncreate-time: ...`.