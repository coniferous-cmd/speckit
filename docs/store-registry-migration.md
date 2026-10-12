# Migrating legacy store registrations

Older Speckit builds implemented store commands using a separate JSON registry
(`dirs::config_dir()/speckit/store-registry.json`) and a root-level `store.yaml`.
Lifecycle commands such as `context --store` instead used the core YAML registry,
so a listed store could not be resolved.

Store commands now share the core implementation:

- Registry: `$XDG_DATA_HOME/speckit/stores/registry.yaml` when set; otherwise
  `%LOCALAPPDATA%/speckit/stores/registry.yaml` on Windows or
  `~/.local/share/speckit/stores/registry.yaml` on Unix.
- Identity: `<store-root>/.speckit-store/store.yaml`.

## Upgrade existing registrations

1. Before replacing the old executable, save `speckit store list --json` and back
   up its JSON registry. Keep the original files until verification is complete.
2. Install the fixed executable. For each old entry, explicitly register its
   existing path and ID:

   ```sh
   speckit store register /absolute/path/to/store --id existing-id --yes --json
   ```

   `--yes` authorizes creation of the canonical identity file. Registration does
   not delete the legacy registry, root-level `store.yaml`, or project artifacts.
   Existing canonical identities and conflicting registrations are validated,
   not silently overwritten. Registration requires a complete `speckit/` root;
   inspect and repair incomplete roots before retrying.
3. Verify:

   ```sh
   speckit store list --json
   speckit store doctor existing-id --json
   speckit context --store existing-id --json
   ```

Legacy JSON entries are not automatically imported by read commands. This avoids
silently assigning identities or modifying project directories while listing or
resolving context. The new implementation has one authoritative registry; it does
not fall back to the legacy registry.
