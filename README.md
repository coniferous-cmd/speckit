# Speckit

**AI-native spec-driven development workflow.**

Speckit is a Rust CLI that brings structure to AI-assisted coding. It manages specifications, change proposals, and implementation tasks — giving you and your AI tools a shared, version-controlled contract for every change.

```
explore → propose → design → spec → tasks → implement → archive
```

## Why Speckit?

AI coding agents are powerful but chaotic. Without structure, they produce inconsistent results, lose context between sessions, and make architectural decisions silently. Speckit solves this by making every change go through a spec-first pipeline:

- **Proposals** document the why and what before any code is written
- **Designs** capture architectural decisions
- **Specs** define requirements with testable acceptance criteria
- **Tasks** break work into numbered, trackable steps
- **Archives** preserve completed changes for future reference

All artifacts are markdown, version-controlled alongside your code.

## Installation

```bash
cargo build --release
```

The binary is at `target/release/speckit`.

## Quick Start

```bash
# Initialize Speckit in your project (interactive tool selection)
speckit init

# Or non-interactive for a specific AI tool
speckit init --tools claude

# Create a new change
speckit new change add-user-auth

# Check artifact completion status
speckit status --change add-user-auth

# Get enriched instructions for writing an artifact
speckit instructions proposal --change add-user-auth

# List all changes
speckit list

# Archive a completed change
speckit archive add-user-auth
```

## Core Commands

| Command | Description |
|---------|-------------|
| `speckit init` | Initialize Speckit in a project (interactive tool selection) |
| `speckit new change <name>` | Create a new change with scaffolded artifacts |
| `speckit status --change <name>` | Show artifact completion status |
| `speckit instructions <artifact>` | Get enriched instructions for an artifact |
| `speckit list` | List changes (use `--specs` for specs) |
| `speckit show <name>` | Show a change or spec |
| `speckit validate <name>` | Validate a change or spec for structural correctness |
| `speckit archive <name>` | Archive a completed change |
| `speckit schemas` | List available workflow schemas |
| `speckit config get/set/list` | Manage configuration |
| `speckit store setup/list` | Manage standalone stores |
| `speckit doctor` | Run diagnostics on Speckit root |
| `speckit context` | Print working context/brief (supports `--json`) |
| `speckit completion install <shell>` | Install shell completions (bash, zsh, fish, powershell) |

Most commands support `--json` for programmatic/agent consumption.

## Workflow

Every change follows a structured pipeline:

1. **Explore** — Brainstorm and investigate in read-only thinking mode
2. **Propose** — Create a change proposal with rationale (the "why" and "what changes")
3. **Design** — Produce a design document with architectural decisions
4. **Spec** — Write requirements with scenarios (acceptance criteria)
5. **Tasks** — Break the spec into numbered implementation tasks
6. **Apply / Continue / Update** — Work through tasks, tracking progress
7. **Archive** — Mark a change as complete, merge specs into the main spec set

Changes live in `speckit/changes/` with scaffolded markdown files for each artifact.

## AI Tool Integrations

Speckit generates tool-specific skill files and slash commands for **38 AI tools**, including:

| Category | Tools |
|----------|-------|
| **IDE Agents** | Claude Code, Cursor, Copilot, Cline, Windsurf, Kilo, Trae, PearAI |
| **Cloud Agents** | Devin, Codex, Amazon Q, GitHub Copilot Cloud |
| **Chat / CLI** | Gemini, ChatGPT, Kiro, OpenCode, Aider |
| **Frameworks** | Continue, Void, Roo Code, Conductor, Goose, Crush |
| **IDE Extensions** | VS Code, JetBrains, Zed, Neovim, Emacs |

Run `speckit init` to select your tools interactively, or `speckit init --tools claude,cursor` for non-interactive setup.

## Configuration

**Project-level** — `speckit/config.yaml` (inside the `.speckit` directory)

**Global** — `~/.config/speckit/config.json` (profile, delivery, feature flags)

```bash
speckit config list              # Show all config
speckit config get <key>         # Get a value
speckit config set <key> <value> # Set a value
```

## Stores

Standalone Speckit repos can be registered, shared, and managed independently of the project. Worksets provide personal working views that compose folders across stores.

```bash
speckit store setup    # Register a store
speckit store list     # List registered stores
```

## Shell Completions

```bash
speckit completion install bash
speckit completion install zsh
speckit completion install fish
speckit completion install powershell
```

## Project Structure

```
speckit/
├── Cargo.toml                   # Workspace manifest
├── crates/
│   ├── speckit-cli/             # Binary crate (CLI entry point)
│   ├── speckit-core/            # Core library (schemas, parsers, validation, adapters)
│   ├── speckit-commands/        # Command implementations + artifact templates
└── .github/workflows/ci.yml    # CI: fmt, clippy, build, test on Linux/macOS/Windows
```

## License

MIT — see [Cargo.toml](Cargo.toml) for details.
