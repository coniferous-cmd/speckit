//! Regenerate the parity fixture hashes in `src/templates/generation.rs`.
//!
//! Run after intentionally changing a workflow template or the shared guard:
//!
//! ```sh
//! cargo run -p speckit-core --example parity_hashes
//! ```
//!
//! Each line is `<skill-dir>  <sha256>  <same|DRIFT>`: the hash of that skill's
//! generated content rendered with `PARITY_BASELINE_VERSION` and normalised by
//! `parity_hash`, next to whether it still matches the checked-in fixture.
//! Copy every DRIFT line's hash into `parity_fixture_hashes()` in the same
//! commit as the template change; `tests/skill_parity.rs` fails until you do.

fn main() {
    use speckit_core::templates::generation::{
        PARITY_BASELINE_VERSION, generate_skill_content, get_skill_templates,
        parity_fixture_hashes, parity_hash,
    };

    let fixtures = parity_fixture_hashes();
    let mut drifted = 0;
    for entry in get_skill_templates(None) {
        let content = generate_skill_content(&entry.template, PARITY_BASELINE_VERSION, None);
        let hash = parity_hash(&content);
        let status = match fixtures.get(entry.dir_name.as_str()) {
            Some(expected) if *expected == hash => "same",
            Some(_) => {
                drifted += 1;
                "DRIFT"
            }
            None => {
                drifted += 1;
                "NO FIXTURE"
            }
        };
        println!("{}  {hash}  {status}", entry.dir_name);
    }
    println!("\n{drifted} of {} skills drifted", fixtures.len());
}
