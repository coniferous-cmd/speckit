# Speckit

**面向 AI 的规范驱动开发工作流。**

Speckit 是一款 Rust CLI，为 AI 辅助编程带来清晰的结构。它管理规范、变更提案和实施任务——让你与 AI 工具能针对每一次变更，共享一份受版本控制的约定。

```
探索 → 提案 → 设计 → 规范 → 任务 → 实施 → 归档
```

## 为什么选择 Speckit？

AI 编程代理能力很强，但也可能缺乏章法。没有明确流程时，它们会产出不一致的结果、在不同会话间丢失上下文，或悄然做出架构决策。Speckit 通过“先写规范”的流水线来解决这些问题：

- **提案**：在编写任何代码前记录变更的原因和内容
- **设计**：沉淀架构决策
- **规范**：使用可测试的验收标准定义需求
- **任务**：将工作拆分为带编号、可追踪的步骤
- **归档**：保留已完成变更，供后续查阅

所有产物均为 Markdown 文件，与代码一同进行版本控制。

## 安装

```bash
cargo build --release
```

可执行文件位于 `target/release/speckit`。

## 快速开始

```bash
# 在项目中初始化 Speckit（交互式选择 AI 工具）
speckit init

# 或为指定 AI 工具进行非交互式初始化
speckit init --tools claude

# 新建一个变更
speckit new change add-user-auth

# 检查产物完成状态
speckit status --change add-user-auth

# 获取用于编写某个产物的增强指引
speckit instructions proposal --change add-user-auth

# 列出所有变更
speckit list

# 归档已完成的变更
speckit archive add-user-auth
```

## 核心命令

| 命令 | 说明 |
|---------|-------------|
| `speckit init` | 在项目中初始化 Speckit（交互式选择 AI 工具） |
| `speckit new change <name>` | 新建一个变更，并生成产物脚手架 |
| `speckit status --change <name>` | 显示产物完成状态 |
| `speckit instructions <artifact>` | 获取某个产物的增强编写指引 |
| `speckit list` | 列出变更（使用 `--specs` 列出规范） |
| `speckit show <name>` | 显示一个变更或规范 |
| `speckit validate <name>` | 验证变更或规范的结构正确性 |
| `speckit archive <name>` | 归档已完成的变更 |
| `speckit schemas` | 列出可用工作流模式 |
| `speckit config get/set/list` | 管理配置 |
| `speckit store setup/list` | 管理独立存储库 |
| `speckit doctor` | 对 Speckit 根目录运行诊断 |
| `speckit context` | 输出工作上下文/简报（支持 `--json`） |
| `speckit completion install <shell>` | 安装 Shell 补全（bash、zsh、fish、powershell） |

大多数命令支持 `--json`，便于程序或代理调用。

## 工作流

每个变更都遵循一条结构化流程：

1. **探索**：以只读思考模式进行头脑风暴和调研。
2. **提案**：创建变更提案，说明理由（“为什么做”以及“改什么”）。
3. **设计**：产出包含架构决策的设计文档。
4. **规范**：使用场景编写需求（即验收标准）。
5. **任务**：将规范拆分为带编号的实施任务。
6. **实施 / 继续 / 更新**：逐项完成任务，并追踪进度。
7. **归档**：将变更标记为完成，并把规范合并到主规范集。

每项变更位于 `speckit/changes/`，其中包含为各类产物生成的 Markdown 脚手架文件。

## AI 工具集成

Speckit 可为 **38 种 AI 工具**生成专属的技能文件和斜杠命令，其中包括：

| 分类 | 工具 |
|----------|-------|
| **IDE 代理** | Claude Code、Cursor、Copilot、Cline、Windsurf、Kilo、Trae、PearAI |
| **云端代理** | Devin、Codex、Amazon Q、GitHub Copilot Cloud |
| **聊天 / CLI** | Gemini、ChatGPT、Kiro、OpenCode、Aider |
| **框架** | Continue、Void、Roo Code、Conductor、Goose、Crush |
| **IDE 扩展** | VS Code、JetBrains、Zed、Neovim、Emacs |

运行 `speckit init` 可交互式选择工具；或运行 `speckit init --tools claude,cursor` 完成非交互式配置。

## 配置

**项目级**：`speckit/config.yaml`（也支持 `speckit/config.yml`，`.yaml` 优先）

**全局**：`~/.config/speckit/config.json`（配置档案、交付方式和功能开关）

```bash
speckit config list              # 显示全部配置
speckit config get <key>         # 获取一个值
speckit config set <key> <value> # 设置一个值
```

### 在 Git worktree 中实施

在所选项目的配置文件中启用工作区指引（不是全局配置）：

```yaml
schema: spec-driven
use-worktree: true
```

运行 `speckit instructions implement --change <name> --json`，或调用生成的 implement-change skill。仅当 `use-worktree: true` 且状态为 `ready` 时，JSON 才包含 `useWorktree: true` 和独立的 `workspaceInstruction`，文本输出也显示对应指引。配置为 false、缺省，或状态为 `blocked` / `all_done` 时，两个字段和额外文本区域均省略；schema 原有 instruction 保持不变。已有的生成 skill 可通过 `speckit update` 刷新。

指引要求 agent **为每个 change 创建或验证复用一个独立 worktree**，报告目录和分支，并将代码编辑、构建和测试指向该目录。本地规划文件路径与任务勾选也指向目标 worktree；显式选定的外部 store 则保留 `--store <id>` 作用域和原规划路径，agent 必须先确认其对应的代码仓库。

必要规划输入必须在目标工作区中存在且保持最新。若未提交的规划内容不可用或已过期，agent 应暂停并请你先提交，不自动复制或提交。仓库或工作区归属不明确、准备失败时也应暂停，不回退到原目录实施。完成后保留分支和 worktree 供审查，不自动合并或清理。

这是 **agent 行为约定，而非 CLI 强制隔离**：查询实施指引不会创建分支或 worktree，也不会执行修改 Git 状态的操作。

## 存储库

独立的 Speckit 仓库可被注册、共享，并独立于项目进行管理。工作集（Workset）可将多个存储库中的文件夹组合成个人工作视图。

```bash
speckit store setup    # 注册一个存储库
speckit store list     # 列出已注册的存储库
```

## Shell 补全

```bash
speckit completion install bash
speckit completion install zsh
speckit completion install fish
speckit completion install powershell
```

## 项目结构

```
speckit/
├── Cargo.toml                   # 工作区清单
├── crates/
│   ├── speckit-cli/             # 二进制包（CLI 入口）
│   ├── speckit-core/            # 核心库（模式、解析器、验证器、适配器）
│   ├── speckit-commands/        # 命令实现和产物模板
└── .github/workflows/ci.yml     # CI：在 Linux/macOS/Windows 上执行 fmt、clippy、构建和测试
```

## 许可证

MIT——详见 [Cargo.toml](Cargo.toml)。
