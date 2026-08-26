# Agent Switchboard

> 面向 `Codex` 和 `Claude Code` 的本地、安全提供商控制台。

## 状态

`Tauri 2` 桌面应用已实现真实配置单路径：运行时只读取和管理本机 `Codex` 与 `Claude Code` 用户级配置。供应商记录保存在应用数据目录，首次启动为空；应用不会生成预置配置或供应商。

## 已实现能力

- 安全发现本地 `Codex` 和 `Claude Code` 配置。
- 新建、编辑、删除或只读导入每个客户端的提供商配置档。
- 供应商显式声明路由模式：`官方登录` 或 `自定义服务`；空地址不会被推断为官方。
- Codex 档案级模型运行参数（`model_reasoning_effort` 含 `xhigh`、`model_reasoning_summary`、`model_verbosity`、`model_context_window`）。
- Claude 四档模型映射（主模型、Haiku、Sonnet、Opus）与可选模型列表 `availableModels`。
- 将通用配置与提供商专属叠加层分离。
- 切换前预览精确的配置差异，并将确认绑定到当时的文件与候选内容；任一方变化都必须重新预览。
- 缺失的用户级配置只会在确认后创建，撤回会还原为原本不存在的状态。
- 备份、校验、原子写入，并在切换失败时回滚。
- 生效状态：当前路由是否匹配某个档案、是否被外部修改、上次切换时间与生效范围警告。
- 显示写入锁的空闲、占用、遗留与不确定状态；遗留锁只能经显式确认清理。
- 手动端点验证（WinHTTP）：可达性、HTTP 状态、延迟与检测时间，不自动选择端点。
- 撤回上一次切换；查看当前文件与任一备份的受管键差异。
- Codex 只保存并写入环境变量名 `env_key`，不保存密钥；Claude Code 保留既有登录与凭据环境。
- 展示诊断信息和恢复历史。

## 首个版本不做的事

- 管理 `Codex` 和 `Claude Code` 以外的客户端。
- 替代任一客户端的官方登录或凭据缓存。
- 运行云服务、遥测管道、代理服务器或用户账户系统。
- 请求失败后自动选择其他提供商。
- 未经未来功能明确授权，批量修改项目级配置。

## 架构方向

| 层 | 当前职责 |
| --- | --- |
| 桌面外壳 | 面向 Windows 的 `Tauri 2` 应用 |
| 界面 | `React` 与 `TypeScript`；仅呈现带类型的状态并请求操作 |
| 领域层 | 用于配置档、所有权、预览、锁和结果的 Rust 契约 |
| 客户端适配器 | 用于 `Codex TOML` 与 `Claude JSON` 的解析、合并和渲染操作 |
| 切换执行器 | 加锁、冲突检查、备份、校验、原子替换、恢复和诊断 |
| 本地数据 | 应用数据目录中的 `state/profiles.json` 与备份元数据 |
| 凭据 | Codex 只记录环境变量名；Claude Code 保留其既有登录与环境 |
| 写入安全 | 解析 → 差异 → 候选哈希确认 → 备份 → 校验 → 原子替换 → 验证 |

唯一事实来源将是一份带类型的配置补丁契约。提供商配置档只存储小型叠加层，而非用户配置文件的完整副本。属于宿主的字段保持不变。

## 后端参考边界

两个参考项目都为后端行为提供启发，但 `Agent Switchboard` 将独立实现，并以自身契约测试。

| 关注点 | 参考输入 | `Agent Switchboard` 决策 |
| --- | --- | --- |
| 提供商与通用配置 | `CC Switch` | 将客户端专属的提供商小型叠加层与通用配置分开存储；保留宿主所有字段。 |
| 客户端配置操作 | `CC Switch` | 为每种受支持格式使用显式适配器，而非通用文本替换路径。 |
| 执行安全 | `Codex++` | 在带类型的操作结果中明确锁状态、备份位置、变更文件、警告和恢复结果。 |
| 事务边界 | `Codex++` | 让规划和客户端适配器保持无副作用；由一个切换执行器拥有真实文件变更权。 |
| 用户界面 | `Agent Switchboard` 的 `DESIGN.md` | 独立创建 `Frosted Relay` 界面。不得复制任一项目的布局、组件、资源、交互文案或品牌。 |

`CC Switch` 使用 `MIT` 许可证，而 `Codex++` 仅采用 `AGPL-3.0-only`。二者都不是本项目的代码来源；其中 `Codex++` 尤其如此，未经明确的 `AGPL` 许可决定，不得改编其源码和界面。

## 产品原则

1. 切换执行前必须可解释。
2. 未知配置属于宿主客户端，不属于本应用。
3. 即使在备份和诊断中，密钥也必须保持保密。
4. 切换必须可逆。
5. 界面应让当前路由状态一目了然。

## Windows 工具链

`rust-toolchain.toml` 将本项目固定到 `stable-x86_64-pc-windows-gnu`，避免依赖本机 MSVC C++ 链接器和 Windows SDK。构建主机仍需让 `scoop gcc` 的 `bin` 目录位于 `PATH`，以提供 GNU 目标需要的 `dlltool.exe`；不会修改用户的全局 Rust 默认工具链。

## GitHub 自动构建

每次推送以及手动触发都会运行 [Windows 打包工作流](.github/workflows/windows-package.yml)：它使用与本地一致的 GNU Rust / MinGW 工具链，先执行 Rust 与前端测试，再构建 NSIS 安装程序，并将 `*-setup.exe` 作为 30 天可下载的 Actions 制品上传。

该工作流不自动创建 GitHub Release，也不包含代码签名；版本发布和签名需要在未来以独立的明确流程处理。

## 文档

| 文件 | 用途 |
| --- | --- |
| [AGENTS.md](AGENTS.md) | 工作与安全规则 |
| [DESIGN.md](DESIGN.md) | `Frosted Relay` 视觉与交互契约 |
| [MEMORY.md](MEMORY.md) | 已验证的长期项目决策 |
| [progress.md](progress.md) | 唯一的阶段目标文档：状态、验收与退出条件 |

## 参考资料

- `CC Switch` 仅作为架构与行为参考：
  https://github.com/farion1231/cc-switch
- `Codex++` 实现参考：
  https://github.com/BigPizzaV3/CodexPlusPlus
  其工作区拆分、诊断优先的管理器、可观察锁状态和备份纪律为本项目提供启发。它仅采用 `AGPL-3.0-only`，因此本项目不会复制其代码，也不会采用其 `CDP` 注入、应用补丁或会话 / `SQLite` 改写功能。
- `Codex` 配置与认证：
  https://developers.openai.com/codex/config-reference
- `Claude Code` 设置与环境变量：
  https://code.claude.com/docs/en/settings
  https://code.claude.com/docs/en/env-vars

## 下一个获准里程碑

在用户选定的、可恢复的本机配置副本上，逐项人工验证 Codex 与 Claude Code 的预览、切换、备份和恢复；随后再验证安装包升级不会丢失应用数据。
