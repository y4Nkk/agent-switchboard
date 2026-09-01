# Agent Switchboard

> 面向 `Codex` 和 `Claude Code` 的本地、安全提供商控制台。

## 状态

`Tauri 2` 桌面应用已实现真实配置单路径：运行时只读取和管理本机 `Codex` 与 `Claude Code` 用户级配置。供应商记录保存在应用数据目录，首次启动为空；应用不会生成预置配置或供应商。备份页可选连接用户自己的 Supabase 项目，保存加密后的供应商档案副本。

## 已实现能力

- 安全发现本地 `Codex` 和 `Claude Code` 配置。
- 新建、编辑、删除或只读导入每个客户端的提供商配置档。
- Codex 自定义服务固定使用官方内置 `openai` provider：写入 `model_provider = "openai"` 与 `openai_base_url`，不声明本项目私有的 `model_providers.*` 表。
- Codex 通用设置（`model_reasoning_effort` 含 `xhigh`、`model_reasoning_summary`、`model_verbosity`）与档案专属上下文窗口。
- Claude 四档模型映射（主模型、Haiku、Sonnet、Opus）与可选模型列表 `availableModels`。
- 将通用配置与提供商专属叠加层分离。
- 切换前预览精确的配置差异，并将确认绑定到当时的文件与候选内容；任一方变化都必须重新预览。
- 缺失的用户级配置只会在确认后创建，撤回会还原为原本不存在的状态。
- 备份、校验、原子写入，并在切换失败时回滚。
- 日志页查看软件自身的运行事件：记录阈值可选「调试 / 信息 / 警告 / 错误 / 静默」，已记录事件可按前四种级别筛选；支持手动刷新或打开受限的应用日志文件夹查看轮转文件。显示应用启动、设置/档案变更、切换/恢复、云备份和会话恢复等结果。失败只显示稳定错误码，不展示配置文本、路径、密钥、请求或响应内容。
- 生效状态：当前路由是否匹配某个档案、是否被外部修改、上次切换时间与生效范围警告。
- 显示写入锁的空闲、占用、遗留与不确定状态；遗留锁只能经显式确认清理。
- 手动端点验证（WinHTTP）：可达性、HTTP 状态、延迟与检测时间，不自动选择端点。
- 撤回上一次切换；查看当前文件与任一备份的受管键差异。
- 供应商档案直接保存 API 密钥；编辑器可填写，发现页与 CC Switch 导入可从本机配置导入。Codex 切换写入 `experimental_bearer_token`，Claude 切换写入 `env.ANTHROPIC_AUTH_TOKEN`；差异、预览、日志、错误和诊断始终脱敏。
- CC Switch 导入会将可转换的已启用 JavaScript 用量查询脚本一次性编译为本应用脚本；内建模板、禁用脚本和需要独立凭据或服务地址的脚本保持供应商导入并逐项说明，不把脚本文本或额外凭据发往界面。
- 在备份页配置用户自己的 Supabase 项目后，使用 Supabase Auth 与 RLS 保存加密的应用配置目录副本。备份密码经 Argon2id 派生 AES-GCM 密钥；登录密码、访问令牌和 Supabase secret/service key 不会保存或使用。
- 展示诊断信息和恢复历史。
- 会话管理：扫描本机 `Codex` 与 `Claude Code` 的 JSONL 会话，搜索、筛选并按需查看对话；可在新命令提示符窗口中恢复会话，也可复制恢复命令与已记录的工作目录，不会改写或删除会话文件。
- 概览 Codex 重置信号：概览先显示上一次成功读取的本地公开快照，用户显式刷新后才读取 Codex Runway 的公开 feed；刷新失败保留旧快照并标注缓存状态。它显示最近确认的全局重置、下一次公开预计与 Tibo 最近的重置相关动态，不读取本地会话、凭据或账号额度，且清楚标为非官方信息。切换到 Codex Radar 数据源须先取得其接口授权。
- Windows 系统托盘常驻：左键恢复主窗口；右键原生菜单按 Claude / Codex 分组列出可切换的供应商，真实生效者以勾选项标记；选择其他供应商仍走同一预览与原子切换事务。供应商卡成功读取的用量仅在当前应用进程内缓存，托盘在当前供应商标题和菜单项中显示该摘要，打开或刷新菜单绝不发起查询；菜单始终提供显式退出。
- 顶层「设置」页管理应用自身的窗口行为与外观：关闭策略、主题、动态效果、界面字体、始终置顶；界面默认使用随应用打包的 `Noto Sans SC`，也可切换为系统已安装字体；不写入任何客户端配置文件。

## 首个版本不做的事

- 管理 `Codex` 和 `Claude Code` 以外的客户端。
- 替代任一客户端的官方登录或凭据缓存。
- 运行应用自建的云服务、遥测管道、代理服务器或用户账户系统；可选云备份只连接用户自行配置的 Supabase 项目。
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
| 本地数据 | 应用数据目录中的 `state/configuration/common/{codex,claude}.json`、`state/configuration/providers/{codex,claude}/{id}.json`、`state/configuration/history/{codex,claude}.json`、`state/settings.json`、`state/cloud-backup.json`、备份元数据与受限的应用日志目录 |
| 凭据 | 档案 `apiKey` 明文保存于应用数据目录；切换将其分别写入 Codex bearer token 或 Claude auth token，所有展示/诊断均脱敏。云备份登录密码与访问令牌不落盘，远端只接收加密密文 |
| 写入安全 | 解析 → 差异 → 候选哈希确认 → 备份 → 校验 → 原子替换 → 验证 |

通用设置与每个供应商档案均有独立的、带类型的文件所有者。启用时由执行器合成固定通用设置与选中供应商的完整声明；当前供应商未声明的供应商受管字段会移除，宿主未知字段保持不变。

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

## 开发

- `npm run dev`：启动真实 `Tauri` 后端，并在本次 Vite 开发进程检测到后端首次就绪后打开一次 `http://127.0.0.1:1420`。Vite 只监听该显式 IPv4 回环地址，不提供第二主机名入口；后端热重启复用已有浏览器页面，不会重复打开标签。浏览器经 Vite 代理调用这个本机进程的同一套类型化命令，读取、预览和确认后的写入都作用于实际本机状态与客户端配置。
- `npm run dev:desktop`：启动可见的 `Tauri` 开发壳，供原生窗口、托盘和 WebView2 行为验证；它与浏览器开发入口使用同一后端实现。
- `npm run build`：执行前端类型检查并生成生产静态资源；它不打包桌面安装程序。

浏览器开发页顶部显示「浏览器开发 · 本机后端」。它不含示例数据或内存实现；配置写入仍必须经过现有的确认、加锁、备份、原子替换与写后验证事务。

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
