//! Shared project-root guard for every workflow skill.
//!
//! Generated skills and commands are installed once per machine, so they are
//! offered in every repository the agent opens - including repositories that
//! never ran `speckit init`. Nothing stops the workflow there: `speckit new
//! change` falls back to an implicit root and creates `speckit/` in whatever
//! directory the agent happens to be in.
//!
//! The guard is a single source of truth interpolated into every workflow text
//! at load time, so no skill can silently lose it. `templates/workflows/text/
//! *.md` holds `{{PROJECT_ROOT_GUARD}}` where the paragraph belongs;
//! [`with_project_root_guard`] performs the substitution, and
//! `every_workflow_carries_the_project_root_guard` fails the build if a text
//! stops carrying the marker or a marker is left unreplaced.
//!
//! What follows the check depends on how the workflow was reached, because the
//! two cases want opposite things. A skill the model picked on its own in an
//! unrelated repository must get out of the way: the user asked for help, not
//! for Speckit, and answering with a setup menu is the bug. A user who named
//! Speckit, named the skill, or invoked it by name is owed an answer about
//! Speckit, so that case stops and asks.
//!
//! Invoking a skill by name is an explicit request, so it falls in the second
//! branch.

/// Token each workflow text uses where the guard paragraph belongs.
pub const PROJECT_ROOT_GUARD_PLACEHOLDER: &str = "{{PROJECT_ROOT_GUARD}}";

/// The guard paragraph itself.
///
/// Wording keys on Speckit's own diagnostics rather than OpenSpec's:
/// `speckit list --json` returns a bare array with no `root` field, so the
/// check is the command's failure text, and the store-declaration failure has
/// its own distinct message. Both trigger strings are quoted exactly as
/// `root_selection` emits them so the agent can recognize the case instead of
/// guessing from tone.
pub const PROJECT_ROOT_GUARD: &str = "**Project check:** These steps expect a project that already uses Speckit. Before the first step that writes anything (`new change`, `archive`, `sync specs`, or authoring an artifact file), confirm the project has a root: run `speckit list --json` (with `--store <id>` when a store is selected, since the store is then the root). A JSON array - even an empty one - means the project is set up. An error that begins `No Speckit root found` means it is not: there is no `speckit/` directory here, and a write such as `speckit new change` would create one as a side effect. The command also exits non-zero, which is that answer rather than a broken CLI, so read the message instead of retrying or working around it.

A `No Speckit root found` that lists `Registered stores:` is not a broken setup: it means this machine has stores while this directory has no local root. Pick one (`--store <id>`) or treat the request as unmanaged - it is never a reason to run `speckit init` on your own. A different failure - one that begins `Invalid store declaration in`, or that names this project's `speckit/config.yaml` - means the project does use Speckit through a store this machine cannot resolve (the store is not registered, or the `store:` line is malformed). Do not treat it as uninitialized and skip the branches below: stop before writing and show the user that error's message and its fix.

Otherwise, with no root, what happens next depends on how this workflow was reached:

- **Auto-selected**: you chose this workflow yourself, without the user naming Speckit, naming this skill, or invoking this skill by name. Stop using Speckit and answer the request normally, as you would with no Speckit installed. Do not ask them to set anything up and do not mention Speckit setup.
- **Explicit Speckit request**: the user named Speckit, named this skill, or invoked this skill by name. Stop before writing and ask how to proceed: set this project up (`speckit init`), target a store they already have (`--store <id>`), or continue without Speckit for this request. Wait for their answer.

In both branches, never create the root as a side effect: do not run `speckit init` until the user asks for it, do not hand-create `speckit/` files, and do not let a command create it.";

/// Substitute the guard into a workflow body.
///
/// Bodies that carry no marker are returned unchanged: an artifact-only text
/// with no steps to guard is legitimate, but every skill workflow must carry
/// the marker, which the test in `mod.rs` enforces.
pub fn with_project_root_guard(body: &str) -> String {
    body.replace(PROJECT_ROOT_GUARD_PLACEHOLDER, PROJECT_ROOT_GUARD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_marker() {
        let body = format!("head\n\n{PROJECT_ROOT_GUARD_PLACEHOLDER}\n\ntail");
        let rendered = with_project_root_guard(&body);
        assert!(rendered.contains(PROJECT_ROOT_GUARD));
        assert!(!rendered.contains(PROJECT_ROOT_GUARD_PLACEHOLDER));
    }

    #[test]
    fn leaves_a_body_without_the_marker_alone() {
        assert_eq!("no guard here", with_project_root_guard("no guard here"));
    }
}
