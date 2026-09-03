# 项目问题检查报告

检查日期：2026-09-03

## 总体结论

项目当前可以编译，现有测试全部通过，但 workflow 的核心一致性仍存在问题。最主要的问题集中在自定义 schema、artifact graph、store 根目录解析和跨平台 JSON 路径处理。

当前仓库没有未提交改动，`main` 比 `origin/main` 多 1 个提交。当前提交本身标注了尚未完成 build/test 验证。

## 验证结果

- `cargo fmt --all -- --check`：通过
- `cargo build --workspace --all-targets --all-features`：通过
- `cargo test --workspace --all-features`：431 个测试全部通过
- Clippy：无错误，但存在多个 warning
- 构建存在 `proc-macro-error2 v2.0.1` future-incompatibility warning

## 主要问题

### 1. 高优先级：自定义 schema 不会被 status 正确使用

变更目录的 `.speckit.yaml` 可以声明 `schema: custom`，但 `status` 在没有显式传入 `--schema` 时仍然使用 `spec-driven`。

相关位置：

- `crates/speckit-commands/src/workflow/status.rs:90`
- `crates/speckit-core/src/artifact_graph/instruction_loader.rs:215`

已用临时项目复现：自定义产物文件存在，但 `status --json` 仍输出内置的 `proposal/specs/design/tasks`，并将 schema 报告为 `spec-driven`。

### 2. 高优先级：instructions 命令没有使用 artifact graph

`crates/speckit-commands/src/workflow/instructions.rs:138` 仍然硬编码四种产物：

- proposal
- specs
- design
- tasks

影响：

- 自定义 schema 的 artifact 无法生成 instructions
- instructions 与 status 的依赖关系不一致
- `design` 被错误地要求依赖 `specs`
- `tasks` 没有正确检查 `specs + design`
- core 中已有的 `generate_instructions` 没有真正接入 CLI

### 3. 高优先级：`--store` 在创建和归档流程中基本无效

`new change` 接收了 `store` 参数，但仍使用当前工作目录：

- `crates/speckit-commands/src/workflow/new_change.rs:119`

archive 虽然定义了 `ArchiveOptions.store`，但核心逻辑直接使用传入的 `project_path`：

- `crates/speckit-core/src/archive.rs:69`
- `crates/speckit-core/src/archive.rs:74`

结果是 `list/status/instructions` 和 `new/archive` 可能操作不同的 Speckit 根目录。

### 4. 中优先级：Windows JSON 路径格式不一致

status JSON 中：

- `change_root`、`planningHome` 使用 `/`
- `artifactPaths.*.resolvedOutputPath` 使用 `\\`

这会增加 AI agent 或脚本处理路径时的兼容风险。

### 5. 中优先级：change list 静默吞掉解析错误

以下位置使用 `unwrap_or(0)`：

- `crates/speckit-commands/src/change.rs:187`
- `crates/speckit-commands/src/change.rs:245`

proposal 解析失败时，列表会显示 `delta_count: 0`，用户看不到真实错误，容易误以为没有 delta。

## 其他工程质量问题

- Clippy warning 被 CI 用 `-A warnings` 全部忽略。
- 构建提示 `proc-macro-error2 v2.0.1` 将来可能无法通过 Rust 编译。
- core 中存在两套 `change_metadata` 实现，可能导致行为分叉。
- artifact glob 扫描手动递归目录，遇到循环目录 symlink 存在递归风险。

## 建议修复顺序

1. 统一 schema 解析逻辑，优先读取变更目录的 `.speckit.yaml`。
2. 让 status、instructions、implement、archive 共用同一个 artifact graph。
3. 统一所有命令的 store/root resolution。
4. 统一 JSON 中的路径格式。
5. 增加 custom schema、store 和 Windows JSON 的集成测试。

