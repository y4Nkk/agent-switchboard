# Agent Switchboard 目标与进度

本文件是项目**唯一**的阶段目标文档。不得创建平行的 `GOALS.md`。`README.md` 说明产品；本文件记录每个阶段必须达成的目标、验收方式以及何时可开始下一阶段。

## 工作规则

- 同时最多只能有一个实施阶段处于进行中。
- 仅在每项列出的验收检查都有证据后，才能将阶段标记为已完成。
- 如目标、范围或验收条件变化，必须先在此更新，再改动实现。
- 将长期结论记录在 `MEMORY.md`；不得将本文件变成会话记录。
- 即使所有技术检查均通过，任何写入真实 `Codex` 或 `Claude Code` 文件的阶段仍需要用户明确授权。
- `CC Switch` 和 `Codex++` 可为独立编写的后端行为提供启发；界面由 `DESIGN.md` 独立实现，不得从任一项目迁移。

## 阶段图

| 阶段 | 目标 | 状态 |
| --- | --- | --- |
| 00 | 建立产品、安全与设计基线 | 已完成 |
| 01 | 在不触碰用户配置的前提下运行隔离桌面外壳 | 已完成 |
| 02 | 定义一份带类型的配置补丁契约和测试配置样本 | 已完成 |
| 03 | 安全读取并预览 `Codex` 与 `Claude Code` 配置 | 已完成 |
| 04 | 以事务且可逆的方式切换隔离测试配置 | 已完成 |
| 05 | 围绕真实预览构建 `Frosted Relay` 界面 | 已完成 |
| 06 | 以只读模式发现真实本地配置 | 已完成 |
| 07 | 提供显式的真实切换、恢复和 Windows 打包 | 进行中 |

## 00 — 文档与视觉基线

**目标：** 在任何代码或真实配置集成存在前，建立一套精简且不重叠的项目规则。

**范围**

- 创建项目目录。
- 定义产品范围、安全规则、设计系统、长期记忆和阶段目标。
- 定义参考边界：`CC Switch` 和 `Codex++` 为后端行为提供启发；`Frosted Relay` 是项目自己的界面。

**验收**

- `AGENTS.md` 将产品限制为 `Codex` 和 `Claude Code`。
- `README.md` 说明产品边界与计划架构。
- `DESIGN.md` 定义 `Frosted Relay`、`Dual Relay`、材质、字体与无障碍约束。
- `MEMORY.md` 仅包含简洁、可复用的项目决策。
- 本文件定义所有未来阶段的目标和验收条件。
- 文档区分后端参考模式与原创界面工作，并禁止从任一参考项目复制界面 / 代码。

**退出条件：** 五份根目录文档均存在、交叉引用正确，且未创建或修改任何代码或真实配置。

**结果：** 于 2026-08-26 完成。

## 01 — 隔离桌面外壳

**目标：** 创建最小的 `Tauri` + `React` + `TypeScript` 应用，使其能在本地打开而不读取或写入 `Codex` 或 `Claude Code` 状态。

**范围**

- 建立工作区、前端、Rust 核心和应用外壳。
- 使用 `DESIGN.md` 中的结构实现占位外壳。
- 只添加类型检查和聚焦单元测试所需的工具。

**验收**

- 开发应用能够打开。
- 进程不扫描、读取或写入真实用户配置路径。
- 前端与 Rust 检查成功运行。
- 运行时样式令牌只有一个所有者；组件不得引入局部色板或模糊常量。

**退出条件：** 能够在没有任何提供商功能的情况下验证隔离桌面外壳。

**结果：** 于 2026-08-26 完成。工作区由 `crates/asb-core`（纯契约）+ `crates/asb-switch`（仅执行器具备写入能力）+ `src-tauri`（`Tauri 2`、`React` / `TS`）组成。`cargo check --workspace --all-targets` 通过；`tsc --noEmit` 通过；`src/styles/tokens.css` 是唯一令牌所有者，并由 `src/styles/tokens.test.ts` 约束。开发应用窗口可在桌面打开（人工验证：名为 `Agent Switchboard` 的窗口渲染了侧边栏、`Dual Relay` 和状态面板；早期只暴露应用元数据）。

## 02 — 配置契约与测试配置样本

**目标：** 在接触文件格式前，为提供商配置档、通用配置、所有权、预览和备份定义一份经过校验的带类型契约。

**范围**

- 创建带类型的 `AppKind`、`ProviderProfile`、`CommonConfigPatch`、`SwitchPreview` 和 `BackupRecord` 契约。
- 定义可观察的锁状态契约，区分空闲、占用、陈旧和不确定状态。
- 准确定义 `Codex` 与 `Claude Code` 中哪些键属于应用。
- 创建包含宿主所有字段的合成 `TOML` 与 `JSON` 测试配置样本。
- 在编写任一格式适配器前，定义纯规划、客户端适配器与切换执行器之间的边界。

**验收**

- Rust、界面绑定、持久化和测试样本消费同一份契约。
- 报告锁状态时不隐式删除或恢复锁。
- 无效的提供商和补丁形状无法通过校验。
- 测试样本不包含真实凭据或私有端点。
- 所有权测试证明未识别键始终在补丁范围外。
- 规划与适配器被证明不产生变更；只有执行器能够接收真实写入依赖。

**退出条件：** 不再有任何模块需要猜测一次切换可以变更哪些字段。

**结果：** 于 2026-08-26 完成。契约位于 `crates/asb-core/src/contracts.rs`（`AppKind`、`ProviderProfile` / `ProviderDraft`、`CommonConfigPatch` / `PatchEntry` / `PatchValue`、`SwitchPlan`、`SwitchPreview` / `KeyChange` / `ChangeKind`、`BackupRecord`、`RouteState`、`ImportProposal`）；所有权表位于 `ownership.rs`（`Codex` 所有的键 + `model_providers.asb.*` 前缀、`Claude` 所有的键）；锁契约位于 `lock.rs`，其中纯函数 `classify_lock` 区分空闲 / 占用 / 陈旧 / 不确定。`validate.rs` 会拒绝空名称、非 `http(s)` URL、形似密钥或非法的 Codex 环境变量名、Claude 环境变量名、宿主所有的补丁键和应用不匹配，并给出修复提示。`test_support.rs` 仅在测试构建中提供样本，包含宿主所有字段且不含凭据。规划 / 适配器在结构上不产生变更：`asb-core` 完全没有文件系统依赖；只有 `asb-switch` 接收写入能力（注入 `SwitchIo` trait）。

## 03 — 读取并预览两个客户端配置

**目标：** 从隔离测试样本和受支持的用户级 `Codex`、`Claude Code` 配置解析内容，并生成不产生变更的预览。

**范围**

- `Codex TOML` 适配器。
- `Claude Code` 用户设置 `JSON` 适配器。
- 解析器诊断和脱敏配置差异。

**验收**

- 有效与无效 `TOML` / `JSON` 测试样本均有聚焦测试。
- 预览列出每个变更键、保留键、警告和备份目标。
- 生成预览不会写入文件。
- 每份返回的预览和错误都会脱敏密钥。

**退出条件：** 用户无需执行切换即可理解提议的切换。

**结果：** 于 2026-08-26 完成。`adapter/codex.rs` 使用保留格式的 `toml_edit` 编辑（宿主注释 / 布局保留，已由测试证明）；`adapter/claude.rs` 使用启用 `preserve_order` 的 `serde_json`。公共边界 `adapter::{preview, render, validate_syntax, route_state}` 会先校验规划（子模块入口为 `pub(crate)`，因此没有路径可绕过校验）。预览用脱敏后的变更前 / 后值列出每个变更键、保留宿主键、警告和备份目标。解析是纯操作（输入 `&str`，输出 `String` —— 结构上不写入）；解析错误包含行号，且会清除形似令牌的值；脱敏同时基于键与值形状（稳定的 `••••••••` 占位符）。测试样本覆盖有效与无效 `TOML` / `JSON`、幂等渲染、宿主键保留和脱敏。

## 04 — 隔离测试配置的事务式切换

**目标：** 在提供商配置档之间安全切换，其备份和回滚行为与生产契约等价。

**范围**

- 文件加锁、冲突检测、带时间戳备份、临时写入、校验、原子替换和写后验证。
- 用户可见的锁诊断以及显式的陈旧锁恢复流程。
- 恢复紧邻的前一份备份。
- 使用内容哈希或等价的显式比较检测外部变更。

**验收**

- 提供商 A → 提供商 B → 恢复会保留每个宿主所有的测试配置字段。
- 无效临时写入会恢复前一份内容。
- 检测到外部编辑会阻止切换，并产生可用诊断。
- 占用或不确定锁会阻止切换；陈旧锁恢复会被记录且需要显式操作。
- 执行结果会报告锁状态、变更目标、警告、备份位置和恢复结果，无需由界面自行推断。
- 测试仅在隔离的临时目录中运行。

**退出条件：** 不得有任何失败模式能静默留下部分配置写入。

**结果：** 于 2026-08-26 完成。`asb-switch` 执行器实现：加锁 → 哈希检查 → 预览 → 渲染 → 再次哈希 → 备份（+ 哈希旁车文件）→ 临时写入 → 语法校验 → 原子重命名（`MOVEFILE_REPLACE_EXISTING`）→ 写后验证 → 释放；若写后验证失败，则恢复紧邻的前一份备份。锁文件（`<target>.asb-lock`）通过纯契约和真实 Windows `pid` 探测分类（`OpenProcess`；访问被拒绝 → 不确定）；占用 / 陈旧 / 不确定锁会在不删除锁的情况下阻止操作；`recover_stale_lock` 是显式操作并返回已记录条目。通过与预览时哈希进行 `SHA-256` 比较检测外部编辑。`SwitchOutcome` 报告锁状态、变更目标、警告、备份记录和恢复结果。11 项集成测试仅在 `tempfile` 目录中运行，其中包括用于临时写入失败和写后校验损坏的确定性失败注入（注入 `SwitchIo`）。

## 05 — `Frosted Relay` 工作界面

**目标：** 按 `DESIGN.md` 将预览、配置档、诊断和恢复组织成清晰的桌面工作流。

**范围**

- `Dual Relay`、提供商列表、通用设置表单、预览检查器和备份历史。
- 空状态、加载状态、校验状态、确认状态和失败状态。
- 键盘导航和减少动态效果行为。

**验收**

- 每个视觉令牌均来自唯一令牌所有者。
- `Dual Relay` 除颜色外，还通过文字和位置表示状态。
- 差异和配置文字无需背景模糊也可阅读。
- 界面评审确认标签、徽标和副标题传达真实信息，而非装饰空白区域。
- 界面测试证明组件请求带类型的预览和操作，而不构造配置文本或文件系统路径。
- 类型检查通过；聚焦界面测试覆盖主要操作和错误状态。
- 视觉评审确认没有嵌套玻璃和不必要的装饰性动画。

**退出条件：** 能够完全通过界面完成受控配置切换和恢复。

**结果：** 于 2026-08-26 完成。`Frosted Relay` 界面按 `DESIGN.md` 实现：`Dual Relay`（双轨道、圆点 + 文字 + 位置）、提供商行、通用设置叠加表单、差异检查器（接近实心、代码字体）、带确认面板的备份历史表、空 / 加载 / 错误状态、键盘焦点环和减少动态效果支持。`src/api/client.ts` 是唯一与后端通信的模块；`src/boundary.test.ts` 强制组件既不导入后端，也不构造配置文件文本。令牌所有权由测试约束。隔离测试覆盖选择、预览、确认、切换与恢复；真实用户文件写入不由自动化测试执行。

## 06 — 只读本地发现

**目标：** 识别已安装的 `Codex` 与 `Claude Code` 用户级配置，并在不改变用户状态的前提下报告其就绪情况。

**范围**

- 发现受支持的配置位置。
- 检测缺失文件、解析错误、受管理设置和不受支持的提供商形状。
- 展示只读诊断摘要和导入建议。

**验收**

- 发现不会变更文件系统或凭据。
- 人工验证确认发现后真实配置时间戳和内容均不变。
- 报告不受支持状态，而非猜测或覆盖。

**退出条件：** 用户能在充分了解安全状态后决定是否导入当前设置。

**结果：** 于 2026-08-26 完成。`asb-core::discovery` 是纯操作（注入读取函数和路径），将每个文件分类为缺失 / 解析错误（行号 + 已清除的消息）/ 正常，并附带受管理标记与警告（不受支持的 Codex 协议、明文 `ANTHROPIC_AUTH_TOKEN`）。仅当当前 Codex 自定义供应商使用 `responses` 协议时才提供导入建议；Claude 建议只读取模型与服务地址。`discover_local` 命令最多读取两个真实用户级文件；「发现」页面只展示路径、状态、警告与显式导入操作，不写入文件。

## 07 — 真实配置单路径与打包

**目标：** 移除运行时虚拟配置路径和预置供应商，使应用只管理本机 `Codex` 与 `Claude Code` 配置，并保留由用户确认的可逆切换与 Windows 打包。

**范围调整（2026-08-26）：** 不再提供两套目标选择。供应商配置档保存在应用数据目录的独立存储中，首次启动为空；它们保存稳定标识、客户端、路由模式、名称、模型、端点、Codex 环境变量名及档案级模型选项，绝不保存密钥。通用补丁与不含密钥的切换日志另行保存。Claude Code 凭据继续由其既有登录或环境管理。所有状态、预览、切换、恢复和备份历史均指向用户级配置路径。自动化测试仍使用隔离的临时文件，不构成运行时模式。

**范围**

- 为真实用户配置写入提供显式确认流程。
- 删除运行时虚拟路径、预置供应商和相关界面选择器。
- 提供真实供应商配置档的新增、编辑、删除与只读导入入口。
- 将 Codex 环境变量名写入受管表，并在所有预览、错误和诊断中脱敏密钥。
- 备份浏览器、恢复流程和发布打包。

**验收**

- 每次真实切换均在确认前展示精确差异和备份位置。
- 首次启动没有预置供应商；每个可切换供应商均来自用户新增或真实配置导入。
- 运行时不存在虚拟配置命令、非本机目标路径或相关界面文案。
- 状态、预览、切换、恢复和备份历史只访问真实用户级配置路径。
- 受控且由用户授权的切换与恢复均经过人工验证。
- 打包后安装与启动不会丢失应用自身的本地数据。
- 应用诊断、导出或软件包产物中不出现密钥。

**退出条件：** 用户能够安全地将应用用于日常 `Codex` / `Claude Code` 提供商管理。

**当前状态（2026-08-26）：** 运行时已迁移为真实配置单一路径：`config_status`、预览、切换、恢复、锁和备份命令都解析用户级目标；旧的虚拟命令、路径、预置供应商和界面选择器已删除。供应商的新增、编辑、删除、只读导入和空初始存储已实现；Codex 托管表使用 `wire_api = "responses"`。

**概览本机会话速览（完成，2026-08-31）：** 新增了一个仅由用户显式读取的概览面板，复用既有 `list_sessions` 的只读受控扫描结果，而不引入 Codex Runway 的额度、成本、远端账号接口或本地会话索引。面板只按 Codex / Claude Code 显示已发现会话数量、最近本地记录更新时间和目录读取提示，并提供进入既有「会话」页的入口；不暴露会话标题、源路径、对话内容、凭据或虚构用量。已验证：未请求时不扫描；同一读取操作不并发堆积；成功、空结果、部分目录提示和读取失败均有明确 UI；前端继续只经带类型 `client.ts` 调用，且不修改任何会话文件。`npm test`（29 文件、128 测试）与 `npm run build` 均通过。

**路由模式与模型映射（2026-08-26 第二轮）：** 供应商档案显式声明 `官方登录` / `自定义服务` 两种路由模式，空地址不再被推断为官方。Codex 官方模式写回 `model_provider = "openai"` 并移除 `model_providers.asb` 托管表；Claude 官方模式移除 `env.ANTHROPIC_BASE_URL`，凭据继续由既有登录管理。Claude 档案使用主模型、Haiku、Sonnet、Opus 与 `availableModels`；已弃用的 `ANTHROPIC_SMALL_FAST_MODEL` 会在下一次切换清理，导入时升格为 Haiku 档。Codex 的运行参数属于供应商档案，通用配置只保留真正的全局键（Codex `disable_response_storage`）。旧 `profiles.json` 迁移会按地址补齐路由模式，并将可确定的旧档案级通用设置写入同客户端的每个档案；遇到凭据引用、无法归属的服务表或无效形状时停止迁移且不改写原存储，绝不静默丢弃数据。

**生效状态与回退（2026-08-26 审查收尾）：** `config_status` 报告 `MatchStatus`：匹配当前档案、档案或通用设置已变更、已恢复备份、外部修改、未托管或不可判定。恢复记录优先于同形状档案，因此不会错误点亮供应商行。预览同时绑定当前文件哈希与候选渲染哈希；档案或通用设置在确认前变化会被拒绝，界面清除旧预览。缺失的用户级文件只会在确认后创建，撤回会恢复为“不存在”。恢复前快照拥有旁车元数据，因而也可撤回；备份浏览只接受属于当前客户端、当前本机目标且与旁车路径一致的记录。写入配置成功但审计日志保存失败时，操作以警告而非失败返回。概览显示可观察写入锁，并要求确认后才清理遗留锁。

**端点验证（2026-08-26 审查收尾）：** 供应商编辑器提供手动“验证端点”：Windows 原生 WinHTTP 使用系统自动代理发送 HEAD，在 HTTP `405` / `501` 或请求错误时回退 GET，只展示可达性、HTTP 状态、延迟与检测时间，不自动选择端点。地址变更会使旧结果失效。

**验证证据（2026-08-26 审查收尾）：** `cargo fmt --all -- --check`、`cargo test --workspace`（96 项）、`cargo build --workspace`、`npm run typecheck`、`npm test`（35 项）与 `npm run build` 已通过；NSIS 安装程序已生成到 `target/release/bundle/nsis/Agent Switchboard_0.1.0_x64-setup.exe`。关键新增回归覆盖候选预览过期、首次创建后撤回为缺失、恢复后恢复前快照可用、恢复写后校验回滚、遗留锁空闲判定、旧存储迁移保真、审计日志失败警告与前端预览失效。本轮仍未对用户真实配置执行切换或恢复，安装程序也尚未安装或启动验证。

**从 CC Switch 导入供应商（2026-08-27 完成）：** 发现页提供只读扫描本机 CC Switch 数据库（`~/.cc-switch/cc-switch.db`，SQLite，`mode=ro&immutable=1` 连接）并一键导入供应商档案。范围与规则：

- 仅导入 `app_type` 为 `codex` / `claude` 的供应商；其余客户端列入跳过清单并说明超出产品范围。
- 密钥永不导入：提案结构在类型层面无携带密钥的字段；`ANTHROPIC_AUTH_TOKEN`、`auth.OPENAI_API_KEY` 与 OAuth 令牌仅用于判断路由模式，值不进入任何输出、警告或日志。
- Claude 映射：`env.ANTHROPIC_BASE_URL` → 服务地址（有→自定义，无→官方）；`ANTHROPIC_MODEL` → 主模型；`ANTHROPIC_DEFAULT_{OPUS,SONNET,HAIKU}_MODEL` → 三档模型映射；无法表示的键仅以字段名列警告。
- Codex 映射：`auth` 含 OAuth 令牌或空 `OPENAI_API_KEY` → 官方登录；否则解析内嵌 TOML 的 `model_providers` 表（`wire_api` 必须为 `responses`），表内额外键（如 `http_headers`）会导致该供应商跳过并说明原因，TOML 解析失败同样跳过。
- 通用配置（`common_config_*`）不导入，由用户在通用设置页自行配置。
- 去重：与现有档案七字段全等者标记“已存在，将跳过”；导入时重新扫描避免过期预览。
- 全程不写真实 `Codex` / `Claude Code` 配置；唯一写入目标是应用自有 `state/profiles.json`。

实现：映射规则唯一所有者为 `asb-core::ccswitch`（纯函数，注入行数据）；SQLite 只读访问在 `src-tauri/ccswitch_source.rs`（`rusqlite` bundled，首个 C 编译依赖，已在 windows-gnu 工具链验证）；命令 `scan_ccswitch` / `import_ccswitch_profiles`；界面为发现页「从 CC Switch 导入」区块，逐项复选（通用 Checkbox 组件）、批量导入并汇总结果。`LocalState::profile_exists` 与 `import_profile` 共用同一七字段判等谓词。

验证证据：`cargo fmt --all -- --check` 通过；`cargo test --workspace` 111 项全部通过（含 asb-core 映射 12 项、tempdir 假库扫描/导入集成 3 项）；`npm run typecheck` 与 `vitest` 40 项通过（含扫描预览、勾选、导入参数与结果汇总）；对本机真实库执行 `--ignored` 只读扫描测试：10 行供应商 → 7 可导入 + 3 跳过，警告全部来自固定模板、无密钥值；NSIS 包已重建并静默安装。

**供应商列表拖拽排序（2026-08-28 完成）：** 供应商行新增行首拖拽把手，列表可拖拽重排（2026-08-28 用户指令，参考 CC Switch 的交互方式；界面用本系统令牌实现）。显示顺序的唯一所有者是 `state/profiles.json` 的 `profiles` 数组——不引入每档案排序字段，界面、持久化与运行时消费同一顺序。`reorder_profiles` 命令按客户端校验排序清单（必须覆盖该客户端全部档案、不得重复、不得混入另一客户端）后重写该客户端在数组中的片段，另一客户端档案的绝对位置不变；非法清单整体拒绝、不部分写入。前端引入 `@dnd-kit`（core / sortable / utilities，React 19 兼容）：`PointerSensor` 8px 启动约束保证行点选不被吞、`KeyboardSensor` + `sortableKeyboardCoordinates` 提供键盘排序、`closestCenter` + 垂直列表策略；把手是行内唯一拖拽目标，把手按钮 ≥40px 且保留可见键盘焦点。拖拽中的行加强为行动蓝描边与路由投影，不缩放。

验证证据：`cargo fmt --all -- --check` 通过；`cargo test`（src-tauri 17 项，含重排两条：跨客户端位置保真、非法清单拒绝且不改写存储）；`npm run typecheck` 与 `npm test` 49 项通过（含把手按需渲染、键盘重排上报新顺序）。

**供应商行「启用」主按钮（2026-08-28 完成）：** 非生效供应商行的动作簇左侧新增实心行动蓝「启用」主按钮（Play 图标，复用共享 `.asb-btn-primary` 令牌样式），生效行不显示、由既有「使用中」胶囊表达状态（2026-08-28 用户指令补齐 CC Switch 式行主按钮；行为经用户选定）。行为契约：启用按钮点击＝选中该行并拉起变更预览（与预览图标同一处理器），写入仍需显式确认——为此变更预览面板头部新增「确认切换」主按钮，复用概览页路由控件同款 `ConfirmSheet` 确认弹窗与 `runSwitch`（带 `confirmWrite`）。供应商页由此具备完整的「启用 → 看差异 → 确认切换」闭环，此前确认入口仅在概览页。安全规则不变：每次真实切换在确认前展示精确差异与备份位置，任何行内按钮都不直接写配置。现展示时机见「供应商行『启用』主按钮选中门控」条目。

验证证据：`npm run typecheck` 与 `npm test` 50 项通过（含新增：生效行无启用按钮、非生效行点击启用回调携带完整档案）。

## 当前风险

- 真实配置切换与恢复仍需在用户选定的可恢复配置副本上逐项人工验证。
- "需要重启客户端生效"目前是静态提示；应用不检测 Codex / Claude Code 进程的启动时间。
- 除非明确将其加入新目标，`MCP` 管理仍不在这些初始阶段范围内。

**通用设置勾选写入（2026-08-28 完成）：** 通用设置页改为 `Codex` / `Claude` 双 `Tab`（品牌图标分段，与供应商页一致），每个勾选行对应一条官方配置参考中的真实配置行，勾选 = 该行写入通用配置文件，取消勾选 = 该行移除，写经由执行器完整事务（加锁 → 外部变更校验 → 备份 → 临时写入 → 语法校验 → 原子替换 → 写后校验）即刻生效，不等「安全切换」。选项目录：

- Codex（`~/.codex/config.toml`）：`disable_response_storage = true`、`hide_agent_reasoning = true`、`show_raw_agent_reasoning = true`。
- Claude（`~/.claude/settings.json`）：`alwaysThinkingEnabled = true`、`spinnerTipsEnabled = false`、`attribution.coAuthoredBy = false`（官方已弃用 `includeCoAuthoredBy`，故采用 `attribution` 路径）。

契约变化：`PatchEntry.value` 升为 `Option`（`null` = 移除该行，叠加与通用两条写路径一致执行 `None → RemoveIfPresent`，取消勾选不再遗留旧行）；开关目录唯一所有者 `asb_core::ownership::common_toggles`，键同步进入 `CODEX_OWNED_KEYS` / `CLAUDE_OWNED_KEYS`；`SwitchLog` 增加 `operation`（`serde default` 兼容旧记录），通用写入记 `commonSettings` 并产生新匹配态 `MatchStatus::matchesSettings`（概览显示「与上次通用设置写入一致」）；执行器抽出与 `SwitchPlan` 解耦的事务核心，`execute_common` / `read_common_preview` 与切换共用同一管线；命令层新增 `common_toggles`（返回目录 + 文件真实行状态）与 `apply_common`（要求 `confirm_write`）。勾选状态始终反映配置文件真实行状态（失败后自动重查），修复 `toml_edit` 标量 repr 携带 decor 空格导致精确比较失真的隐患。验证：`cargo test --workspace`（106 项）与 `npm test`（50 项）通过；真实用户文件仍未被自动化测试触碰。

**开发机调试开关（2026-08-28）：** 应对「每次改动都要重新构建」的迭代成本：`tauri` 启用 `devtools` 特性，新增 `toggle_devtools` 命令并在前端监听 `F12` 切换 WebView 检查器（唯一后端入口 `src/api/client.ts::toggleDevtools`，边界测试约束不变）。运行中的 exe 可直接改 CSS / DOM 实时预览；源码级迭代用 `npm run tauri dev`（Vite 热重载，调试构建自带 devtools）。按用户指令：仅在用户明说「打包」时执行 `npm run tauri build`。验证：`cargo test --workspace`（122 项）与 `npm test`（50 项）通过；本轮未构建交付物。

**推理强度档位滑块（2026-08-28 完成）：** 通用设置 `Codex` `Tab` 新增推理强度滑块（结构参考 `spiralcoder` 的 `Slider`，视觉走本系统令牌：行动蓝填充 + 刻度 + 白滑钮），六档：默认（移除该行）→ `minimal / low / medium / high / xhigh`，滑动即经 `apply_common` 事务写入 `model_reasoning_effort` 并立刻生效。契约随之迁移单一所有者：`model_reasoning_effort` 从 `PROFILE_EXCLUSIVE_KEYS` 移出（保留在 `CODEX_OWNED_KEYS`），`CodexModelSettings` 删除 `reasoningEffort` 字段——供应商编辑器的运行参数只保留摘要 / 详细度 / 上下文窗口，适配器运行参数叠加与发现导入同步收敛；通用补丁校验拒绝官方档位之外的值。旧存储迁移：`profiles.json` 中档案级 `reasoningEffort` 显式升格进同客户端通用补丁（通用补丁已声明该键时不覆盖；多个档案档位冲突时停止迁移并说明，绝不静默丢弃）。新命令 `common_effort` 返回文件真实档位，滑块始终反映文件状态。验证：`cargo fmt --check`、`cargo test --workspace`（123 项）、`npm run typecheck`、`npm test`（51 项）通过；按用户指令本轮未打包。

**Tooltip 与复选框复刻（2026-08-28 完成）：** 从 `spiralcoder` 复刻两个通用组件并接入。`Tooltip`：引入 `@radix-ui/react-tooltip`（悬停/聚焦触发、Portal 定位、150ms 延迟），深色墨底 + 画布白字 + 模态投影，动画随 `prefers-reduced-motion` 关闭；禁用按钮经 `span` 锚点包裹也能给出原因提示。`Checkbox`：改为参考项目的 `sr-only input + 绘制控件` 结构（`data-checked` 驱动、悬停描边、按下缩放、内嵌高光），新增 `indeterminate` 半选态（读屏 `mixed`），既有调用方 API 不变。接入点：通用设置推理强度档位值签（悬停显示将写入的确切行 / 默认档说明）与 `Dual Relay` 的「查看变更」「安全切换」按钮（禁用时悬停解释前置条件：未选档案 / 未确认预览 / 写入中）。验证：`npm run typecheck`、`npm test`（54 项）与 `cargo check` 通过；本轮未打包。

**滑块完整样式复刻（2026-08-28）：** 按用户指令把推理强度滑块升级为 `spiralcoder` 完整复刻：胶囊轨道（墨色 9% 底 + 内嵌描边）、能量渐变填充（新令牌 `--asb-gradient-energy`：安全绿→行动蓝→Claude 紫 96°，本系统第二处获准渐变表面）、填充内 9 颗白色微光粒子（glow + 1.8s 闪烁动画、逐粒子定位与延迟）、刻度悬停放大（ease-out-back）、28px 白滑钮（悬停加深投影、按下 0.96 + 120ms 快速跟随）、原生 range 全尺寸透明叠加（webkit/moz thumb 透明占位）。`tokens.css` 新增 `--asb-motion-fast` 与 `--asb-ease-out-back`；`prefers-reduced-motion` 下全部动效静止。验证：`npm run typecheck`、`npm test`（54 项）通过；未打包。

**原生 tooltip 全量替换（2026-08-28 完成）：** 界面不再使用浏览器原生 `title` 提示，全部改为复刻的深色 `Tooltip` 组件。替换点：供应商页与通用设置页的品牌图标 Tab（`Codex` / `Claude`，补 `aria-label` 维持可及名）、新建供应商 `+` 钮（禁用态经 `span` 锚点包裹）、窗口控制三钮（最小化 / 最大化-还原（动态）/ 关闭，`side=bottom`）、供应商列表的拖拽手柄与启用 / 预览 / 编辑 / 删除五个行内钮（拖拽手柄与 dnd-kit 事件经 Radix `Slot` 合并无冲突）。`ConfirmSheet` 的 `title` 属性是对话框标题而非提示，保持不变。可及名一律由 `aria-label` 承担，提示仅作视觉补充。验证：`npm run typecheck`、`npm test`（54 项，含 dnd-kit 键盘拖拽）通过；未打包。

**配置预览改为候选文件全文（2026-08-28）：** 按用户指令把通用设置页的预览模块从抽象差异列表换成「优质打印」的配置文件内容本身：`preview_common` 现返回 `{ file, content }`——`content` 是叠加补丁应用后的完整候选文件文本（`adapter::common_render`，宿主内容逐字保留）；新组件 `FileContentPreview` 以行号 + 轻量语法着色渲染（表头/花括号加粗、键名弱化、布尔值安全绿加粗，TOML 与 JSON 行形皆识别），近实心代码底、320px 内滚动。验证：独立 `CARGO_TARGET_DIR` 编译（dev 实例占用主 target，不触碰）与 `npm test`（54 项）通过；未打包。

**时间显示改为本地时区（2026-08-28 完成）：** 按用户指令，全局时间显示从 `UTC` 原文改为本机本地时区渲染。唯一所有者 `src/lib/time.ts::timeLabel` 保持不变，实现改为经 `Date` 本地取值并附带真实偏移标注（`YYYY-MM-DD HH:MM:SS（UTC+8）`，非整点偏移如 `（UTC+5:30）`）。覆盖全部消费点：概览匹配态与上次切换、备份历史、探针面板、撤销/恢复提示。测试确定性：`vite.config.ts` 的 `test.env` 固定 `TZ=Asia/Shanghai`（Windows Node 已验证识别），本机（UTC+8）与 CI runner（UTC）断言一致；`App.test` 断言更新为转换后的 `16:00:00（UTC+8）`，反向锁定换算逻辑。验证：`npm run typecheck`、`npm test` 54 项通过；未打包。

**自有滚动条全量替换（2026-08-28 完成）：** 按用户指令设计本项目自己的滚动条并替换全部原生滚动条——「墨线滚动条」（`Frosted Relay` 语言）：`10px` 槽位、轨道 / 按钮 / 角块全透明（无凹槽底色），滑钮以透明描边内缩为 `4px` 墨色药丸线（`background-clip: padding-box`），拖拽命中区保持全槽宽；悬停加深、按下最强，最短 `32px` 防短内容滑钮过小；深浅模式各一套基于正文墨色的 `tint`，不引入第二强调色。契约：规则唯一所有者为 `base.css` 的 `::-webkit-scrollbar` 全局块，覆盖工作区（`.asb-main`）、配置预览（`.asb-filepreview-body`）与文本域等一切滚动表面；值由 `tokens.css` 新令牌提供（`--asb-scrollbar-gutter` / `-thumb` / `-thumb-hover` / `-thumb-active`、`--asb-radius-pill`）；刻意不使用标准 `scrollbar-width` / `scrollbar-color`（Chromium 121+ 下会禁用自定义伪元素样式，WebView2 即受影响）。设计意图登记 `DESIGN.md §6`。验证：`npm run typecheck`、`npm test` 55 项通过（含令牌所有权测试，确认无散落色值）；未打包。

**选中与生效行的渐变描边样式（2026-08-28 完成）：** 按用户指令参考 CC Switch 与 spiralcoder 两个项目，为供应商行的选中态补齐样式搭配与颜色。映射：spiralcoder 的「accent border」渐变描边（静／动两档）→ 选中行用静态蓝→亮蓝 conic 渐变描边，生效行用同款缓慢流动（6s 一周，`prefers-reduced-motion` 时静止）；CC Switch 生效卡的同色软投影 → 新令牌 `--asb-accent-shadow`（明暗两套）。两态共享安静蓝调实心表面（`--asb-row-selected`），层次为：悬停＝表面轻染、选中＝表面＋静态描边＋软投影、生效＝表面＋流动描边＋软投影＋「使用中」胶囊＋头像翻转；状态仍由文字与描边共同承载，不只靠颜色。实现为 `padding-box/border-box` 双层背景（描边即渐变），前置纯色回退声明，`conic-gradient`/`color-mix`/`@property` 不受支持时退回行动蓝实线描边；全部值由 `tokens.css` 拥有（`--asb-accent-border-paint`、`--asb-accent-shadow`、`--asb-accent-border-duration`，明暗各一套），组件零色值。设计意图登记 `DESIGN.md §8`。

验证证据：`npm run build` 通过（CSS 产物含全部新规则）；以构建产物 CSS 起`本地 HTTP` 临时页，用 Playwright 截图并经视觉模型核验三态渲染——普通行白底细边、选中行渐变描边＋浅蓝底＋蓝调投影＋启用按钮、生效行同套＋使用中胶囊，无渲染异常；`npm test` 56 项通过（类名未动，纯样式变更）；临时产物已清理。

**供应商行地址链接与明细分行（2026-08-28 完成）：** 按用户指令，行内明细从「主机名 · 模型」单行改为两行：首行是可点击的服务地址链接（仍显示主机名、行动蓝、悬停下划线），次行是模型（仅有值时渲染），无地址时首行保留路由模式文案。跳转链路：查证 wry 源码确认 WebView2 下未接管的新窗口请求一律 `SetHandled(true)` 静默拦截，纯 `<a target="_blank">` 在 Tauri 内点击无效——因此引入官方 `tauri-plugin-opener`（Rust 侧注册插件，JS 侧 `openUrl`，锚点 `preventDefault` 后走插件），能力作用域限定 `opener:allow-open-url` 仅允许 `http/https` URL，与项目权限收敛姿态一致。锚点保留 `href` 与 `title`（悬停可见完整地址），键盘焦点环复用全局 `:focus-visible`。设计意图登记 `DESIGN.md §8`。

验证证据：`npx vitest run src/components/ProviderList.test.tsx` 9 项通过（含新增：链接渲染独立成行且 `href` 为完整地址、点击调用 `openUrl` 传完整 URL，插件以 `vi.mock` 隔离）；`tsc` 对本改动零错误（仓库剩余错误来自并行会话的 `__debug3__.test.tsx` 调试残留）；Rust 侧 `cargo check -p agent-switchboard` 通过（含新插件依赖）。

**配置预览换行显示（2026-08-28 完成）：** 按用户指令消除预览模块的横向滚动：`FileContentPreview` 行结构由 `white-space: pre` 整行不折改为「行号列 + 文本列」网格——行号列固定右对齐，文本列 `pre-wrap` + `overflow-wrap: anywhere`（保留缩进与空格、超长无空格令牌如长服务地址也能折行），折行文本始终对齐行号列右侧，不再回卷到行首。差异检查器与键值面板本就 `overflow-wrap: anywhere`，无需改动。契约登记 `DESIGN.md §8`（配置预览任何情况下不得出现横向滚动）。验证：`npm run typecheck` 通过、`npm test` 60 项中 59 过（唯一失败为用户并发改动的 `BackupHistory` 缓存断言，非本轮引入）；未打包。

**通用设置完整覆盖官方目录（2026-08-28 完成）：** 按用户指令把通用设置从「3 开关 + 1 滑块」扩为完整官方目录（依据当日核对的 Codex 官方 config 参考与 Claude Code settings 参考逐键核实类型与取值）：`Codex` 13 个开关 + 8 个档位（推理强度滑块、推理摘要、回复详细度、助手个性、网页搜索、沙箱模式、批准策略、会话历史），`Claude` 15 个开关 + 2 个档位（输出风格、通知渠道），按五组 / 三组分组呈现（`common_groups` 为分组顺序唯一所有者）。契约变化：`ownership.rs` 新增 `ChoiceSpec`（键 / 标签 / 分组 / 控件 / 官方取值）与 `common_choices`；`common_toggles` 增加分组；通用补丁校验改为目录驱动（档位键必须是目录取值之一，开关键必须精确携带 applied 值，`BadReasoningEffort` 等三个旧错误合并为 `BadCommonValue`，三个取值常量迁入目录）。`model_reasoning_summary` / `model_verbosity` 循 `model_reasoning_effort` 先例从档案升格为通用设置：`CodexModelSettings` 收敛为仅上下文窗口，档案编辑器、适配器叠加与发现导入同步收敛，旧存储迁移把档案级三键（含冲突检测，冲突即停止）升格进通用补丁。命令层 `common_effort` 删除，新 `common_choices` 返回 `{ groups, choices }`（含文件真实值，超出目录的原样展示）。前端 `GeneralSettingsForm` 重写为分组渲染：复选行 + 通用滑块行 + 分段药丸（`radiogroup`、白凸起激活段 + 行动蓝描边、透明边框防布局跳动、`prefers-reduced-motion` 关闭过渡）。刻意不收录并在 `DESIGN.md` 记录边界：供应商路由 / 凭据、`MCP` / 权限 / 沙箱策略表、路径与自由文本键、宿主内部状态。`disable_response_storage` 在现行官方参考页已不见单列，但为用户指定项予以保留。验证：`cargo test --workspace`（126 项，独立 `CARGO_TARGET_DIR`）、`cargo fmt`、`npm run typecheck`、`npm test`（本轮相关 59 项中 58 过；`ProviderList` 两文件因用户并发引入未安装的 `@tauri-apps/plugin-opener` 无法收集、`BackupHistory` 缓存断言为并发改动，均非本轮引入）；未打包。

**代码预览统一组件（2026-08-28 完成）：** 按用户指令把所有代码预览模块统一为单一组件并全部接入：新 `src/components/CodePreview.tsx` 是代码预览渲染的唯一所有者，导出两个视图——`CodePreview`（文件视图：行号 + TOML/JSON 令牌着色 + 上一轮的换行行为）与 `DiffPreview`（差异视图：键 + 旧值 → 新值的分层着色，旧值弱化、箭头装饰、移除警示、新值承重，即用户当日加入的 `asb-diff-old/-arrow/-remove/-new` 样式语言）。接入点：通用设置的文件预览（原 `FileContentPreview` 删除，旧路径移除）、供应商页变更预览（`PreviewInspector` 内的差异列表）、备份历史的「查看差异」（`BackupHistory.DiffRow`）。消除的重复：`changeLine` 此前在 `PreviewInspector` 与 `BackupHistory` 逐字重复，两处差异列表标记各自维护且已分叉（单行拼接 vs 分层展开）——统一为展开式规范形态，`BackupHistory` 测试的整行正则断言随之改为分层断言；`PreviewInspector` 中用户已改的「保持不变的设置」措辞保留。新增 `CodePreview.test.tsx` 锁定两个视图的契约（行号 / 令牌类名 / 旧新值分层 / （无）与移除态）。验证：`npm run typecheck`、`npm test` 63/63 通过；未打包。

**编辑器搬运 CC Switch 模块（2026-08-28 完成）：** 按用户指令考察 CC Switch 参考项目（浅克隆 `github.com/farion1231/cc-switch` 至临时目录，MIT，仅作行为参考）后在供应商编辑器落地两项能力，其余模块因产品边界排除（本地代理 / 请求覆盖 / UA 伪装 / OAuth 托管 / 计费倍率 / 故障转移与端点自动选择 / 整文件 settingsConfig 编辑 / 第三方中继预设清单——分别违反「不做 LLM 代理、不做故障转移、凭据不归应用管、叠加层只拥有声明字段」等既定规则）。落地项：①本地元数据字段「官网地址」（可选 `http(s)` URL）与「备注」（可选，上限 500 字）——契约 `ProviderDraft` / `ProviderProfile` 新增 `websiteUrl` / `notes`（`serde default` 兼容旧存储），校验器拒绝非 `http(s)` 官网地址与超长备注，仅存本应用、绝不写入客户端配置；CC Switch 导入扫描随之读取并携带其 `website_url` / `notes` 列（3.x 架构必有）。②「获取模型」：主模型字段旁新按钮经新命令 `fetch_provider_models` 以 WinHTTP `GET` 拉取供应商的 OpenAI 兼容模型列表（路径规则：基址以 `/v{N}` 结尾拼 `models`，否则拼 `/v1/models`；`data[].id` 按序去重；非 2xx / 解析失败给平实错误文案；首版不携带凭据，需鉴权的端点会以 HTTP 错误反馈，可手填模型名），成功后出现下拉选择填充主模型；列表随服务地址变更即失效。探测面板的延迟毫秒显示 CC Switch 端点测速已覆盖，无需搬运。验证：`cargo test --workspace`（130 项，含 `models_path_for` 路径规则、模型 id 解析去重、官网地址 / 备注校验、CC Switch 元数据携带断言）、`npm run typecheck`、`npm test` 65/65（含编辑器获取模型填充与元数据提交用例）通过；按工作流本轮未打包，临时克隆已留在系统临时目录供后续参考。

**变更预览移入行内展开（2026-08-28 完成）：** 按用户指令，眼睛开关展开的变更预览从供应商面板底部移到**所展开行卡片内部、行线正下方**。结构：`asb-row-item` 卡片改为纵向两段——`asb-row-line`（原 68px 行内容，横向布局原样迁移）＋展开区；`ProviderList` 新增 `renderPreview` 渲染插槽，展开区内容由 App 注入（「变更预览」标题＋「确认切换」主按钮＋差异检查器，所有权仍在 App），列表只负责放置，展开区仅在该行 `previewOpen` 时渲染。展开区与行线以细分隔线分界、复用卡片自身表面（玻璃内不放玻璃）。展开行同时被拖拽时拖拽变换作用于整卡（含展开区），dnd 碰撞几何不受影响。设计意图登记 `DESIGN.md §8`。

验证证据：`npx tsc --noEmit` 零错误；`npm test` 66 项通过（含新增结构测试：展开区以 `.asb-row-line + .asb-preview-inline` 相邻选择器断言挂在所展开行卡片内、且全列表仅渲染一份预览内容；既有眼睛开关与 App 集成测试全过）。

**官方登录模式彻底移除（2026-08-28 完成）：** 按用户指令把官方登录从供应商档案契约中整体删除（上一轮仅移除编辑器入口）。契约：`ProviderDraft` / `ProviderProfile` 删除 `mode` 字段——所有档案一律为自定义服务（`RouteMode` 枚举保留，唯一消费者是 `RouteState.route_mode`，即对真实配置文件「当前是否官方登录」的只读观察，概览与发现页显示不变）。同一改动内同步收紧：校验器删除官方分支与 `OfficialForbidsBaseUrl` / `OfficialForbidsEnvKey` 错误（服务地址缺失一律拒绝，两客户端一致）；两个适配器删除官方渲染臂（不再支持「切换到官方」，Claude 端点恒为声明式、Codex 管理表恒为自定义表）；发现页导入——Claude 无自定义端点的配置不再产生导入建议（`claude_import_is_supported` 收紧为「有服务地址」）；CC Switch 导入——官方行（Codex OAuth/官方、Claude 无端点）从「导入为官方档案」改为跳过并说明原因。存储迁移：`migrate_store` 剔除存量官方档案（含 mode 缺失且无服务地址的前代记录），首次剔除前把原始 `profiles.json` 备份为 `profiles.json.pre-official-removal.bak`，随后单向写回无 `mode` 形态；官方档案的通用配置若仍带档案级键且无宿主档案则响亮报错保留原文件（新增两条迁移测试锁定）。真实存储检查：仅 1 条官方档案（`OpenAI官方订阅`，无路由数据）、通用段仅通用键，可平滑剔除。顺带修复：`StoredProfile` 补 `notes` / `website_url` 透传（上一轮遗漏会导致元数据在重启后丢失）。前端：类型、编辑器草稿、供应商行明细（移除官方标签分支）、CC Switch 扫描元信息行同步；全部夹具改为无 `mode` 形态。验证：`cargo test --workspace`（130 项）、`npm test` 66/66、`npm run typecheck` 通过；未打包。

**选中行动态渐变与按钮变色（2026-08-28 完成）：** 按用户指令修复选中行三处视觉问题并移植 spiralcoder 全局渐变色系（参考源码 `F:\projects\spiralcoder\web\src\styles\{tokens,accent-border,buttons}.css`，用户明确指示搬运色系）：①中心扩散移除——旧选中表面是半透明 `rgba(action,0.08)`，边框区的 conic 渐变透过卡面显形为中心射线（用户截图标记的缺陷）；移植 spiralcoder 的做法把选中表面改为不透明 `color-mix`（亮 8% 混白、暗 16% 混深底），渐变只存在于边框环。②四色环系统替换旧三段蓝环：环停唯一所有者 `tokens.css` 新令牌——`--asb-accent-ring-{blue,violet,mauve,gold}`（`#046aff/#7f66f8/#c39cad/#ffe6b6`，0°/108°/216°/320°/360° 五停闭合），分静止档（`-stops-rest`，`color-mix` 半透明）与强调档（`-stops-active`，全色）；选中行由静态描边升级为静止档 6s 流动，生效行升级为强调档 4s 流动——强度与动效共同标记命中行；`prefers-reduced-motion` 下两档均静止；旧 `--asb-accent-border-paint` 令牌（明暗两处）删除，无残留消费点。③选中行内「启用」主按钮换装移植的按钮渐变 `--asb-gradient-button`（青→蓝→粉 88° 五停，明暗同值）＋内嵌蓝光晕（悬停叠加白光增强）——实心行动蓝在蓝调选中表面上同系相融（用户截图第二缺陷）；作用域限定 `.asb-row-item.is-selected .asb-btn-primary`，其余主按钮（确认切换、安全切换、`+` 新建）保持行动蓝。全部新色值仅落 `tokens.css`（令牌所有权测试约束），`DESIGN.md §8` 已登记设计意图与不透明表面的原因。验证：`npm run typecheck`、`npm test` 66 项、`npm run build` 通过；未打包。

**获取模型列表修复（2026-08-28 完成）：** 按用户指令修复供应商编辑器「获取模型」必然失败的问题。两层根因：①桥接参数错位——`client.ts` 以 `baseUrl` 键传参而 Rust 命令参数名为 `url`，Tauri 按名匹配，点击即在调用桥报错，根本到不了 HTTP 层（前端 mock 与 Rust 直连测试各自绕过了桥接，两层测试都没拦住）；②凭据缺失——实测用户中转站对 `/v1/models` 返回 401 `API_KEY_REQUIRED`，首版「不带凭据」设计在所有鉴权端点上必然失败。修复：参数名对齐为 `{ url, app, envKey }` 并以测试锁定；命令层新增 `app` / `env_key` 参数，在点击时从用户自有配置解析密钥——Codex 读档案 `env_key` 指向的进程环境变量，Claude 读真实 `~/.claude/settings.json` 的 `env` 块（`ANTHROPIC_AUTH_TOKEN` 优先，空值视为缺失）再回退进程环境变量；密钥只进请求头（`Authorization: Bearer` + `x-api-key` 双生态 + `anthropic-version`），绝不存储、记录或回显；401/403 文案区分「密钥被拒」与「未找到密钥（指明来源）」。解析逻辑为纯函数（注入环境与文件读取器），唯一所有者 `probe::{codex_credential, claude_credential, claude_token_from_settings, auth_headers}`。验证：`cargo test --workspace` 132 项（独立 `CARGO_TARGET_DIR`，用户 dev 实例占用主 target；含 4 条新凭据测试）、`npm test` 78/78（含参数名锁定与 Codex envKey 传递两条）、`npm run typecheck` 对本改动零错误（剩余 2 错误来自用户并行会话新建的 `mock-dev-entry.tsx` 进行中文件）；curl 实测确认端点鉴权要求；未打包。同日文案统一（用户指令）：「环境变量名」字段与全部相关错误文案改为「密钥」口径——编辑器标签「密钥环境变量名」、校验文案（`InvalidEnvKey` / `EnvKeyLooksLikeSecret` / `ClaudeEnvKeyUnsupported`，后者说明密钥保存在环境变量、不进本应用）、获取模型失败的来源提示；代码标识符 `env_key` 与存储契约不变（字段存的仍是变量名，不是密钥本身）。同日可用性补充（用户提问「密钥在哪里填写」暴露说明缺失）：编辑器「密钥环境变量名」标签悬停 Tooltip 讲清密钥值的去向——Windows 用户环境变量（与本字段同名的变量），Codex 运行时自读、本应用不保存；Claude 侧无此字段（密钥由 Claude Code 自身管理）。验证：`npm run typecheck`、`npx vitest run` 全量通过。

**下拉选择复用组件（2026-08-28 完成）：** 按用户指令复刻 spiralcoder 的下拉选择菜单并统一接入：新 `src/components/Select.tsx` 是下拉选择的唯一所有者（`Radix Select`：字段形触发器 + 右侧弱化 chevron、Portal 玻璃菜单浮层、当前项右置勾选指示、上下滚动钮；触发器与 `asb-input` 同语言，浮层走玻璃菜单材质，`220ms` 淡入缩放进入，值全部来自令牌，`tokens.css` 新增 `--asb-glass-fallback-menu` 实心后备）。接入点：供应商编辑器「客户端」与「获取模型」后的「选择模型」两处原生 `<select>` 全量替换，行为保持（切换客户端清空模型参数；模型可经「（从获取列表选择）」清除——Radix 拒绝空串选项值，以 `MODEL_NONE` 哨兵映射回 null）；通用设置页为分段药丸 / 滑块，无下拉，不涉及。测试基建：`src/test/setup.ts` 补 jsdom 缺失的指针捕获与 `scrollIntoView` 存根。设计意图登记 `DESIGN.md §8`。验证：`npm run typecheck`、`npm test` 77/77（含新增 `Select.test.tsx` 四项：占位符与选中标签 / 选项点击回调 / 勾选指示 / 禁用不展开；`ProviderEditor` 模型选择用例改为触发器 + 选项点击路径）；未打包。

**输入框与文本域复用组件（2026-08-28 完成）：** 按用户指令复刻 spiralcoder 的输入框（`web/src/components/ui/{input,textarea}.tsx`）并统一接入：新 `src/components/Input.tsx` 与 `Textarea.tsx` 是文本控件的唯一所有者——原生属性（`type` / `required` / `placeholder` / `rows` / `min` / 无障碍属性等）全量透传，字段外观类由组件内部持有，消费方不再自写 `className`；spiralcoder 的 `appearance` 三态与 `controlSize` 四档按「无消费者即不实现」裁剪，仅保留本系统实际使用的代码变体（`code` 布尔 → `asb-code` 等宽）。`.asb-input` 补齐 spiralcoder 字段契约的缺失状态（此前无任何交互态）：placeholder 弱化色、悬停行动蓝描边、`focus-visible` 焦点环（替换浏览器默认 outline，与复选框 / 选择触发器同语言）、禁用 0.45 透明 + 默认光标、120ms 描边与阴影过渡（`prefers-reduced-motion` 经令牌归零）——值全部来自 `tokens.css`。接入点：供应商编辑器全量 10 个 `<input>`（名称 / 官网地址 / 服务地址 / 主模型 / 环境变量名 / 上下文窗口 / 三档模型映射）与 2 个 `<textarea>`（备注 / 可选模型列表）；通用设置为复选框 / 滑块 / 分段药丸，无文本输入，不涉及。验证：`npm run typecheck`、`npx vitest run` Input / Textarea / ProviderEditor 三文件 15 项通过（含新增：原生属性透传可及树 / 输入回调 / 禁用不可输入 / code 变体类名；既有编辑器行为用例不受影响）；未打包。

**窗口控制改走 Win32 系统命令（2026-08-28 完成）：** 按用户指令把标题栏三钮（最小化 / 最大化-还原 / 关闭）的执行路径从 tauri 窗口抽象（`window.minimize()/maximize()/close()`）改为 Windows 原生实现，前端样式零改动。背景：三钮本就是自绘（无装饰窗口 + webview 内 SVG 按钮），此前仅点击动作经 tauri 转发 `ShowWindow`。实现：`commands.rs` 四个窗口命令统一经 `windows-sys` 走 `SendMessageW(WM_SYSCOMMAND, SC_*)`——与系统标题栏按钮同一通道（`SC_MINIMIZE` / 最大化态经 `IsZoomed` 判定后发 `SC_MAXIMIZE` 或 `SC_RESTORE` / `SC_CLOSE`，`SC_CLOSE` 经 `WM_CLOSE` 仍进 tauri 的 close-requested 流程）；`window_is_maximized` 同步改用原生 `IsZoomed`；HWND 取自 `window.hwnd()`（`windows` crate 句柄与 `windows-sys` 同为 `*mut c_void`，直接透传）。`Cargo.toml` 的 `windows-sys` 增加 `Win32_UI_WindowsAndMessaging` 特性。前端 `WindowControls` / api client / 命令签名均未动。架构定案（用户三选一拍板「维持现状」）：自绘按钮 + 系统命令通道即最终形态——真原生标题栏（顶部加系统栏、纯色不能磨砂）与 WebView2 Window Controls Overlay（理想形态但 SDK prerelease、wry 不支持 wry#1650、fork 即技术债）均被否；悬停最大化不弹 Snap Layouts 为 WebView2 平台限制，已知且接受，决策登记 `MEMORY.md`。验证：独立 `CARGO_TARGET_DIR` 下 `cargo check` 通过（本轮窗口命令区域零警告零错误）；全量 `cargo test --workspace` 因用户并发进行中的 `fetch_models` 凭据重构（`probe.rs` / `commands.rs` 测试段编译错误）暂无法收集，非本轮引入，待该改动落地后随跑；未打包。

**供应商 API 密钥档案化（2026-08-28 完成，用户明确选择明文 `profiles.json`）：** 彻底替换旧的 `envKey` / 环境变量凭据模型，无兼容、无迁移、无回退。新唯一契约：`ProviderProfile` / `ProviderDraft` 的必填 `apiKey: String`（`serde deny_unknown_fields`），编辑器中 Codex / Claude 均有必填密码输入「API 密钥」；空白或超过 4096 字符拒绝。`state/profiles.json` 是唯一允许明文持久化档案 API key 的应用文件；旧文件携带 `envKey` 将明确报「来自已不受支持的旧版本」且逐字节不改写（隔离测试锁定）。切换映射：Codex 写受管顶层 `experimental_bearer_token`，Claude 写受管 `env.ANTHROPIC_AUTH_TOKEN`，两键加入各自所有权表与 profile-exclusive 表，通用设置不可写；每次写仍完整经过锁、预览、备份、临时写、校验、原子替换、写后验证与撤回。模型获取命令改为只接收当前编辑器 `apiKey`（`{url, apiKey}`），不再读取 Windows 环境变量或 Claude settings。导入恢复为直接可用：CC Switch Codex 从 `auth.OPENAI_API_KEY`、Claude 从 `env.ANTHROPIC_AUTH_TOKEN`（其次 `ANTHROPIC_API_KEY`）携带 key；本机发现从 Codex `experimental_bearer_token` / Claude env 生成导入草案；key 仅存在导入 draft，不进入 RouteState、告警或诊断。安全边界同时加强：`ProviderProfile` / `ProviderDraft` 自定义 `Debug` 固定将 key 显示为 `••••••••`；`FilePreview.content` 由「原始候选文本」改为展示专用脱敏副本，执行器内部原文仍用于 `rendered_hash`、校验与原子写入；TOML/JSON 按敏感键路径遮罩，包括宿主已有与新写入 bearer/token/api_key。差异、错误、日志与诊断继续只见遮罩。`AGENTS.md`、README、DESIGN.md 的旧“绝不保存明文 key / Codex env_key / Claude 自管”说法同轮删除替换。验证：`cargo fmt --all -- --check`、隔离 `CARGO_TARGET_DIR` 的 `cargo test --workspace` 136/136、`npm run typecheck`、`npx vitest run` 90/90、`npm run build` 均通过；未打包。

**变更预览接入优质打印全文（2026-08-28 完成）：** 按用户指令把优质打印模块接入供应商变更预览，替换真实配置下不可读的「保持不变的设置」扁平键列表（Codex 宿主键可达 90+ 个）。契约迁移（单一所有者 = `asb-switch::executor::FilePreview`）：`FilePreview` 新增 `content` 字段——`read_preview_for` 把候选渲染一次，`rendered_hash` 与界面展示共用同一文本；`preview_switch` 与 `preview_common` 由此都携带全文。同一改动删除旧路径：`commands.rs` 的 `CommonPreview` 包装结构与其重复的读取＋`common_render` 渲染段整体移除，`preview_common` 直接返回 `FilePreview`（`preview.target` 回填与 `preview_switch` 对齐）；前端 `CommonPreview` 接口删除。界面：`PreviewInspector` 的「保持不变的设置」键值行与顶部路径 h2 移除（路径由 `CodePreview` 文件条显示，避免重复），在差异视图与警告之后渲染 `CodePreview` 候选全文——被保留的宿主键原位可见，`320px` 限高滚动不撑爆行内展开区。安全不变性以隔离测试锁定：执行后落盘文件与 `fp.content` 逐字节相等。验证：`cargo test -p asb-switch`（3 + 16 项含新断言）、`npm run typecheck`（仅剩用户并发 `mock-dev-entry.tsx` 的既有脚本作用域错误，非本轮引入）、`npm test` 78/78（`PreviewInspector` 新增候选全文原位断言；`fetch_provider_models` 新签名断言由用户会话同步更新）；`cargo test --workspace` 暂被用户进行中的凭据改动（`claude_credential` 可见性）阻塞，与本轮无关；未打包。

**扁平列表全量清查与旧契约删除（2026-08-28 完成）：** 按用户指令复查全部列表渲染面并完成优质打印迁移收尾。清查结论：唯一把配置键平铺成顿号列表的就是上一轮已替换的变更预览「保持不变的设置」；其余列表均非键堆——概览配置状态与发现页为结构化状态行（路由 / 匹配 / 警告消息），CC Switch 扫描为复选行＋单行 meta，操作警告为消息 `<ul>`，确认弹窗为单行事实，备份差异已走 `DiffPreview`，通用设置与供应商变更预览已走 `CodePreview` 全文，均维持现状。随之删除旧契约（前端已零消费）：`SwitchPreview.preserved` 字段从 Rust 契约与 TS 接口整体移除，两适配器的 `collect_preserved` 收集逻辑删除，宿主键安全性改由既有的渲染级测试承载（codex `render_preserves_host_content_and_comments`、claude `render_updates_owned_keys_and_keeps_host_blocks`），两个 preview 测试改名并删除 preserved 断言；前端 fixtures 同步。用户未跟踪的 `mock-dev-entry.tsx` 中残留的 `preserved: []` 为无类型 mock 数据，运行时无害，因该文件正被并发会话编辑而未触碰。验证：`cargo test --workspace`（132 项全过，含 asb-core 86 / asb-switch 19）、`npm test` 78/78、`cargo fmt --check` 本轮改动文件零差异（现存 diff 均为用户并发会话文件）；typecheck 仅剩 `mock-dev-entry.tsx` 两个既有脚本作用域错误（非本轮引入）；未打包。

**通用设置勾选导致界面偏移——实测定位与结构修复（2026-08-28 完成）：** 用户报告通用设置页勾选 / 取消勾选导致界面整体偏移。为避免凭读码猜测，搭了临时实测环境（`mock-dev` 入口给真实前端装 stateful invoke 桩 + 完整目录数据 + 真实延迟，Playwright + 系统 Edge 以 PerformanceObserver `layout-shift` 实测点击前后几何），定位出两类真实位移源：其一，操作横幅（错误 / 警告 / 切换结果）渲染在 `.asb-main` 滚动流顶部，其插拔把下方内容整体推移约一行横幅高度（实测 `scrollTop` 跳变 +64px，仅靠 Chromium 滚动锚定掩盖）；其二，配置预览体 `max-height` 随文件行数增长、滚动条槽位出现引发重排（边界几何实测位移分 0.059）。结构修复（无兼容层、无过渡方案）：`App.tsx` 新增 `.asb-workspace` 包裹层，三横幅移出滚动流、装入 `.asb-banner-stack` 绝对定位浮层（`pointer-events: none`、模态投影、近实心底）；`.asb-main` 补 `flex: 1; min-height: 0` 并加 `scrollbar-gutter: stable`；`.asb-filepreview-body` 由 `max-height` 改为固定 `height: 320px` 并加 `scrollbar-gutter: stable`。实测复验全部归零：成功路径（勾选 / 取消 / 分段选择 / 恢复默认，Codex 与 Claude）、错误注入、警告注入、内容恰好充满的边界视口，位移分全部 `0.00000`，错误 / 警告场景 `scrollTop` 不再跳变。脚手架（mock 入口、探针脚本、playwright-core 依赖、vite 实例）已全部删除，`package.json` 仅余用户既有依赖改动。验证：`npm run typecheck`、`npm test` 78/78 通过；未打包。

**差异视图拆分为全局 DiffView 并改红绿语义（2026-08-28 完成）：** 按用户指令把差异渲染实现为独立全局通用组件并统一红绿色设计。拆分：新 `src/components/DiffView.tsx` 是变更行差异渲染的唯一所有者（键 + 旧值 → 新值，等宽字体、近实心底）；`CodePreview.tsx` 收敛为仅文件视图（`DiffPreview` 导出删除）。红绿设计：旧值＝红字 + 浅红 tint 芯片（复用 `--asb-danger`），新值＝绿字 + 浅绿 tint 芯片（新令牌 `--asb-diff-add-text`），移除＝红色加粗「移除」芯片，箭头与文字保证不只靠颜色可读；旧值缺失「（无）」改中性弱化（新 `asb-diff-none`，无旧值即无移除、不染红）。令牌新增明暗两套（`--asb-diff-add-text` / `--asb-diff-remove-bg` / `--asb-diff-add-bg`，移除文字直接走模式自适应的 `--asb-danger`）。接入点：供应商变更预览（`PreviewInspector`）与备份历史「查看差异」（`BackupHistory`）统一改用 `DiffView`。测试：差异用例迁至 `DiffView.test.tsx` 并按红绿语义更新断言（旧值红类、新值绿类、「（无）」中性类），`CodePreview.test.tsx` 收敛为文件视图用例。验证：`npm run typecheck`（除用户 `mock-dev-entry.tsx` 既有错误外零错误）、`npm test` 78/78；Playwright 临时 harness 截图经视觉核验——浅色与深色模式下红 / 绿芯片、红色「移除」、中性「（无）」、弱化键名全部清晰可读，无对比度问题，临时产物已清理；未打包。

**选择框点击的外层蓝环移除（2026-08-28 完成）：** 用户报告所有选择框点击后被一层额外蓝色边框包裹。根因：复选框、单选、滑杆这类输入控件被鼠标点击获得焦点时，Chromium 对其原生恒定匹配 `:focus-visible`（与按钮不同），于是绘制控件（复选框盒、分段药丸、滑块白钮）每次点击都套上焦点环。契约要求键盘焦点必须可见，不能整体删除——改为按输入模式区分：`App.tsx` 新增输入模式跟踪（Tab / 空格 / 方向键按下时在根元素标记 `data-focus-source="key"`，任何指针按下即清除），`base.css` 中三条绘制控件焦点环选择器全部以 `:root[data-focus-source="key"]` 前置门控（复选框盒、分段药丸、滑块钮）。效果：鼠标点击无环；Tab / 方向键 / 空格导航时环照常显示；表单文本输入的点击焦点环不受影响（标准表单行为）。新增 `App.test` 用例锁定标记生命周期（键盘标记、指针清除）。验证：`npm run typecheck`、`npm test` 79/79 通过；未打包。

**全会话「无兼容无过渡无技术债」审计与清债（2026-08-28 完成）：** 用户确立全会话准则后，对本会话全部改动逐项审计。rg 全仓扫描确认：旧类名（`asb-effort-*`）、重复助手（`changeLine`）、前端档位常量（`EFFORT_DETENTS`）、已删契约符号（`common_effort`/`EffortState`/三个取值常量/三个旧错误变体）、实测脚手架（mock 入口、探针脚本、playwright-core）全部清零。唯一真实残留为 `local_state.rs` 的旧数据升格迁移——`StoredCodexOptions` 三个遗留字段、`LEGACY_CODEX_COMMON_KEYS` 表、`legacy_codex_line` 与升格循环、两个迁移测试：其唯一服务对象（在盘 profiles.json）已被该迁移重写，代码已成死过渡路径，整体删除；解析边界同步收紧为 `deny_unknown_fields`（含 serde tag 字段声明），旧形态存储从此响亮报错「旧 Codex 档案模型选项无法解析」而非静默吞字段。既有更早年代的迁移（模式推断、档案级键降格、弃用 Claude 键）非本会话产物，未动。验证：`cargo test --workspace`（130 项）、`cargo fmt`、`npm run typecheck`、`npm test` 84/84 通过（期间偶发失败均为用户并发编辑的瞬时态）；未打包。准则已写入长期记忆。

**备份页新增「打开备份文件夹」（2026-08-28 完成）：** 按用户指令在备份历史面板标题行新增「打开备份文件夹」按钮（与「撤回上一次切换」同入 `asb-panel-actions` 分组，常驻可点、不参与 busy 门控——只读导航动作；记录为空时按钮仍在，可直达目录）。契约：后端新命令 `open_backup_dir` 是该操作的唯一入口——解析 `LocalState::backup_dir()`（应用数据目录 `state/backups`）、按需 `create_dir_all`（空历史也能打开）、经 `tauri-plugin-opener` 的 Rust API（`OpenerExt::open_path`，签名收 `Into<String>`）唤起系统资源管理器；从 Rust 侧调用而非前端 `openPath`，故无需放宽 opener capability 的路径 scope，现有 `opener:allow-open-url` 保持仅 http/https 不变。前端 `client.ts` 新增 `openBackupDir()` 类型化封装，失败经统一 `setError` 呈现类型化错误。`DESIGN.md §8` 操作表同步登记。验证：`cargo check` 通过（仅余 `local_state.rs` `kind` 字段既有 dead_code 警告，非本轮引入；构建期间运行中的 dev 实例占用资源致 os error 32，按既定流程 `taskkill` 后编译通过）；`npx vitest run src/App.test.tsx` 12/12 全量连跑三次稳定（含新增「按钮点击即调 `open_backup_dir`」用例）；`npm run typecheck` 对本轮三个文件零错误（`ConfirmSheet.tsx` 的 key 类型错误为用户并行改动既有，非本轮引入）；未打包。

**时间展示统一组件与探针状态色（2026-08-28 完成）：** 按用户指令三件事：①端点验证结果行以颜色承载结论——可达＝安全绿、不可达＝警示红（新类 `asb-ok-text` / `asb-fail-text`，取既有令牌 `--asb-safe` / `--asb-danger`；文字「可达 · HTTP 状态 · 延迟」/「不可达」同时在场所保不只靠颜色可读）。②时间格式去掉 `（UTC+8）` 偏移后缀——`timeLabel` 只输出 `YYYY-MM-DD HH:MM:SS` 本地时区，`offsetLabel` 助手删除；`vite.config.ts` 的 `TZ=Asia/Shanghai` 测试锚定不变。③时间展示收敛为全局组件：新 `src/components/Time.tsx`（`<time dateTime>` 语义元素）是界面时间渲染的唯一所有者，`timeLabel` 降为格式化纯函数；全部消费点统一接入——概览匹配态三例（`matchLabel` 返回类型放宽为 `ReactNode`）、概览上次切换、备份页上次操作、备份历史表格与恢复确认、探针面板、撤回确认（`ConfirmSheet` 的 `details` 类型放宽为 `ReactNode[]`，键改用索引）。新 `Time.test.tsx` 两条契约：渲染无后缀本地时间 + 扫描测试锁定 `timeLabel` 在 `lib/time.ts` 与组件之外零直接消费者（复用 boundary 测试扫描模式）。验证：`npm run typecheck` 零错误、`npx vitest run` 84/84（20 文件）；未打包。

**开发入口单一化——根治 HMR 窗口闪烁（2026-08-28 完成）：** 用户报告「每次热更新都 build、窗口闪」。`tauri dev --verbose` 实测定位根因：非 `npm run build` 参与热更——Tauri Rust watcher 监听 `src-tauri/`、`crates/`，并行改动 `src-tauri\src\*.rs` 时被判定 `Rebuilding application...` 并重启原生窗口（闪烁即窗口销毁重建，是编译型宿主的固有行为，无热加载可言）。先尝试 `--no-watch` 形态，经用户权衡拍板回到 watcher 模式（A）：Rust 改动自动重建重启，前端改动走 Vite HMR 不闪。无兼容收敛：唯一开发入口 `npm run dev` = `CARGO_TARGET_DIR=target-dev` + `tauri dev`（带 watcher；`target-dev` 与并行实例的默认 `target` 隔离，规避 os error 32 文件锁）；Vite 服务收敛为内部脚本 `dev:frontend`，仅由 `beforeDevCommand` 消费、不是用户入口；本轮临时引入的 `dev:tauri` 别名与旧的独立 `vite` 型 `dev`、历史 `npm run tauri dev` 用法同轮删除，全仓 rg 零残留。`target-dev/` 入 `.gitignore`；`MEMORY.md` 开发机调试条目同步改写。验证：新入口真实启动成功——日志含 `Watching src-tauri / crates/*`、9 秒增量编译后 exe 自 `target-dev\debug` 运行、期间前端 HMR 更新正常送达；`npm run typecheck` 通过；未打包。

**「撤回上一次切换」按钮改红色填充（2026-08-28 完成）：** 按用户指令把备份页标题行的撤回按钮从 `asb-btn-secondary` 换为既有的 `asb-btn-danger`（`--asb-danger` 填充 + `--asb-text-on-action` 白字，明暗模式自适应，该类原唯一消费者是确认弹窗的破坏性确认钮）。`DESIGN.md §8` 操作表新增撤回行，并修正「删除提供商」行的「仅在确认流程中」表述以匹配红色现在同样出现在撤回入口的事实。验证：`npx vitest run src/App.test.tsx` 12/12（撤回流程用例按名称定位按钮、不受类名影响）；`npm run typecheck` 零错误（上一轮 `ConfirmSheet.tsx` 的用户并行错误已被其会话修复）；未打包。

**旧供应商存储严格重置（2026-08-28 完成）：** 实测应用自有 `state/profiles.json` 含 7 条 `envKey` 旧档案；当前 `ProviderProfile` 要求必填 `apiKey` 且拒绝未知字段，旧内容无法无损转换，因此没有加入兼容解析、迁移或回退。`LocalState::load_store` 改为类型化 `ProfileStoreError`（`Unreadable` / `Unsupported`），Tauri 统一映射为 `store-unreadable` / `profile-store-unsupported`；新增 `reset_profile_store(confirm_write)` 命令，复用既有临时文件 + 原子重命名，将仅应用自有 `profiles.json` 替换为空的当前 `ProfileStore`，不触碰应用设置、备份、Codex / Claude 配置或凭据。错误横幅仅在专用错误码下显示确认入口，取消不写入，确认后明确清空档案、通用覆盖与切换记录并刷新界面。旧格式仍被严格拒绝，重置后才可按当前契约重新创建档案。同轮无兼容审计删净过渡残留：`ProfileStore.switch_log` 与 `SwitchLog.operation` 的 `serde(default)`（连同仅为其存在的 `SwitchOp::Default` 实现）整体移除——缺字段的存储一律 `Unsupported` 并进入重置路径，不再静默补默认值（新测试 `incomplete_store_shapes_are_unsupported_not_silently_defaulted` 锁定）；`ProviderProfile` / `ProviderDraft` 六处 Option 字段冗余的 `default` 属性删除（serde 对缺失 Option 本就解析为 None，行为不变）；本轮引入的 `From<ProfileStoreError> for String` 桥接删除，`preview_common` 的 `get_common` 失败统一走 `store_error` 映射（同一文件同一条件同一错误码）。`BackupRecord.target_existed` 的 `default = "backup_target_existed"` 保留未动：它守护磁盘上真实旧备份旁车的可读性，删除会使既有备份历史不可列出，且备份无重置路径承接——若后续要收紧需先设计备份的显式恢复路径。验证：`CARGO_TARGET_DIR=target-check cargo test --workspace` 全部通过、`cargo fmt --all -- --check`、`npm run typecheck` 通过（本轮未改前端逻辑，前端 21/21 见前）；未打包。当前用户的旧 `profiles.json` 未自动清空，需在界面点击“清空旧档案并重新开始”并二次确认后才执行。

**重置按钮不可点击修复（2026-08-31）：** 用户报告「清空旧档案并重新开始」点不了。根因：按钮位于错误横幅内，而横幅层在 2026-08-28 布局位移修复中被定为 `pointer-events: none` 纯信息浮层，按钮继承了不可点击；jsdom 不加载 CSS，故既有测试全绿而真机失效。修复：`base.css` 新增 `.asb-banner > button { pointer-events: auto }`——横幅表面与文字区域保持穿透，仅横幅内操作控件恢复指针事件，布局位移契约（横幅不拦截内容点击、几何不变）不受影响。新增 `src/styles/banner.test.ts` 扫描测试锁定该 CSS 契约（横幅层保持 `none` 且横幅按钮必须 `auto`）；`DESIGN.md §7` 同步修正横幅指针事件表述。

**Windows 托盘与应用设置（2026-08-28 完成）：** 按用户批准方案新增 Windows 系统托盘与顶层「设置」页。原生层：`src-tauri/Cargo.toml` 显式启用 Tauri `tray-icon`；`src-tauri/src/tray.rs` 是唯一 tray 所有者。`lib.rs` 对 `main` 的 `CloseRequested` 按应用设置处理：默认 `hideToTray` 会 prevent_close + hide，`exit` 放行；同一策略覆盖自绘关闭钮、Alt+F4 与任务栏关闭。当前托盘行为见「托盘交互定稿」条目。

应用偏好契约：`local_state.rs::AppSettings` 是唯一所有者，独立原子保存于 app-data `state/settings.json`，绝不进入 `asb-core` 客户端 ownership / `CommonConfigPatch` / `profiles.json`，缺失文件返回完整默认值且不创建文件；严格 `deny_unknown_fields`，未知字段响亮拒绝。顶层导航新增「设置」页；`AppSettingsForm` 管理关闭行为、主题、动态效果、置顶与硬件加速，不显示配置行伪预览。主窗口 focus 时调用现有 `refresh`，界面重取真实状态。

**会话管理（2026-08-28 完成）：** 阶段 07 增加只读会话浏览子目标，范围严格限定为 `Codex` 与 `Claude Code` 的本地 JSONL 会话记录。契约所有者为 `src-tauri/src/session_manager.rs`：扫描固定的用户目录、统一元数据、按客户端 + 会话 ID 重新定位详情；前端不会接收或传入源文件路径。界面目标为顶层「会话」Tab 的筛选列表、按需加载详情、恢复命令与工作目录复制，以及在新命令提示符窗口中实际恢复；不复制 CC Switch 的多客户端、缓存、删除或会话 / SQLite 改写路径。

**托盘可恢复性加固（2026-08-28 完成）：** 用户要求应用顶栏/托盘“必须常驻时刻存在，不能因为报错导致整个应用丢失”。`tray.rs` 的唯一 `TRAY_READY` 状态只在 TrayIconBuilder `.build()` 成功后置位；纯决策 `keep_alive(tray_ready, settings)` 保证没有已创建 tray 时绝不隐藏窗口，`settings.json` 不可读时若 tray 存在则保活/隐藏，用户选择 `exit` 时放行。`lib.rs` 的 `CloseRequested` 和 `RunEvent::ExitRequested` 统一通过该决策；面板的「退出应用」设置一次性 `EXPLICIT_EXIT` 标记后调用 `app.exit(0)`，事件守卫只放行这条明确退出，其余系统退出请求仍隐藏到 tray。新增 Rust 纯函数测试覆盖：设置读取失败仍保活、exit 策略放行、无 tray 禁止隐藏、显式退出标记一次性消费。前端 `AppErrorBoundary` 仅包工作区、不包顶栏/窗口控制：任一业务页面渲染异常时显示「界面未能加载」恢复面，原生 tray 和窗口控制保持可用。

**设置页扩展：外观与窗口（2026-08-28 完成，2026-08-31 更新）：** 按用户选择扩展顶层「设置」页的应用级通用配置（外观、窗口与性能），仍完全独立于 Codex / Claude Code 配置。`AppSettings` 严格完整契约为 `{ closeBehavior, theme, motion, alwaysOnTop, hardwareAcceleration }`：`ThemePreference = system/light/dark`，`MotionPreference = system/reduce`，默认 `hideToTray + system + system + false + true`。为保留实际存在的上一版完整四字段业务数据，首次读取时仅将恰好为该形态的 `state/settings.json` 原子升级为当前对象并写入 `hardwareAcceleration: true`；这不是持续双读，升级成功后磁盘只保留当前契约。旧仅含关闭策略的设置文件、未知字段、非法枚举值一律明确报「应用设置格式无效」，不补字段、不静默改写。原子写入不触碰 profiles、备份或任何客户端配置路径。

即时行为：前端在完整设置成功加载/保存后将 `data-theme` 与 `data-motion` 应用到根元素；`system` 时移除属性。`tokens.css` 是唯一视觉值所有者：显式 light 屏蔽系统 dark，显式 dark 覆盖系统 light，`reduce` 将现有 press/fast/node/sheet 动效时长归零。`AppSettingsForm` 新增「外观」分段（界面主题、动态效果）、「窗口与托盘」中的「窗口始终置顶」复选框，以及「性能」中的「启用硬件加速」复选框。后端 `set_app_settings` 先调用 main window 的 `set_always_on_top`，失败不落盘，避免保存但未生效；硬件加速的写入只在下一次完整启动、WebView2 创建前由启动路径映射为浏览器参数，关闭时在 Wry 默认参数上追加 `--disable-gpu`，已有显式浏览器参数则保留并追加。启动时读取有效完整设置并恢复置顶，设置损坏时保持 tray/window 可恢复外壳并继续采用 GPU 默认行为，严格错误由设置 UI 呈现。App 保存每项时合并完整对象，失败不改变已应用的主题/动效，测试锁定完整提交与失败保持视觉状态。

验证：`cargo fmt --check`、`cargo test --workspace`（全部通过）、`npm run typecheck`、`npm test` 23 文件 98 项全部通过；临时 `target-check` 已清理；README、DESIGN.md 已同步；未打包。

**托盘交互定稿：左键主窗口 + 右键原生菜单（2026-08-28 完成）：** 用户最终选定：左键单击托盘图标恢复并聚焦主窗口，右键弹出 Windows 原生菜单；此前的独立 `tray` WebView 面板整体删除。`tray.rs` 是唯一托盘所有者：菜单为 Agent Switchboard 自有结构——「打开主界面」、两条禁用的 Claude / Codex 当前路由状态（经 `config_status` 读取真实文件，绝不作为未确认切换入口）、分隔线与显式「退出」（`app.exit(0)` 经一次性 `EXPLICIT_EXIT` 标记放行）；`show_menu_on_left_click(false)` 保证左键不弹菜单；写操作成功后经 `tray::refresh` 刷新菜单状态文本。`tauri.conf.json` 删除 `tray` 窗口声明，`lib.rs` 删除面板窗口事件分支与 `show_main_window` / `hide_current_window` / `exit_app` 命令注册；前端删除 `TrayPanel` 组件、测试与 `isTrayWindow` / `showMainWindow` / `hideCurrentWindow` / `exitApp` 封装及对应断言，`main.tsx` 恢复只渲染 `App`，`base.css` 删除面板样式块。`TRAY_READY` 可恢复性与 `keep_alive` 决策不变：无 tray 绝不隐藏窗口，设置不可读仍保活。验证：`cargo test --workspace`（139 项通过；src-tauri 32 passed、1 ignored）、`npm test`（23 文件、102 项）、`npm run typecheck`、`npm run build` 均通过；未打包。

**供应商行「启用」主按钮选中门控（2026-08-31 完成）：** 按用户指令，「启用」主按钮默认不展示，仅在该行被选中且非生效时渲染——`ProviderList.tsx` 的展示条件由 `onActivate && !active` 收紧为 `onActivate && selected && !active`。无兼容清债：①`base.css` 删除 `.asb-row-item.is-selected .asb-btn-primary` 两条规则的 `.is-selected` 前缀——该前缀属旧设计（按钮常驻全部非生效行、选中行换装渐变），按钮现仅存在于选中行，前缀恒成立即死条件；合并为行内主按钮恒为移植渐变身份，注释同步改写前提。②`DESIGN.md §8` 交互契约改为「默认不展示，仅行被选中且非生效时出现在动作簇左侧」，并删除已无对应形态的「实心行动蓝」描述。③测试按新契约重写：未选中时列表无任何启用按钮；选中非生效行显示并路由变更预览流；生效行即使选中也不显示（原「非生效行均显示」用例删除）。验证：`npm run typecheck`、`npm test` 23 文件 102 项全部通过；未打包。

**Tab 连续点击卡死：请求生命周期无债修复（2026-08-31 完成）：** 用户报告 Tab 按钮连续点击直接卡死，审查定位根因不在点击处理本身，而是三条读取链路在连续切换时叠加同步后端命令且旧回包无条件回写：通用设置 Tab 每次切换发起 `preview_common`（同步命令，整文件解析 + 渲染 + 双哈希）、会话页每次进入重挂载触发全量 JSONL 扫描、供应商预览在途请求晚到覆盖最新选择。按「无兼容无过渡无技术债」一轮收敛为统一契约：**同目标在途请求唯一化（复用），身份或内容变化即失效（丢弃回包）**。

实现要点：①`App.tsx` 引入 `requestSwitchPreview` / `requestCommonPreview`（在途 Promise 按目标缓存复用，完成自动出队）与两条失效路径——`retractPreview`（选择 / 客户端 Tab / 收起：仅撤展示，候选缓存仍有效）和 `invalidateCandidates`（切换、恢复、撤回、通用写入、档案增删改、导入、重置存储：清空全部候选缓存，因缓存 Promise 渲染的是写前文件）；全部写操作入口统一走 `invalidateCandidates`，消除散落的裸 `setPreview(null)`。②通用设置改用按客户端独立纪元（`commonEpoch`），替换全局版本号——全局号会在切换客户端时误丢另一客户端的写后回读（勾选 Codex 后切 Claude，Codex 的 toggle/choice/preview 刷新被静默丢弃）；`applyCommonLine` 写后回读三件套同纪元门控，且写前显式使该客户端预览缓存出队，杜绝复用写前候选。③`SessionManager` 改为必填 `active` 契约 + 常驻挂载（`hidden` 分节）：仅在激活时扫描、在途扫描被再次激活复用（乱纪元丢弃），从触发源消除「导航反复进出 → N 次全量扫描」的堆积；启动时不再扫描。④`WindowControls` 三命令补 `.catch`（连点不再产生未处理 rejection）；测试 mock 履约返回 Promise，组件侧不加 `Promise.resolve` 兜底。

回归测试（deferred 控制完成顺序）：通用设置 Tab 快速交替——`preview_common` 恰两次（codex 在途复用）且只显示当前客户端文件；供应商预览 A/B 乱序——B 先回显、A 晚到被丢弃不覆盖；会话未激活不扫描、激活即扫、反复激活共享同一在途扫描（`list_sessions` 恰一次）。验证：`npm run typecheck`、`npx vitest run` 23 文件 106 项全部通过；`rg` 确认兜底 shim、旧全局版本号与未门控回写路径零残留。已发出的 Tauri 同步命令无法被前端中止（平台约束），但其回包现被纪元严格丢弃；后端命令未改动。未打包。

**窗口控制改用 Windows 原生标题栏（2026-08-31 完成）：** 按用户指令（不要自己实现、用 Windows 原生、最新样式、无兼容无过渡无技术债），推翻 2026-08-28「自绘按钮 + 系统命令通道」定案：`tauri.conf.json` 删除 `decorations: false`（恢复默认 `true`），最小化 / 最大化-还原 / 关闭三钮由系统标题栏绘制（Win11 最新样式、Snap Layouts 悬停、系统菜单全套原生）。无兼容全量删除：前端 `WindowControls.tsx` 与测试、`icons.tsx` 四枚窗口图标、`client.ts` 五个封装（连同 `getCurrentWindow` 导入、`App.test.tsx` 的 `@tauri-apps/api/window` mock 与 `window_is_maximized` 分支）、`base.css` 窗口按钮样式块、顶栏三处 `data-tauri-drag-region` 与 `core:window:allow-start-dragging` 权限；Rust 侧 `window_minimize / window_toggle_maximize / window_is_maximized / window_close` 四命令与 `SendMessageW(WM_SYSCOMMAND)` / `IsZoomed` 通道、`windows-sys` 的 `Win32_UI_WindowsAndMessaging` 与 `Win32_Foundation` 特性同轮删除。新增原生标题栏与主题偏好的衔接：`apply_window_settings` 在置顶之外增加 `set_theme`（system → `None` 跟随系统，light / dark → 显式指定，失败码 `window-theme-failed` 先于落盘），顶栏降为应用内页头不再是拖拽区。关闭行为不变：系统关闭钮经 `WM_CLOSE` 仍进 `CloseRequested`，托盘吸收 / 显式退出决策照旧。再评估依据：wry WCO 仍无官方支持（wry#1650、tauri#4531），decorum 等插件按钮为 HTML 复刻而非系统绘制。验证：`cargo fmt --check`、隔离 `CARGO_TARGET_DIR` 下 `cargo test --workspace`（src-tauri 33 过 1 忽略、asb-core 86、执行器 5、切换 16 全过）、`npm run typecheck`、`npx vitest run` 22 文件 101 项全过、`rg` 确认旧契约零残留；未打包（原生标题栏外观需用户在运行实例中验收）。

**窗口控制：原生标题栏当日回退，定稿电脑管家式自绘（2026-08-31）：** 上午按用户「不要自己实现、用 Windows 原生、最新样式」指令切换 `decorations: true` 并全量删除自绘实现；用户运行后对照微软电脑管家质询按钮差异，确认互斥事实（电脑管家按钮本就是无边框窗口自绘 / WebView2 WCO，真原生标题栏无任何样式接口，wry 不支持 WCO——wry#1650 / tauri#4531 开放）后拍板「恢复电脑管家式自绘」。本轮完整还原上午删除的全部代码：`tauri.conf.json` `decorations: false`、capabilities `core:window:allow-start-dragging`、`commands.rs` 的 `main_hwnd` 与 `window_minimize` / `window_toggle_maximize` / `window_is_maximized` / `window_close` 四命令（`SendMessageW(WM_SYSCOMMAND)` + `IsZoomed` 通道）、`lib.rs` 注册、`windows-sys` 的 `Win32_UI_WindowsAndMessaging` / `Win32_Foundation` 特性、前端 `WindowControls` 组件与测试（五用例与原契约一致：窗口态最大化钮单方块图形、最大化态镜像双方块还原图标、resize 事件同步、三钮点击分发各一次、卸载停听——图标形态断言锁死 DESIGN.md「最大化钮镜像实时窗口状态」契约）、四枚窗口图标、`client.ts` 五封装（含 `getCurrentWindow` 导入）、`base.css` 窗口按钮样式块、顶栏 `data-tauri-drag-region` 三处、`App.test.tsx` 的 `@tauri-apps/api/window` mock 与 `window_is_maximized` 分支。原生实验期间临时引入的 `apply_window_settings` `set_theme`（服务于原生标题栏主题跟随）随原生标题栏一并删除，`apply_window_settings` 回到仅置顶单一职责。最终契约与 2026-08-28 定案完全一致；`DESIGN.md` 顶栏契约恢复并补记当日往返证据，`MEMORY.md` 对应条目改写为定案复确认。验证：`cargo fmt --check`、隔离 `CARGO_TARGET_DIR` 下 `cargo test --workspace`（src-tauri 33 过 1 忽略、asb-core 86、执行器 5、切换 16，零警告）、`npm run typecheck`、`npx vitest run` 24 文件 107 项全过、`rg` 确认 `set_theme` / `window-theme-failed` 零残留；未打包（按钮外观需用户在运行实例中验收）。

**阻塞命令全面异步化与会话扫描限界读取（2026-08-31 完成，同日按「无兼容无过渡无技术债」收敛到全部命令）：** 用户报告会话扫描点击卡顿，后续要求参考 CC Switch 源码复刻其实现；原临时克隆已被系统清理，重新浅克隆 `farion1231/cc-switch` 至 `/tmp/cc-switch-ref`（MIT，仅行为参考）。参考定案模式：命令层一律 `async fn` + `tauri::async_runtime::spawn_blocking`（阻塞工作进专用线程池，主线程与 async worker 均不占）；会话元数据只读头部有限行，不整文件解析。期间并行会话把 `commands.rs` 拆分为 `commands/` 域模块（error / window / status / common_settings / switching），拆分基于转换前快照、 interim 转换曾被其覆盖，最终在新结构上一次收敛完毕。最终契约：①`commands/error.rs` 新增唯一所有者 `blocking(task)`——`spawn_blocking` + JoinError（仅闭包 panic）统一映射 `task-interrupted`，全部阻塞命令经它执行，模块文档明示该契约。②全部 24 个涉 I/O 命令转为 `async fn`：设置两命令（`set_app_settings` 仅存储写入进池、窗口置顶留在 async 上下文）、档案 CRUD 六命令、探测 / 模型获取两网络命令、发现与导入（逻辑抽为 `discovery_report()` 供命令与导入共用）、会话三命令（`list_sessions` 由不可失败改为 `Result`，前端 `SessionManager` 既有 `scanError` 捕获承接）、CC Switch 两命令（SQLite）、通用配置六命令（含此前卡顿根因 `preview_common` 整文件解析与 `apply_common` 事务）、切换 / 备份 / 恢复 / 撤回 / 差异 / 打开目录七命令（写命令尾部 `tray::refresh` 随闭包在阻塞线程执行，muda 菜单项线程安全）、状态三命令；仅窗口四钮与 devtools 切换 5 个纯原生快速命令保持同步。③托盘改为消费同步逻辑所有者 `config_status_report`（`status.rs` 抽出，命令与托盘共用，`mod.rs` 转发随之更换）。④`session_manager.rs` 元数据提取由「整文件读入 `Vec<Value>` 再遍历」改为 `BufReader` 流式 + `METADATA_LINE_LIMIT = 32` 头部限界 + 五字段拿齐提前退出（全部字段 first-wins，提前退出语义等价；限界仅当字段迟于 32 行未齐时改变行为，标题走既有回退链，与参考项目同款取舍）；`last_active_at` 维持文件 mtime（优于参考的尾部时间戳解析，无需尾部读取）；`read_messages` 流式化，`read_jsonl` 删除；新增测试锁定限界。安全边界不采纳参考项目做法：路径仍由后端解析、renderer 只传客户端与会话 ID。验证：`cargo fmt`、`cargo test --workspace`（src-tauri 37 过 1 忽略、asb-core 86、执行器 5、切换 16 全过）、`npx vitest run` 26 文件 118 项全过；`npm run typecheck` 仅剩并行会话进行中文件 `src/app/useSwitchOperations.ts` 的未用变量错误（非本轮引入、未触碰）。未打包。

**全局通知组件：复刻 spiralcoder toast 系统（2026-08-31 完成）：** 用户确认项目此前没有全局通知组件（工作区横幅≠全局通知），指令照抄 spiralcoder（uiux）。移植方式沿用 Select 复刻先例：结构与交互契约照抄，视觉映射本系统令牌。新增三文件：`src/components/use-toast.ts`（唯一契约所有者——模块级 store + `toast()` 直调，无 context；逻辑照抄：同屏上限 2、错误置顶不被挤出、按种类自动关闭 info 4.5s / success 3.2s / warning 6s / error 手动、`durationMs: 0` 单条关闭自动、悬停/聚焦/页面隐藏暂停并保留剩余时长、300ms 退场动画期、空内容与非法 duration 抛错）、`ToastStatusIcon.tsx`（四种手绘线性图标，currentColor）、`Toaster.tsx`（应用根挂载一次；关闭钮复用 `CloseIcon`；error 用 `role="alert"` 其余 `role="status"`）。`base.css` 新增 `asb-toast` 样式块：玻璃菜单材质（`--asb-glass-menu` + 菜单级模糊 + 模态投影 + 顶缘高光，`@supports` 回退挂入既有块）、图标色调映射行动蓝/安全绿/警告橙/系统红、进入 240ms / 退出 180ms 上移缩放动画（减动态效果经令牌归零）、栈视口顶部居中 `z-index: 70`。既有横幅层不动（分工记入 DESIGN.md §8）。测试 `Toaster.test.tsx` 六用例锁定：渲染与角色、按种类默认时长自动关闭、悬停暂停/离开续计、错误仅手动关闭、上限 2 且错误不被挤出、空内容拒绝；假定时器下 userEvent 挂起，改用 `fireEvent.pointerOver/Out`（React 由 over/out 合成 enter/leave）与同步 click。验证：`npx vitest run` 25 文件 114 项全过（含令牌所有权测试）、`npm run typecheck` 干净；未打包。

**全局通知接入：一次性操作反馈全面迁至 toast（2026-08-31 完成）：** 用户指令「审查代码，该接入的地方就接入显示」。审查结论与落地：①错误通道分流——`App.tsx` 新增 `reportError` 单一入口，`profile-store-unsupported` 保留横幅（持续错误 + 「清空旧档案并重新开始」承重按钮，测试锁定该交互不变），其余全部失败（约 20 处 catch：刷新、预览、保存/删除/重置、切换/恢复/撤回、锁清理、通用与应用设置、发现、CC Switch、打开备份目录）改发 error toast（title「操作失败」+ 后端 message，不自动关闭）。②写操作结果——`outcome` / `operationWarnings` 两个 state 与对应横幅整体删除：切换成功发 success「已切换到「X」」（描述「{客户端}将在下次启动时读取新配置」），恢复备份 / 撤回同构（「已恢复备份」「已撤回上一次切换」）；带警告时合并为一条 warning（标题带条数，每条警告一行，为此 Toaster 副文容器 `p`→`div`）；切换/恢复/撤回后「无法刷新本机配置发现结果」的附加警告并入同一数组；通用设置写入仅在有警告时发 warning（勾选控件状态即成功反馈，不通知）。③保留原位：发现页 CC Switch 导入结果面板内嵌展示（含未导入明细）、SessionManager 等子组件局部错误、busy 指示器。④无债清理：`base.css` 删 `.asb-banner-warning` / `.asb-banner-list`（零消费者；`.asb-banner-ok` 仍服务导入结果保留）；`SwitchOutcome` 类型 import 删除；全部 `setOutcome` / `setOperationWarnings` 调用点（含客户端 Tab 切换清理）删除。⑤测试隔离：error toast 不自动关闭且 store 跨渲染存留，`use-toast.ts` 导出 `clearToasts()`（立即清空，测试隔离专用）挂入 `test/setup.ts` 全局 afterEach，杜绝跨用例 alert 残留；`App.test.tsx` 断言迁移——切换结果断言改「已切换到「备用网关」」toast、设置保存失败与 store-unreadable 改无名 alert、撤回测试补 toast 断言、`profile-store-unsupported` 横幅断言不变。验证：`npx vitest run` 25 文件 114 项全过、`npm run typecheck` 干净；另修复并发编辑引入的 `setLocks({ codex, claudeLock })` 属性简写键名笔误（git diff 证实 HEAD 为 `claude: claudeLock`）；未打包。

**会话管理模块内部滚动（2026-08-31 完成）：** 用户报告（附 828 条会话的实况 DOM）会话页由整页滚动查看内容，要求模块自身内部滚动——这正是 DESIGN.md §148 既定契约（「列表保留独立的固定高度滚动区，长记录在详情内滚动」）未被实现兑现：`.asb-session-items` 虽有 `overflow:auto`，但其 `flex:1` 没有定高父级（布局随内容撑高、`.asb-main` 整页滚），转录区则是固定 `max-height:560px` 与视口无关。`base.css` 会话块一轮收敛：①`.asb-panel:has(.asb-sessions)` 填满主区（flex 列 + `flex:1` + `min-height:0`，`:has()` 作用域仅此面板、其他页面面板不受影响，WebView2 ≥105 支持、仓库已有先例）；②`.asb-sessions` / `.asb-session-layout` 传递高度（`flex:1` + `min-height:0`，布局保留 `min-height:520px` 作为极矮窗口的地板，此时退回页面滚动属预期兜底）；③布局行模板 `minmax(0,1fr)` 约束面板高度，`.asb-session-items` 补 `min-height:0` 使滚动真正生效；④转录区删 `max-height:560px`，改为填充详情面板余量（`flex:1` + `min-height:0` + `overflow:auto`），固定头部与元数据保持可见；⑤窄屏堆叠（≤900px）行模板改 `auto minmax(0,1fr)`、列表上限 320px→`40vh`，并因堆叠下工具栏换行 + 列表占位导致详情仅剩约 87px、固定头部即占满，改为整个详情面板滚动（`overflow-y:auto`，转录还原为自然高度随面板滚）。实测（Tauri 桩 + 300 会话 / 60 消息）：桌面 1440×820 页面零滚动、列表 496/28170 与转录 288/9302 各自内部滚；窄屏 800×820 页面零滚动、列表 261/28170、详情面板整体滚；极矮 1440×500 亦无页面滚动。验证：`npx vitest run`（SessionManager + tokens 8 项）、`npm run typecheck` 不受 CSS 影响；未打包。

**错误通知默认关闭时长（2026-08-31 完成修订）：** 用户确认需要为错误通知设置关闭时长，`use-toast.ts` 的 `DEFAULT_TOAST_REMOVE_DELAY.error` 由 `0`（仅手动）改为 `10000`——比 warning 的 6s 更长，留足阅读后端错误信息的时间；`durationMs: 0` 仍可对单条强制手动关闭，错误置顶不被挤出的排序规则不变。同步：`reportError` 注释、`Toaster.test.tsx` 错误用例改为「10s 边界自动关闭 + 手动关闭路径」、DESIGN.md §8 自动关闭条目。

**顶栏置顶快捷钮（2026-08-31 完成）：** 按用户指令加入窗口置顶切换按钮。新组件 `src/components/PinTopButton.tsx`（无状态展示件，`active` / `disabled` / `onToggle` 三契约）+ `icons.tsx` 新增 `PinIcon` 图钉图标；位于顶栏导航项与窗口三钮之间，复用 `.asb-winbtn` 尺寸与触控目标，激活态 `is-active` 走行动蓝（`--asb-action`，级联顺序天然压过 `.asb-winbtn:hover`，不写冗余悬停规则），提示与可及名「置顶窗口 / 取消置顶」并携带 `aria-pressed`；因按钮会在 busy 期禁用，Tooltip 按既有惯例经 `span.asb-tooltip-anchor` 锚定（Chromium 抑制禁用控件指针事件，直接包裹按钮会令禁用态提示失效）。单一所有权无旁路：按钮不自建状态，镜像 `AppSettings.alwaysOnTop`，点击经 App 既有 `saveAppSettings` 完整对象合并路径（busy 门控、先 native 后落盘、失败保持原视觉），与设置页「窗口始终置顶」复选框是同一设置的两个视图；设置未加载（`appSettings === null`）时不渲染。验证：`npm run typecheck`、`npx vitest run` 26 文件 118 项全过（新增 `PinTopButton` 三用例：未置顶可点、置顶镜像 `aria-pressed`、禁用不分发；App 集成用例断言 `set_app_settings` 精确载荷 `{ settings: 完整五字段对象 }` 与切换后的按钮镜像）；Rust 侧零改动（复用既有 `set_app_settings`）。`DESIGN.md` 设置契约行登记双视图关系；未打包（顶栏外观需用户在运行实例中验收）。

**通知接入无债清理（2026-08-31 完成）：** 按「无兼容无过渡无技术债」复查通知相关全部改动并同轮删净三处残留：①`Toaster.test.tsx` 的 `flushToasts` 逐条点击式清理被 `test/setup.ts` 全局 `clearToasts()` 完全取代，删除（测试文件 afterEach 仅剩 `useRealTimers`，store 清理单一所有者为全局 setup）；②`notifyWriteOutcome` 的 `app: AppKind | null` 参数与「客户端将在下次启动时读取新配置」降级文案删除——参数收紧为必填 `AppKind`：恢复场景改用 `RestoreOutcome.preRestoreBackup.app`（命令返回值自带，`backups.find` 查表、`?? null` 兜底与依赖数组中的 `backups` 一并删除），撤回场景本就必填；③切换场景 `runSwitch` 入口守卫加 `!selectedProfile` 提前返回，「已完成切换」fallback 标题与 `selectedProfile?.app ?? null` 不可达分支删除。`rg` 复查：旧 fallback 文案、`asb-banner-warning` / `asb-banner-list`、`flushToasts`、`setOutcome` / `setOperationWarnings` 全仓零残留。验证：`npx vitest run` 26 文件 118 项全过（含同轮并入的顶栏置顶并发改动）、`npm run typecheck` 干净；未打包。

**通知栈位置定稿左上角（2026-08-31 完成）：** 用户指令通知位置改左上角，`base.css` 的 `.asb-toast-stack` 由顶部居中（`left: 50%` + `translateX(-50%)`）改为 `left: var(--asb-space-2)` 左上对齐（`top` 不变），居中定位的 transform 声明删除——不与进出场 keyframes 的 translateY 冲突；宽度上限与其余契约不变。DESIGN.md §8 栈条目同步。

**软件更新检查与顶栏更新图标（2026-08-31 完成）：** 按用户指令为设置页加入检查更新、为顶栏加入更新时图标。方案边界（windows-gnu 工具链避开 ring/rustls、CI 现只传产物不发 Release，故不引入 `tauri-plugin-updater`）：手动检查、只读、不下载不安装。后端：`probe.rs` 抽出共享 `http_get`（WinHTTP GET 返回状态 + 文本，系统代理与校验器适用，User-Agent 沿用会话 agent 名），新模块 `src-tauri/src/update.rs` 是更新检查契约唯一所有者——`UpdateCheck { currentVersion, latestVersion, updateAvailable, releaseUrl, checkedAt }`，`check()` 经 GitHub `releases/latest` API 读取（404 报「还没有已发布的版本」），纯函数 `is_newer` 做点分数值比较（剥 `v` 前缀、`0.10.0 > 0.9.0`，非数字形态退化为去前缀不等式），5 条单元测试锁定比较与响应解析；命令 `check_update`（async + `spawn_blocking`，当前版本取 `app.package_info()`，即 `tauri.conf.json` 的 `version`），错误码 `update-check-failed`。前端：`client.ts` 增 `UpdateCheck` 类型与 `checkUpdate()`；新组件 `UpdateSection`（设置页「软件更新」分组：检查按钮 + 原位结果「已是最新版本」/「发现新版本 {tag}」+ 当前版本与本地时区检查时间，有新版本时追加「打开下载页面」主按钮经 opener 仅放行 https）与 `UpdateButton`（顶栏导航与置顶钮之间的行动蓝下载箭头钮，仅 `updateAvailable` 时渲染，提示「发现新版本 {tag}」，点击跳设置页）；状态所有者 `app/useUpdateCheck.ts` 挂入 `useSwitchboardModel`（busy 门控、失败走全局错误 toast），随用户当日的 App→pages/app/ 重构接入 `AppShell` 的 `update` 插槽与 `SettingsPage`。注意：仓库 CI 目前不发布 GitHub Release，发布前该检查恒报「还没有已发布的版本」；要使其生效需在 CI 增加 tag→Release 步骤（未在本轮范围内）。验证：`cargo fmt --all`、隔离 `CARGO_TARGET_DIR` 下 `cargo test -p agent-switchboard` 42 过 1 忽略（含新增 5 项）、`npm run typecheck` 零错误、`npx vitest run` 新增 `UpdateSection` 5 项 + `UpdateButton` 2 项全过（全量 28 文件中仅 `ProviderEditor.test.tsx` 2 项失败，为本轮未触碰的用户并发重构瞬时态，两次运行失败数在 1↔2 间波动）；未打包（顶栏图标与设置分组外观需用户在运行实例中验收）。

**规模规则全仓审查与拆分重构（2026-08-31 完成）：** 按 AGENTS.md §6 阈值（文件 500 审查 / 800 硬线；函数 80 审查 / 120 必拆）全仓复测并实施拆分。审查基线用脚本统计非空非注释行、Rust 内联 `#[cfg(test)]` 模块不计入。**超硬线必拆项：**①`src/App.tsx` 1396 行、`App()` 1226 行 → 分解为「模型组合 + 视图组合 + 页面 + 领域钩子」四层：`app/useSwitchboardModel.ts`（唯一模型组合层，118 行，只组合钩子不含逻辑）、`App.tsx`（视图组合，151 行）、`pages/`（Overview/Providers/CommonSettings/Settings/Backups/Discovery+CcImport 六页文件，均 ≤230 行）、`app/` 九个领域钩子（useConfigSnapshot / useSwitchPreview / useCommonSettings / useAppSettings / useDiscovery / useCcImport / useSwitchOperations / useProviders / useOperationFrame）+ global-effects + notifications + AppShell + OperationConfirmSheets，`lib/client-name.ts` 为 AppKind 显示名单唯一所有者；全部 DOM 与行为逐块搬迁未改契约，期间与用户的 toast / 置顶 / 更新检查并发改动两次撞写，均以重读合并方式收敛（写失败即 mtime 拒绝，无静默覆盖）。②`src-tauri/src/commands.rs` 836 行（剔测试）→ `commands/` 域模块（error / window / status / common_settings / switching + mod.rs 薄委托），命令在定义路径注册（Tauri 宏需定义处解析），`lib.rs` invoke_handler 改全路径，tray 所需 `config_status` / `ConfigFileStatus` 经 pub(crate) 再导出保持不动。③`executor.rs` 的 `execute_locked` 141 行 → 按业务步骤拆为 `read_unchanged_current`（读取 + 外部变更拒写）/ `plan_candidate`（预览 + 渲染 + 渲染哈希拒写）/ `back_up_current`（备份 + 元数据）/ `commit_rendered`（临时写 → 校验 → 原子替换 → 写后验证 + 失败恢复），`execute_locked` 收敛为 48 行事务编排、全路径 `finish` 释放锁，阶段名与错误变体零变化；同轮把 `display.rs`（预览展示脱敏）与 `restore.rs`（公开恢复 + 备份列举）拆为独立模块，`asb_switch` crate 根再导出面不变。**审查不拆项（单一职责成立）：**`ownership.rs` 555 行为所有权常量目录 + 查询的唯一所有者、`adapter/claude.rs` 469 / `codex.rs` 为各客户端格式契约所有者、`local_state.rs` 剔测试后约 300 行，均不混第二职责，不为凑行数机械拆分。**例外条款判定（80–120 审查带）：**`fetch_models` 108 / `http_get` 90（用户侧已抽）为完整 WinHTTP 请求生命周期、嵌套闭包承载 FFI 句柄清理，跨函数拆散 unsafe 句柄所有权无业务边界；`restore` 107 为连续事务（锁 → 校验 → 恢复前快照 → 写入 → 验证）；`import_proposal` 82 为每客户端一个完整解析分支——均符合「连续事务 / 完整解析」例外画像，不动。**遗留：**`App()` 151 行超 120 函数线：纯声明式布线（8 页面块 + 弹层 + 壳），全部可命名业务单元已抽出，再拆只会产生名称空泛的转发组件（AGENTS.md §6 明禁），留待页面接线稳定后随用户并发演进再收敛。验证：`cargo test --workspace` 全过（src-tauri 42 过 1 忽略、asb-core 86、执行器 5、切换 16）、`npm run typecheck` 零错误、`npx vitest run` App 集成 20 项与 ProviderEditor 9 项单跑全过（全量并发跑时 5s 超时抖动为用户 dev 实例 + watcher 负载所致，失败集合逐次漂移、单跑稳定全绿）；复测无任何文件超 800、无函数超 120（App 除外如上）。未打包。
