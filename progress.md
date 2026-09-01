# Agent Switchboard 目标与进度

本文件是项目**唯一**的阶段目标文档。不得创建平行的 `GOALS.md`。`README.md` 说明产品；本文件记录每个阶段必须达成的目标、验收方式以及何时可开始下一阶段。

## 工作规则

- 同时最多只能有一个实施阶段处于进行中。
- 仅在每项列出的验收检查都有证据后，才能将阶段标记为已完成。
- 如目标、范围或验收条件变化，必须先在此更新，再改动实现。
- 不得将会话记忆、本机代理状态或个人工作记录写入本仓库。
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
- 定义产品范围、安全规则、设计系统和阶段目标。
- 定义参考边界：`CC Switch` 和 `Codex++` 为后端行为提供启发；`Frosted Relay` 是项目自己的界面。

**验收**

- `AGENTS.md` 将产品限制为 `Codex` 和 `Claude Code`。
- `README.md` 说明产品边界与计划架构。
- `DESIGN.md` 定义 `Frosted Relay`、`Dual Relay`、材质、字体与无障碍约束。
- 本文件定义所有未来阶段的目标和验收条件。
- 文档区分后端参考模式与原创界面工作，并禁止从任一参考项目复制界面 / 代码。

**退出条件：** 四份根目录文档均存在、交叉引用正确，且未创建或修改任何代码或真实配置。

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

**结果：** 于 2026-08-26 完成。契约位于 `crates/asb-core/src/contracts.rs`（`AppKind`、`ProviderProfile` / `ProviderDraft`、`CommonConfigPatch` / `PatchEntry` / `PatchValue`、`SwitchPlan`、`SwitchPreview` / `KeyChange` / `ChangeKind`、`BackupRecord`、`RouteState`、`ImportProposal`）；所有权表位于 `ownership.rs`（Codex 受管顶层键含 `model_provider` / `openai_base_url`，所有 `model_providers.*` 表都归宿主所有；以及 Claude 所有的键）；锁契约位于 `lock.rs`，其中纯函数 `classify_lock` 区分空闲 / 占用 / 陈旧 / 不确定。`validate.rs` 会拒绝空名称、非 `http(s)` URL、形似密钥或非法的 Codex 环境变量名、Claude 环境变量名、宿主所有的补丁键和应用不匹配，并给出修复提示。`test_support.rs` 仅在测试构建中提供样本，包含宿主所有字段且不含凭据。规划 / 适配器在结构上不产生变更：`asb-core` 完全没有文件系统依赖；只有 `asb-switch` 接收写入能力（注入 `SwitchIo` trait）。

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

**范围调整（2026-08-26，2026-09-01 更新）：** 不再提供两套目标选择。供应商配置档保存在应用数据目录的独立供应商文件中，首次启动为空；它们保存稳定标识、客户端、名称、模型、端点、API 密钥及档案级模型选项。密钥只保存在 `state/configuration/providers/{codex,claude}/{id}.json`，切换分别写入 Codex `experimental_bearer_token` 与 Claude `env.ANTHROPIC_AUTH_TOKEN`，所有展示、预览、日志、错误和诊断均脱敏。通用设置与写入历史各有独立文件。所有状态、预览、切换、恢复和备份历史均指向用户级配置路径。自动化测试仍使用隔离的临时文件，不构成运行时模式。

**范围**

- 为真实用户配置写入提供显式确认流程。
- 删除运行时虚拟路径、预置供应商和相关界面选择器。
- 提供真实供应商配置档的新增、编辑、删除与只读导入入口。
- 将 Codex 环境变量名写入受管表，并在所有预览、错误和诊断中脱敏密钥。
- 备份浏览器、恢复流程和发布打包。
- 在通用设置中直接管理 Codex 全局 `AGENTS.md` 与 Claude Code 全局 `CLAUDE.md`，不建立第二份提示词存储；编辑区按内容在受限高度内自适应，达到上限后仅自身滚动。

**验收**

- 每次真实切换均在确认前展示精确差异和备份位置。
- 首次启动没有预置供应商；每个可切换供应商均来自用户新增或真实配置导入。
- 运行时不存在虚拟配置命令、非本机目标路径或相关界面文案。
- 状态、预览、切换、恢复和备份历史只访问真实用户级配置路径。
- 受控且由用户授权的切换与恢复均经过人工验证。
- 打包后安装与启动不会丢失应用自身的本地数据。
- 应用诊断、导出或软件包产物中不出现密钥。
- 提示词文档保存不暴露绝对路径，并经过版本比对、锁、隔离备份、原子替换和写后恢复验证。

**退出条件：** 用户能够安全地将应用用于日常 `Codex` / `Claude Code` 提供商管理。

**当前状态（2026-08-26）：** 运行时已迁移为真实配置单一路径：`config_status`、预览、切换、恢复、锁和备份命令都解析用户级目标；旧的虚拟命令、路径、预置供应商和界面选择器已删除。供应商的新增、编辑、删除、只读导入和空初始存储已实现；Codex 自定义路由固定使用内置 `openai` 与 `openai_base_url`，不写项目私有 provider 表。

**Codex 官方订阅额度（完成，2026-09-01）：** 新增一条独立于 `UsageQuery` 的只读能力，仅服务于 Codex 官方登录档案。`asb-core` 拥有已脱敏的额度结果契约；桌面服务临时读取已有 Codex OAuth 登录态、查询服务端限额窗口，并只归一化窗口名称、已用百分比、重置时间和可恢复状态。它不接管、复制、持久化或写入登录缓存，不将 OAuth 令牌或账号标识返回渲染器，也不复用第三方档案的地址、API 密钥或脚本。供应商行使用 Frosted Relay 自有的紧凑账本与细进度轨呈现，不迁移 CC Switch 视觉、文案或组件；首次显示读取一次，随后只允许手动刷新，失败时保留进程内最近一次成功结果并标明过期。未登录、失效登录、服务拒绝或网络/响应故障都有可恢复状态。验证：`cargo fmt --check` 与 `cargo test --workspace`（桌面库 120 通过、1 个既有真实 CC Switch 数据库测试忽略；核心 111、执行器 7、集成 29 均通过）、`npm test`（228 通过）、`npm run typecheck`、`npm run build` 均通过；未读取真实 OAuth 文件或执行实际额度查询，故尚未做真实账户的桌面运行时验收。

**主窗口居中与启动更新提示（完成，2026-09-01）：** Tauri 主窗口配置新增 `center: true`，使原生窗口在创建前按当前显示器居中，不依赖页面加载后再移动。更新检查从纯手动改为应用挂载后一次无阻塞、失败静默的 GitHub Release 查询；检查仍不下载、不安装、不轮询。发现较新版本时，顶栏出现带「更新」文字与状态点的可见入口，点击进入设置页并由用户选择打开 Release 下载页；设置页仍提供手动检查，只有手动检查失败才报告错误。状态与并发锁唯一归 `useUpdateCheck` 所有，启动检查不复用全局操作忙碌层。

**概览 Codex 重置信号（完成，2026-08-31）：** 概览不承载会话管理。`codex_reset.rs` 成为 `https://www.codexrunway.com/api/status.json` 的唯一后端契约所有者：一次手动读取后规范化最近确认的全局重置、`resetTimeline.nextSchedule` 公告预计与 Tibo 最近的重置相关帖子（仅放行其 `x.com/thsottiaux/status/{id}` 原帖链接）；不访问本地会话、凭据、账号额度或 X 的直接接口。最后一次成功的规范化公开结果由 `LocalState` 独立原子保存为 app-data `state/codex-reset-cache.json`，仅包含展示快照，不保存原始 payload 或凭据。概览启动只调用 `get_cached_codex_reset_status` 读取该快照并标出「本地缓存」，用户手动刷新才调用 `check_codex_reset_status`；刷新失败保留旧快照，缓存写入失败也仍展示新鲜结果并明确警告下次启动无法保留。缓存损坏响亮拒绝且不改写。当前 UI 显示来源健康提示、公开 feed 时间与非官方/非账号额度说明，并支持原帖跳转。Codex Radar 的替换数据源需先取得接口授权，未授权前不调用其完整 API。已验证：`cargo test -p agent-switchboard codex_reset`（7 过，含缓存持久化/损坏缓存用例）、`npx vitest run src/components/CodexResetPanel.test.tsx src/api/client.test.ts`（2 文件、12 测试）通过；此前 `npx vite build` 通过，`npm run build` 未完整通过的既有原因仍是本轮未触碰的 `SessionManager.tsx:395` 将 `string | null` 传给仅接受 `string` 的函数。

**会话页缓存模式（完成，2026-08-31）：** `SessionManager` 移植 `CC Switch` 的查询缓存语义（其前端以 `react-query` + `staleTime: 30s` 实现，本仓库不引入该依赖，以组件内时间戳与缓存表重新实现）：激活时缓存期内零请求直接显示列表；过期后先显示缓存再后台重扫，列表不闪回「正在扫描」占位；对话内容按 `app:sessionId` 缓存同样 30 秒，重新选中最近查看的会话即时出内容；缓存至多保留最近查看的 16 条对话记录，超出时淘汰最旧条目，保证长驻托盘进程内存有上界；「刷新会话」保留强制重扫；后台刷新失败保留已显示数据并提示错误，仅首扫失败回落空态。扫描触发的判定只读扫描时间戳、不读列表 state，避免首扫失败后 state 变化立即自我重扫。修复了每次切回会话页都整页重扫的问题。已验证：`npx vitest run src/components/SessionManager.test.tsx` 14 项通过（含期内重进零重扫、过期后台重扫保列表、后台失败保数据、消息缓存零重取、上限淘汰重取）；`tsc` 对 `SessionManager.tsx` 零错误；`npx vite build` 通过；浏览器连 dev 实测切页重进零新请求、重选零新请求。上轮记录的 `SessionManager.tsx:395` 类型错误已确认消除；当前 `tsc` 全仓剩余错误来自并行进行中的 `interfaceFont` 测试夹具，与本组件无关。


**会话消息目录清洗（完成，2026-08-31）：** 查证 `CC Switch` 的「会话内逐项对话定位」：其形态为消息目录（提取用户消息 → 点击滚动居中 → 高亮 2 秒），本项目会话页初版已有等价实现（`outlineItems` + `jumpToMessage`，含 `> 2` 条显示门槛与契约测试）。真正的差距是目录噪音清洗，本轮以自有契约复刻：`Codex` 以用户角色记录的非对话载荷不进入目录——`AGENTS.md` 指令转储与 `<environment_context>` 整条隐藏；`# Context from my IDE setup:` 包装提取内嵌请求段（取最后一个匹配标题的负载，避免选中文件内容里的同名标题干扰），提不出请求段整条隐藏。清洗只作用于 `Codex` 会话，`Claude Code` 不清洗；消息数组本身不重排，目录项仍指向原始索引。已验证：`npx vitest run src/components/SessionManager.test.tsx` 16 项通过（新增：Codex 注入载荷不进目录且 IDE 包装提取真实提问、Claude 同形消息仍进目录）；`tsc` 对 `SessionManager.tsx` 零错误；`npx vite build` 通过。

**供应商编辑器连通检测（完成，2026-08-31）：** 按用户指定的形态（配置页内、主模型右侧按钮栏）移植 `CC Switch` 的可达性检测模式，升级既有 `probe.rs` / `ProbePanel`，旧的两态契约（`url` / `reachable` 字段、表单底部「验证端点」按钮、HEAD→GET 回退）已同轮删除。现契约为三态 `grade`（`ok` / `slow` / `unreachable`）：收到任意 `HTTP` 响应即算可达（状态码只证明端点存活），延迟超过 6 秒判「连通但较慢」，仅网络级失败判「无法连通」并按 `WinHTTP` 错误码给出失败类别（域名解析失败 / 连接超时 / 连接被拒绝 / `TLS` 握手失败 / 网络请求失败）；超时类失败自动重试一次（策略函数与网络层分离、可单测），拒连与 `DNS` 失败立即定论。探测统一 `GET` 且不读响应体、不携带密钥、不发送模型请求；结果行附常驻说明「可达不代表密钥或模型配置正确」。唯一按钮与「获取模型」并列，服务地址字段只保留输入，结果显示在这组操作下方；地址变更即重置结果。已验证：`cargo test -p agent-switchboard probe` 10 项通过（分级阈值、超时重试一次、非超时不重试、二次超时终局、错误码分类）；当前按钮位置复验：`npx vitest run src/components/ProviderEditor.test.tsx src/components/ProbePanel.test.tsx` 21 项通过，`npm run typecheck` 与 `npm run build` 通过，未启动或打开浏览器。

**供应商用量查询 V2（完成，2026-08-31；展示形态于 2026-09-01 更新）：** 供应商编辑器已移除「查询用量」按钮，主模型右侧只保留连通检测与获取模型。每张供应商卡片右侧的操作按钮区都有唯一的无文字用量图标：未配置时直达独立工作区；已配置时默认显示并读取余额、已用、总量、查询方式、时间与刷新读数，并从读数内「编辑查询」回到同一工作区。图标的操作语义仅经悬浮提示与无障碍名称表达。独立工作区自己拥有查询草稿与「保存查询」动作，保存直接更新应用档案并回到列表，不再和供应商编辑草稿耦合。`UsageQuery` 是唯一带 `kind` 的契约：`declarative` 保留现有一次 `GET` + JSON 路径方式，`script` 保存用户自编 JavaScript。脚本在受限 `QuickJS` 中只生成一次 `GET` / `POST` 请求与 JSON 提取函数，实际网络仍统一由 `WinHTTP` 执行；脚本没有文件、进程、会话或直接网络能力，错误不回显脚本或密钥。旧的精确裸声明式 `usageQuery` 只在应用自有 `profiles.json` 或云备份恢复时一次性补入 `kind: "declarative"` 并原子改写；其余旧 / 残缺形状拒绝且不改写。保存边界同时校验数据形状与脚本可加载性，桌面与开发 RPC 均运行同一查询命令。已验证：`npx vitest run src/components/UsageQueryWorkspace.test.tsx src/components/ProviderUsagePanel.test.tsx src/components/ProviderList.test.tsx src/pages/ProvidersPage.test.tsx`（22 项通过）、用量相关的 `ProviderEditor` 定向断言（2 项通过）、`npm run build` 与目标文件 `git diff --check` 通过；未启动或打开浏览器。

**供应商用量账本默认展开（完成，2026-09-01）：** 已配置 `UsageQuery` 的供应商卡片进入列表即显示并读取紧凑账本区；未配置卡片不增加占位，仍由用量图标进入独立配置工作区。账本标题为「用量 / 字段提取或脚本 / 更新时间」，右侧保留「编辑查询」和「刷新」；每额度只保留名称、余额或已用主数字和总量，只有能够由总量与已用或余额推导比例时才显示细进度轨。原三列大表格、整块灰底和点击后才读取的展开路径已删除；用量图标改为默认展开状态下的收起/再次展开控制。验证：`npx vitest run src/components/ProviderUsagePanel.test.tsx src/components/ProviderList.test.tsx src/pages/ProvidersPage.test.tsx`（23 项通过）、`npm run typecheck` 与 `npm run build` 通过；未启动桌面实例做视觉验收。

**系统托盘快速切换与缓存用量摘要（完成，2026-09-01）：** 原静态的 Codex / Claude 路由文本已删除。`tray.rs` 以真实档案和客户端状态动态构建两个原生子菜单；当前实际匹配的供应商显示为禁用勾选项，选择其他供应商先调用共享 `preview_switch`，再带预览哈希调用唯一的 `execute_switch` 事务，托盘不生成配置文本、不开新写入路径。`query_profile_usage` 成为已保存供应商的唯一受管查询入口：前端仅提交 `profileId`，后端解析查询、服务地址和密钥并将成功摘要暂存于进程内；托盘只读取该缓存，在每个已有读数的菜单项与当前供应商标题显示第一条紧凑额度，不在打开菜单时联网。更新/删除档案会使对应缓存失效，重置档案会清空缓存；任何档案变更、查询成功或切换后均刷新菜单。档案状态不可读时保留可打开主界面的恢复菜单。验证：隔离 Rust 构建目录中 `cargo test -p agent-switchboard tray --lib`（6 项通过）、`cargo fmt --check`、`npx vitest run src/api/client.test.ts src/components/ProviderUsagePanel.test.tsx src/components/ProviderList.test.tsx`（3 文件、37 项）、`npm run typecheck`、`npm run build` 与目标文件 `git diff --check` 通过；未启动桌面实例做真实托盘交互验收。

**用量图标化（完成，2026-08-31）：** 卡片用量入口已收进既有操作按钮区，视觉只显示图标，继续以 tooltip、`aria-label` 与展开态表达真实操作。已验证：`npx vitest run src/components/ProviderList.test.tsx src/pages/ProvidersPage.test.tsx`（14 项通过）、`npm run typecheck`、`npx vite build --logLevel error` 与目标文件 `git diff --check` 通过；未启动或打开浏览器。

**供应商卡点击高亮移除（完成，2026-08-31）：** 点击供应商仍保留内部选择语义，用于预览、编辑与启用流程，但不再给卡片附加 `.is-selected` 或渐变描边、蓝调表面、投影等视觉态。启用按钮的持久可见性改直接消费行控件的 `aria-selected`，避免保留仅为呈现服务的选择类；实际匹配配置的 `.is-live` 卡片及「使用中」状态保持不变。验证：`npx vitest run src/components/ProviderList.test.tsx`、`npm run typecheck` 与 `npx vite build --logLevel error`。

**供应商卡连通性测试图标（2026-08-31 实现，2026-09-01 更新）：** 供应商行右侧既有动作簇新增无文字连通性图标。首次点击直接复用 `probe_endpoint` 对该档案的服务地址做检测，检测中禁用图标，结果（结论 / HTTP 状态 / 延迟 / 时间与范围说明）在同一张卡片内展开；结果显示时图标保持激活，提示和无障碍名称改为「收起 {供应商} 连通性结果」，再次点击移除结果区域，收回后再次点击重新检测。不新增第二个网络 API、凭据路径或模型请求。编辑器与卡片共享 `useEndpointProbe` 请求生命周期和 `ProbeFeedback` 展示，卡片可见性只由本地 `probeOpen` 与探测结果共同派生，地址变更时两处均废弃旧结果。验证：`npx vitest run src/components/ProviderList.test.tsx src/components/ProbePanel.test.tsx`（25 项）、`npm run typecheck` 与目标文件 `git diff --check` 通过。

**供应商卡操作区排序（完成，2026-09-01）：** 按用户指定将右侧图标动作簇固定为「编辑 → 预览/收起预览 → 测试连通性 → 用量查询/配置用量 → 删除」；只调整 DOM 次序，五项既有条件、图标、提示和行为均不变。验证：`npx vitest run src/components/ProviderList.test.tsx`（16 项）通过。

**路由、模型映射与通用设置（2026-09-01 契约重塑）：** 供应商档案以显式 `routeMode` 表示 `custom` 或 `official`。自定义档案必须声明服务地址与密钥；官方入口不存储地址、密钥、模型覆盖或用量脚本，凭据继续由 Codex / Claude 原生登录管理。Codex 自定义档案固定写回 `model_provider = "openai"`，并通过官方 `openai_base_url` 使用自定义服务；不创建或覆盖任何 `model_providers.*` 表。Claude 自定义档案使用主模型、Haiku、Sonnet、Opus 与 `availableModels`；`ANTHROPIC_SMALL_FAST_MODEL` 是供应商受管清理字段，当前档案未声明时一定移除，导入时升格为 Haiku 档。启用官方入口会清理本应用受管的自定义路由字段，保留宿主未知字段与客户端登录缓存。每客户端的通用设置存于 `state/configuration/common/{codex,claude}.json`，字段都是完整具体值。启用或重新应用当前供应商时，执行器合成「固定通用设置 + 完整供应商声明」，清理上一供应商拥有但当前未声明的字段，并保留未知宿主字段。旧聚合文件及上一版拆分档案均只做一次确定性、原子迁移；运行时没有兼容读取。

**官方设置目录覆盖（2026-09-01 完成第一阶段）：** `asb_core::ownership::official_setting_directory` 成为 Codex / Claude Code 官方设置族的唯一覆盖图，并直接派生已有可写标量的目录行；每一族都标注为「基础参数」「独立模块」或「保留不写入」。因此页面不再把 MCP、Hooks、权限/沙箱、环境映射、模型选择器、项目/本地/受管配置、官方身份与运行态笼统藏为“未知键”：用户可见其精确路径族及当前真实边界。全局 `AGENTS.md` / `CLAUDE.md` 被明确归入官方目录中的独立模块，仍走专属文件事务；基础参数仍只写应用状态，供应商重新应用仍是唯一真实客户端配置写入路径。目录不引入任意 TOML/JSON 编辑器，也不改变项目、受管或登录资源的所有权。验收：core 目录测试、Tauri 编辑器投影测试、通用设置页目录/文档隔离测试与 TypeScript 类型检查。

**生效状态与回退（2026-09-01 收敛）：** `config_status` 报告 `MatchStatus`：匹配当前供应商投影、供应商或通用设置已变更、已恢复备份、外部修改、未托管或不可判定。恢复记录优先于同形状档案，因此不会错误点亮供应商行。通用设置保存只更新应用状态；页面据最新投影记录和哈希显示已应用、待重新应用或真实配置已被外部修改。只有供应商预览绑定当前文件哈希与候选渲染哈希，确认前任一方变化都会拒绝写入。缺失的用户级文件只会在确认后创建，撤回会恢复为“不存在”。恢复前快照拥有旁车元数据，因而也可撤回；备份浏览只接受属于当前客户端、当前本机目标且与旁车路径一致的记录。供应商投影的应用记录保存失败时，客户端文件会在同一事务中恢复；概览显示可观察写入锁，并要求确认后才清理遗留锁。

**端点验证（2026-08-26 审查收尾）：** 供应商编辑器提供手动“验证端点”：Windows 原生 WinHTTP 使用系统自动代理发送 HEAD，在 HTTP `405` / `501` 或请求错误时回退 GET，只展示可达性、HTTP 状态、延迟与检测时间，不自动选择端点。地址变更会使旧结果失效。

**验证证据（2026-08-26 审查收尾）：** `cargo fmt --all -- --check`、`cargo test --workspace`（96 项）、`cargo build --workspace`、`npm run typecheck`、`npm test`（35 项）与 `npm run build` 已通过；NSIS 安装程序已生成到 `target/release/bundle/nsis/Agent Switchboard_0.1.0_x64-setup.exe`。关键新增回归覆盖候选预览过期、首次创建后撤回为缺失、恢复后恢复前快照可用、恢复写后校验回滚、遗留锁空闲判定、旧存储迁移保真、审计日志失败警告与前端预览失效。本轮仍未对用户真实配置执行切换或恢复，安装程序也尚未安装或启动验证。

**从 CC Switch 导入供应商（2026-08-27 完成）：** 发现页提供只读扫描本机 CC Switch 数据库（`~/.cc-switch/cc-switch.db`，SQLite，`mode=ro&immutable=1` 连接）并一键导入供应商档案。范围与规则：

- 仅导入 `app_type` 为 `codex` / `claude` 的供应商；其余客户端列入跳过清单并说明超出产品范围。
- 自定义供应商的 API 密钥在后端提案中保留并仅写入应用自有档案：Codex 取 `auth.OPENAI_API_KEY`，Claude 取 `env.ANTHROPIC_AUTH_TOKEN` 或 `env.ANTHROPIC_API_KEY`。官方行导入为无密钥的官方入口，不读取或复制 OAuth / 登录令牌。扫描响应是无密钥摘要，导入响应只返回数量与跳过项；两种响应、警告和日志均不含密钥值。
- Claude 映射：`env.ANTHROPIC_BASE_URL` → 服务地址（有→自定义，无→官方）；`ANTHROPIC_MODEL` → 主模型；`ANTHROPIC_DEFAULT_{OPUS,SONNET,HAIKU}_MODEL` → 三档模型映射；CC Switch 的模型名末尾 `[1m]` 或 `[1M]` 归一为语义化 1M 开关，实际 Claude 配置始终渲染为小写 `[1m]`；无法表示的键仅以字段名列警告。
- Codex 映射：`auth` 含 OAuth 令牌或空 `OPENAI_API_KEY` → 官方登录；否则解析内嵌 TOML 的 `model_providers` 表（`wire_api` 必须为 `responses`），表内额外键（如 `http_headers`）会导致该供应商跳过并说明原因，TOML 解析失败同样跳过。
- 通用设置项不从发现文件导入；用户在通用设置页自行保存。启用的、可转换的 CC Switch 查询脚本随其供应商档案导入。
- 去重：与现有档案七字段全等者标记“已存在，将跳过”；导入时重新扫描避免过期预览。
- 全程不写真实 `Codex` / `Claude Code` 配置；唯一写入目标是应用自有配置目录。

实现：映射规则唯一所有者为 `asb-core::ccswitch`（纯函数，注入行数据）；SQLite 只读访问在 `src-tauri/ccswitch_source.rs`（`rusqlite` bundled，首个 C 编译依赖，已在 windows-gnu 工具链验证）；命令 `scan_ccswitch` / `import_ccswitch_profiles`；界面为发现页「从 CC Switch 导入」区块，逐项复选（通用 Checkbox 组件）、批量导入并汇总结果。`LocalState::profile_exists` 与 `import_profile` 共用同一七字段判等谓词。

验证证据：`cargo fmt --all -- --check` 通过；`cargo test --workspace` 111 项全部通过（含 asb-core 映射 12 项、tempdir 假库扫描/导入集成 3 项）；`npm run typecheck` 与 `vitest` 40 项通过（含扫描预览、勾选、导入参数与结果汇总）；对本机真实库执行 `--ignored` 只读扫描测试：10 行供应商 → 7 可导入 + 3 跳过，警告全部来自固定模板、无密钥值；NSIS 包已重建并静默安装。

**供应商列表拖拽排序（2026-08-28，2026-09-01 契约更新）：** 供应商行新增行首拖拽把手，列表可拖拽重排（2026-08-28 用户指令，参考 CC Switch 的交互方式；界面用本系统令牌实现）。显示顺序的唯一所有者是每个客户端目录内供应商文件的 `order` 字段；`reorder_profiles` 命令按客户端校验排序清单（必须覆盖该客户端全部档案、不得重复、不得混入另一客户端）及每个文件版本后，在单个存储事务中写入新顺序；非法或陈旧清单整体拒绝、不部分写入。前端引入 `@dnd-kit`（core / sortable / utilities，React 19 兼容）：`PointerSensor` 8px 启动约束保证行点选不被吞、`KeyboardSensor` + `sortableKeyboardCoordinates` 提供键盘排序、`closestCenter` + 垂直列表策略；把手是行内唯一拖拽目标，把手按钮 ≥40px 且保留可见键盘焦点。拖拽中的行加强为行动蓝描边与路由投影，不缩放。

验证证据：`cargo fmt --all -- --check` 通过；`cargo test`（src-tauri 17 项，含重排两条：跨客户端位置保真、非法清单拒绝且不改写存储）；`npm run typecheck` 与 `npm test` 49 项通过（含把手按需渲染、键盘重排上报新顺序）。

**供应商行「启用」主按钮（2026-08-28 完成）：** 非生效供应商行的动作簇左侧新增实心行动蓝「启用」主按钮（Play 图标，复用共享 `.asb-btn-primary` 令牌样式），生效行不显示、由既有「使用中」胶囊表达状态（2026-08-28 用户指令补齐 CC Switch 式行主按钮；行为经用户选定）。行为契约：启用按钮点击＝选中该行并拉起变更预览（与预览图标同一处理器），写入仍需显式确认——为此变更预览面板头部新增「确认切换」主按钮，复用概览页路由控件同款 `ConfirmSheet` 确认弹窗与 `runSwitch`（带 `confirmWrite`）。供应商页由此具备完整的「启用 → 看差异 → 确认切换」闭环，此前确认入口仅在概览页。安全规则不变：每次真实切换在确认前展示精确差异与备份位置，任何行内按钮都不直接写配置。现展示时机见「供应商行『启用』主按钮选中门控」条目。

验证证据：`npm run typecheck` 与 `npm test` 50 项通过（含新增：生效行无启用按钮、非生效行点击启用回调携带完整档案）。

**Supabase 加密云端备份（2026-08-31 完成，用户指令「放到备份页内」）：** 备份页在本地历史下方新增「加密云端备份」，不增加设置页入口。`src-tauri/src/cloud_backup.rs` 是远端契约唯一所有者：用户填写自己的 Supabase HTTPS 项目地址、publishable key 与登录邮箱；应用显示一次性初始化 SQL，创建 `agent_switchboard_cloud_backups`，启用 RLS，仅向 `authenticated` 授予 select/insert/update，并以 `(select auth.uid()) = user_id` 分别约束读、写入和更新。用户每次上传/恢复临时输入 Supabase 登录密码（`/auth/v1/token?grant_type=password`），不持久化登录密码、access/refresh token 或 secret/service key；`profiles.json`（供应商 API key、通用配置、切换记录）在本机以用户备份密码的 Argon2id 派生 AES-GCM 密钥加密，远端只接收密文。`state/cloud-backup.json` 仅原子保存项目地址、publishable key、邮箱。上传和恢复都需确认；恢复只替换本机档案数据，不写 Codex/Claude 真实配置，随后失效预览并刷新状态。`probe.rs` 收敛出共享 `http_request` WinHTTP POST/GET 通道；Tauri 与浏览器 debug bridge 均注册同一套带类型命令。测试：Rust `cargo test -p agent-switchboard`（隔离 target）67 过 1 忽略，覆盖加密往返、错误密码、RLS SQL、设置持久化；前端 `CloudBackupPanel`/API/App 定向 35 项全过，`rustfmt --check` 新模块通过，`git diff --check` 无问题。未作真实 Supabase 上传/恢复（本轮无用户项目凭据）。全量前端目前被并行中的 `ProviderEditor.test.tsx:60` invoke mock 类型错误和同文件 5 秒超时阻断；全 workspace Rust 测试另被并行新增 `ProviderProfile.usage_query` 未更新 `asb-switch/tests/switch_tests.rs` 的两处 fixture 阻断，均未触碰。

**备份页双 Tab（2026-08-31 完成，用户指令「备份页内双tab页实现」）：** 备份页改为「本地备份」与「加密云端备份」双 `Tab`，默认本地历史。撤回上一次切换、打开备份文件夹与历史恢复只在本地页签显示；Supabase 连接、加密上传与恢复只在云端页签显示。页签切换仅改变界面状态，不读写档案或云端数据。新增 `BackupsPage` 定向测试，连同云端面板、API 边界和应用集成测试共 36 项通过。

**本地备份内滚动（2026-08-31 完成，用户指令「本地备份实现模块内自身滚动」）：** 本地备份页签填满主工作区，历史容器 `asb-backups` 成为唯一滚动所有者；撤回、打开文件夹和上次操作说明固定在模块顶部。外层 `asb-main` 不再因备份行增长而承担该页签的滚动；空历史保持普通空态。

**撤回差异确认（2026-08-31 完成，用户指令「撤回弹窗显示差异化」）：** 撤回确认弹窗在写入前读取目标备份的受管键差异，并按撤回实际方向呈现为“当前 → 将恢复备份”。差异载入中或失败时确认按钮禁用，取消会丢弃此次只读结果且绝不写入；长差异在弹窗明细区内滚动，确认 / 取消操作固定可见。

**CC Switch 查询脚本导入（完成，2026-09-01）：** 只读扫描额外读取 providers.meta.usage_script。唯一映射点 asb_core::ccswitch 把可表示的已启用 JavaScript 自定义源码一次性编译为本应用的 UsageQuery::Script；持久化和运行时没有 ccswitch 脚本种类或兼容分支。导入脚本仅使用档案的 baseUrl 与 apiKey，并将单项或多项套餐结果转换为 UsageSummary.readings；界面逐项显示具名套餐，绝不合并或丢弃套餐名。无源码内建模板、禁用脚本、独立查询凭据/地址、未知占位符和运行时不可加载源码均保留供应商导入并以字段名说明。脚本文本、查询凭据和提案对象始终留在后端，扫描/导入响应仅包含安全的“可导入”状态和计数。验证：cargo test -p asb-core ccswitch、cargo test -p agent-switchboard ccswitch_source --lib、cargo test -p agent-switchboard usage_query --lib、npm run typecheck、定向 vitest 通过；未对真实客户端配置写入或执行网络查询。

## 当前风险

**软件日志五档复验阻断（2026-08-31）：** 本轮完成时的日志定向 Rust 测试、应用设置测试、前端定向测试与 `cargo check` 均已通过；随后并行模型契约新增 `ClaudeModelSettings.primaryOneM / sonnetOneM / opusOneM`，但 `crates/asb-core/src/ccswitch.rs:137` 与 `crates/asb-core/src/discovery.rs:250` 尚未同步初始化，当前会阻断后续 `cargo test`；同一并行改动也使全仓 `npm run typecheck` 在 `ProviderEditor.tsx:100` 和其测试夹具报相同三字段缺失。以上文件不属于软件日志范围，本轮未修改，待其所有者收敛后再做全仓复验。

**软件自身日志五档与中文界面（2026-08-31 完成，用户指令「日志等级应该分为这么多类并汉化」）：** `runtime_log.rs` 将持久化的记录阈值与已写入事件的严重性拆为两个闭合契约：`debug / info / warn / error / silent` 是 `AppSettings.runtimeLogLevel` 的唯一阈值，`debug / info / warn / error` 才可作为 `RuntimeLogEntry.level` 进入日志表；`silent` 只停止后续写入，绝不伪装成一条日志。启动读取已持久化阈值，设置成功落盘后立即更新进程缓存，插件保持 Debug 上限而由契约本身过滤，故没有双持久化状态或旧级别回退。`settings.json` 收紧为七字段当前形态；仅恰好上一版六字段形态会原子补入 `info`，更旧或未知形态明确拒绝。日志页复用既有 `Select` 与 `useAppSettings` 的唯一保存路径，中文呈现「调试 / 信息 / 警告 / 错误 / 静默」选择器及前四级筛选；窄屏控件纵向排列。验证：`npm run typecheck`、`npx vitest run src/pages/LogsPage.test.tsx src/api/client.test.ts src/App.test.tsx src/components/AppSettingsForm.test.tsx`（50 项）、隔离 `CARGO_TARGET_DIR` 下 `cargo test -p agent-switchboard runtime_log --lib`（5 项）、`cargo test -p agent-switchboard local_state --lib`（24 项）和 `cargo check -p agent-switchboard` 全通过；按用户要求未启动桌面/浏览器、未操作真实配置或日志目录。

**软件日志 Tab（2026-09-01 收敛）：** 用户澄清要的是软件自身日志后，日志页不承载配置写入历史或主从详情；写入历史仅由配置状态和撤回操作消费。唯一后端所有者为 `src-tauri/src/runtime_log.rs`：官方 `tauri-plugin-log` 仅接受本应用的闭合 JSON 运行事件，当前文件 20 MiB、保留 4 个轮转归档、跨启动追加；记录应用启动及显式的应用设置/档案、通用设置、配置切换与恢复、锁恢复、云端备份、会话恢复、CC Switch 档案导入操作，成功写 `info`，失败只写既有稳定错误码，绝不写档案名、路径、配置、密钥、请求/响应、异常消息或栈。`list_runtime_logs` 同时经 Tauri 与浏览器开发桥只读该应用日志目录，解析当前及归档文件、倒序返回最多 500 条；日志页自己加载/刷新，不再混入 `useConfigSnapshot` 的配置状态轮询，提供「全部 / 信息 / 错误」筛选及时间、级别、事件、错误码表格。新增 `open_runtime_log_dir`：后端自行解析并在必要时创建应用日志目录，再交给系统文件管理器，渲染层不接收路径；日志页提供对应的「打开日志文件夹」按钮。公开 CC Switch 源码只作为“统一后端记录 → 有界文件轮转 → 前端只读”的职责参考，未复制代码或视觉。

- 真实配置切换与恢复仍需在用户选定的可恢复配置副本上逐项人工验证。
- "需要重启客户端生效"目前是静态提示；应用不检测 Codex / Claude Code 进程的启动时间。
- 除非明确将其加入新目标，`MCP` 管理仍不在这些初始阶段范围内。

**通用设置与供应商投影（2026-09-01 完成）：** 通用设置页以 `Codex` / `Claude` 双 `Tab` 编辑 `state/configuration/common/{codex,claude}.json` 的具体合法值。目录的所有权、控件、值约束和供应商缺席清理由 `asb_core::ownership::setting_specs` 唯一拥有。保存不接触客户端文件、备份、锁或写入历史；保存后保留草稿失败态，并显示已应用、待重新应用或外部修改状态。真实配置仅由供应商事务生成：完整供应商声明与固定通用设置合成，当前供应商没有声明的旧供应商字段会移除，未识别宿主字段保持不变。供应商、通用设置和写入历史各有独立文件；旧聚合数据只在首次读取时确定性迁移，成功后删除源文件，运行时不再读取。真实用户文件未被自动化测试触碰。

**通用设置单一页尾工具栏（2026-09-01 完成）：** 全量恢复、保存状态与保存操作收敛为一个页面页尾：左侧「全部恢复默认值」，右侧「状态 → 保存通用设置」。待重新应用或真实配置异常占用同一状态槽；预览维持在其后的独立分隔区。分组只保留本组恢复，页面不再存在重复页尾或跨页面跳转操作。

**开发机调试开关（2026-08-28）：** 应对「每次改动都要重新构建」的迭代成本：`tauri` 启用 `devtools` 特性，新增 `toggle_devtools` 命令并在前端监听 `F12` 切换 WebView 检查器（唯一后端入口 `src/api/client.ts::toggleDevtools`，边界测试约束不变）。运行中的 exe 可直接改 CSS / DOM 实时预览；源码级迭代用 `npm run tauri dev`（Vite 热重载，调试构建自带 devtools）。按用户指令：仅在用户明说「打包」时执行 `npm run tauri build`。验证：`cargo test --workspace`（122 项）与 `npm test`（50 项）通过；本轮未构建交付物。

**推理强度档位滑块（2026-09-01 收敛）：** `model_reasoning_effort` 是 Codex 通用设置的具体档位：`minimal / low / medium / high / xhigh`。滑块保存到通用设置文件，供应商重新应用时才投影。其所有权、合法值、控件和供应商清理由 `setting_specs` 唯一提供，档案不再携带该字段；旧档案级值仅经一次冲突安全迁移进入固定客户端通用设置文件。

**Tooltip 与复选框复刻（2026-08-28 完成）：** 从 `spiralcoder` 复刻两个通用组件并接入。`Tooltip`：引入 `@radix-ui/react-tooltip`（悬停/聚焦触发、Portal 定位、150ms 延迟），深色墨底 + 画布白字 + 模态投影，动画随 `prefers-reduced-motion` 关闭；禁用按钮经 `span` 锚点包裹也能给出原因提示。`Checkbox`：改为参考项目的 `sr-only input + 绘制控件` 结构（`data-checked` 驱动、悬停描边、按下缩放、内嵌高光），新增 `indeterminate` 半选态（读屏 `mixed`），既有调用方 API 不变。接入点：通用设置推理强度档位值签（悬停显示将写入的确切行 / 默认档说明）与 `Dual Relay` 的「查看变更」「安全切换」按钮（禁用时悬停解释前置条件：未选档案 / 未确认预览 / 写入中）。验证：`npm run typecheck`、`npm test`（54 项）与 `cargo check` 通过；本轮未打包。

**滑块完整样式复刻（2026-08-28）：** 按用户指令把推理强度滑块升级为 `spiralcoder` 完整复刻：胶囊轨道（墨色 9% 底 + 内嵌描边）、能量渐变填充（新令牌 `--asb-gradient-energy`：安全绿→行动蓝→Claude 紫 96°，本系统第二处获准渐变表面）、填充内 9 颗白色微光粒子（glow + 1.8s 闪烁动画、逐粒子定位与延迟）、刻度悬停放大（ease-out-back）、28px 白滑钮（悬停加深投影、按下 0.96 + 120ms 快速跟随）、原生 range 全尺寸透明叠加（webkit/moz thumb 透明占位）。`tokens.css` 新增 `--asb-motion-fast` 与 `--asb-ease-out-back`；`prefers-reduced-motion` 下全部动效静止。验证：`npm run typecheck`、`npm test`（54 项）通过；未打包。

**原生 tooltip 全量替换（2026-08-28 完成）：** 界面不再使用浏览器原生 `title` 提示，全部改为复刻的深色 `Tooltip` 组件。替换点：供应商页与通用设置页的品牌图标 Tab（`Codex` / `Claude`，补 `aria-label` 维持可及名）、新建供应商 `+` 钮（禁用态经 `span` 锚点包裹）、窗口控制三钮（最小化 / 最大化-还原（动态）/ 关闭，`side=bottom`）、供应商列表的拖拽手柄与启用 / 预览 / 编辑 / 删除五个行内钮（拖拽手柄与 dnd-kit 事件经 Radix `Slot` 合并无冲突）。`ConfirmSheet` 的 `title` 属性是对话框标题而非提示，保持不变。可及名一律由 `aria-label` 承担，提示仅作视觉补充。验证：`npm run typecheck`、`npm test`（54 项，含 dnd-kit 键盘拖拽）通过；未打包。

**候选文件全文与通用配置片段预览（2026-09-01 收敛）：** 候选文件全文只属于供应商投影预览：它展示当前通用设置和完整供应商声明合成后的真实候选内容，并保留宿主未知字段。通用设置页最底部可按需查看由后端适配器从当前草稿渲染的只读通用配置片段（Codex 为 TOML、Claude 为 JSON）；片段只含非默认的通用拥有字段，不读取或写入宿主文件、不包含供应商或宿主字段，不能伪装为客户端最终候选。

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

**窗口控制改走 Win32 系统命令（2026-08-28 完成）：** 按用户指令把标题栏三钮（最小化 / 最大化-还原 / 关闭）的执行路径从 tauri 窗口抽象（`window.minimize()/maximize()/close()`）改为 Windows 原生实现，前端样式零改动。背景：三钮本就是自绘（无装饰窗口 + webview 内 SVG 按钮），此前仅点击动作经 tauri 转发 `ShowWindow`。实现：`commands.rs` 四个窗口命令统一经 `windows-sys` 走 `SendMessageW(WM_SYSCOMMAND, SC_*)`——与系统标题栏按钮同一通道（`SC_MINIMIZE` / 最大化态经 `IsZoomed` 判定后发 `SC_MAXIMIZE` 或 `SC_RESTORE` / `SC_CLOSE`，`SC_CLOSE` 经 `WM_CLOSE` 仍进 tauri 的 close-requested 流程）；`window_is_maximized` 同步改用原生 `IsZoomed`；HWND 取自 `window.hwnd()`（`windows` crate 句柄与 `windows-sys` 同为 `*mut c_void`，直接透传）。`Cargo.toml` 的 `windows-sys` 增加 `Win32_UI_WindowsAndMessaging` 特性。前端 `WindowControls` / api client / 命令签名均未动。架构定案（用户三选一拍板「维持现状」）：自绘按钮 + 系统命令通道即最终形态——真原生标题栏（顶部加系统栏、纯色不能磨砂）与 WebView2 Window Controls Overlay（理想形态但 SDK prerelease、wry 不支持 wry#1650、fork 即技术债）均被否；悬停最大化不弹 Snap Layouts 为 WebView2 平台限制，已知且接受。验证：独立 `CARGO_TARGET_DIR` 下 `cargo check` 通过（本轮窗口命令区域零警告零错误）；全量 `cargo test --workspace` 因用户并发进行中的 `fetch_models` 凭据重构（`probe.rs` / `commands.rs` 测试段编译错误）暂无法收集，非本轮引入，待该改动落地后随跑；未打包。

**供应商 API 密钥档案化（2026-08-28，2026-09-01 存储更新）：** `ProviderProfile` / `ProviderDraft` 的 `apiKey: String` 由每个客户端的独立供应商文件保存，编辑器中 Codex / Claude 均有必填密码输入「API 密钥」；空白或超过 4096 字符拒绝。切换映射：Codex 写受管顶层 `experimental_bearer_token`，Claude 写受管 `env.ANTHROPIC_AUTH_TOKEN`；通用设置不可写。模型获取命令只接收当前编辑器 `apiKey`。CC Switch 和本机发现仅在后端导入边界读取凭据；密钥不进入 RouteState、告警或诊断。`FilePreview.content` 是展示专用脱敏副本，执行器内部原文只用于哈希、校验与原子写入；TOML/JSON 按敏感键路径遮罩。差异、错误、日志与诊断继续只见遮罩。

**变更预览接入优质打印全文（2026-08-28 完成）：** 按用户指令把优质打印模块接入供应商变更预览，替换真实配置下不可读的「保持不变的设置」扁平键列表（Codex 宿主键可达 90+ 个）。契约迁移（单一所有者 = `asb-switch::executor::FilePreview`）：`FilePreview` 新增 `content` 字段——`read_preview_for` 把候选渲染一次，`rendered_hash` 与界面展示共用同一文本；`preview_switch` 与 `preview_common` 由此都携带全文。同一改动删除旧路径：`commands.rs` 的 `CommonPreview` 包装结构与其重复的读取＋`common_render` 渲染段整体移除，`preview_common` 直接返回 `FilePreview`（`preview.target` 回填与 `preview_switch` 对齐）；前端 `CommonPreview` 接口删除。界面：`PreviewInspector` 的「保持不变的设置」键值行与顶部路径 h2 移除（路径由 `CodePreview` 文件条显示，避免重复），在差异视图与警告之后渲染 `CodePreview` 候选全文——被保留的宿主键原位可见，`320px` 限高滚动不撑爆行内展开区。安全不变性以隔离测试锁定：执行后落盘文件与 `fp.content` 逐字节相等。验证：`cargo test -p asb-switch`（3 + 16 项含新断言）、`npm run typecheck`（仅剩用户并发 `mock-dev-entry.tsx` 的既有脚本作用域错误，非本轮引入）、`npm test` 78/78（`PreviewInspector` 新增候选全文原位断言；`fetch_provider_models` 新签名断言由用户会话同步更新）；`cargo test --workspace` 暂被用户进行中的凭据改动（`claude_credential` 可见性）阻塞，与本轮无关；未打包。

**扁平列表全量清查与旧契约删除（2026-08-28 完成）：** 按用户指令复查全部列表渲染面并完成优质打印迁移收尾。清查结论：唯一把配置键平铺成顿号列表的就是上一轮已替换的变更预览「保持不变的设置」；其余列表均非键堆——概览配置状态与发现页为结构化状态行（路由 / 匹配 / 警告消息），CC Switch 扫描为复选行＋单行 meta，操作警告为消息 `<ul>`，确认弹窗为单行事实，备份差异已走 `DiffPreview`，通用设置与供应商变更预览已走 `CodePreview` 全文，均维持现状。随之删除旧契约（前端已零消费）：`SwitchPreview.preserved` 字段从 Rust 契约与 TS 接口整体移除，两适配器的 `collect_preserved` 收集逻辑删除，宿主键安全性改由既有的渲染级测试承载（codex `render_preserves_host_content_and_comments`、claude `render_updates_owned_keys_and_keeps_host_blocks`），两个 preview 测试改名并删除 preserved 断言；前端 fixtures 同步。用户未跟踪的 `mock-dev-entry.tsx` 中残留的 `preserved: []` 为无类型 mock 数据，运行时无害，因该文件正被并发会话编辑而未触碰。验证：`cargo test --workspace`（132 项全过，含 asb-core 86 / asb-switch 19）、`npm test` 78/78、`cargo fmt --check` 本轮改动文件零差异（现存 diff 均为用户并发会话文件）；typecheck 仅剩 `mock-dev-entry.tsx` 两个既有脚本作用域错误（非本轮引入）；未打包。

**通用设置勾选导致界面偏移——实测定位与结构修复（2026-08-28 完成）：** 用户报告通用设置页勾选 / 取消勾选导致界面整体偏移。为避免凭读码猜测，搭了临时实测环境（`mock-dev` 入口给真实前端装 stateful invoke 桩 + 完整目录数据 + 真实延迟，Playwright + 系统 Edge 以 PerformanceObserver `layout-shift` 实测点击前后几何），定位出两类真实位移源：其一，操作横幅（错误 / 警告 / 切换结果）渲染在 `.asb-main` 滚动流顶部，其插拔把下方内容整体推移约一行横幅高度（实测 `scrollTop` 跳变 +64px，仅靠 Chromium 滚动锚定掩盖）；其二，配置预览体 `max-height` 随文件行数增长、滚动条槽位出现引发重排（边界几何实测位移分 0.059）。结构修复（无兼容层、无过渡方案）：`App.tsx` 新增 `.asb-workspace` 包裹层，三横幅移出滚动流、装入 `.asb-banner-stack` 绝对定位浮层（`pointer-events: none`、模态投影、近实心底）；`.asb-main` 补 `flex: 1; min-height: 0` 并加 `scrollbar-gutter: stable`；`.asb-filepreview-body` 由 `max-height` 改为固定 `height: 320px` 并加 `scrollbar-gutter: stable`。实测复验全部归零：成功路径（勾选 / 取消 / 分段选择 / 恢复默认，Codex 与 Claude）、错误注入、警告注入、内容恰好充满的边界视口，位移分全部 `0.00000`，错误 / 警告场景 `scrollTop` 不再跳变。脚手架（mock 入口、探针脚本、playwright-core 依赖、vite 实例）已全部删除，`package.json` 仅余用户既有依赖改动。验证：`npm run typecheck`、`npm test` 78/78 通过；未打包。

**差异视图拆分为全局 DiffView 并改红绿语义（2026-08-28 完成）：** 按用户指令把差异渲染实现为独立全局通用组件并统一红绿色设计。拆分：新 `src/components/DiffView.tsx` 是变更行差异渲染的唯一所有者（键 + 旧值 → 新值，等宽字体、近实心底）；`CodePreview.tsx` 收敛为仅文件视图（`DiffPreview` 导出删除）。红绿设计：旧值＝红字 + 浅红 tint 芯片（复用 `--asb-danger`），新值＝绿字 + 浅绿 tint 芯片（新令牌 `--asb-diff-add-text`），移除＝红色加粗「移除」芯片，箭头与文字保证不只靠颜色可读；旧值缺失「（无）」改中性弱化（新 `asb-diff-none`，无旧值即无移除、不染红）。令牌新增明暗两套（`--asb-diff-add-text` / `--asb-diff-remove-bg` / `--asb-diff-add-bg`，移除文字直接走模式自适应的 `--asb-danger`）。接入点：供应商变更预览（`PreviewInspector`）与备份历史「查看差异」（`BackupHistory`）统一改用 `DiffView`。测试：差异用例迁至 `DiffView.test.tsx` 并按红绿语义更新断言（旧值红类、新值绿类、「（无）」中性类），`CodePreview.test.tsx` 收敛为文件视图用例。验证：`npm run typecheck`（除用户 `mock-dev-entry.tsx` 既有错误外零错误）、`npm test` 78/78；Playwright 临时 harness 截图经视觉核验——浅色与深色模式下红 / 绿芯片、红色「移除」、中性「（无）」、弱化键名全部清晰可读，无对比度问题，临时产物已清理；未打包。

**选择框点击的外层蓝环移除（2026-08-28 完成）：** 用户报告所有选择框点击后被一层额外蓝色边框包裹。根因：复选框、单选、滑杆这类输入控件被鼠标点击获得焦点时，Chromium 对其原生恒定匹配 `:focus-visible`（与按钮不同），于是绘制控件（复选框盒、分段药丸、滑块白钮）每次点击都套上焦点环。契约要求键盘焦点必须可见，不能整体删除——改为按输入模式区分：`App.tsx` 新增输入模式跟踪（Tab / 空格 / 方向键按下时在根元素标记 `data-focus-source="key"`，任何指针按下即清除），`base.css` 中三条绘制控件焦点环选择器全部以 `:root[data-focus-source="key"]` 前置门控（复选框盒、分段药丸、滑块钮）。效果：鼠标点击无环；Tab / 方向键 / 空格导航时环照常显示；表单文本输入的点击焦点环不受影响（标准表单行为）。新增 `App.test` 用例锁定标记生命周期（键盘标记、指针清除）。验证：`npm run typecheck`、`npm test` 79/79 通过；未打包。

**全会话「无兼容无过渡无技术债」审计与清债（2026-08-28 完成）：** 用户确立全会话准则后，对本会话全部改动逐项审计。rg 全仓扫描确认：旧类名（`asb-effort-*`）、重复助手（`changeLine`）、前端档位常量（`EFFORT_DETENTS`）、已删契约符号（`common_effort`/`EffortState`/三个取值常量/三个旧错误变体）、实测脚手架（mock 入口、探针脚本、playwright-core）全部清零。唯一真实残留为 `local_state.rs` 的旧数据升格迁移——`StoredCodexOptions` 三个遗留字段、`LEGACY_CODEX_COMMON_KEYS` 表、`legacy_codex_line` 与升格循环、两个迁移测试：其唯一服务对象（在盘 profiles.json）已被该迁移重写，代码已成死过渡路径，整体删除；解析边界同步收紧为 `deny_unknown_fields`（含 serde tag 字段声明），旧形态存储从此响亮报错「旧 Codex 档案模型选项无法解析」而非静默吞字段。既有更早年代的迁移（模式推断、档案级键降格、弃用 Claude 键）非本会话产物，未动。验证：`cargo test --workspace`（130 项）、`cargo fmt`、`npm run typecheck`、`npm test` 84/84 通过（期间偶发失败均为用户并发编辑的瞬时态）；未打包。准则已写入长期记忆。

**备份页新增「打开备份文件夹」（2026-08-28 完成）：** 按用户指令在备份历史面板标题行新增「打开备份文件夹」按钮（与「撤回上一次切换」同入 `asb-panel-actions` 分组，常驻可点、不参与 busy 门控——只读导航动作；记录为空时按钮仍在，可直达目录）。契约：后端新命令 `open_backup_dir` 是该操作的唯一入口——解析 `LocalState::backup_dir()`（应用数据目录 `state/backups`）、按需 `create_dir_all`（空历史也能打开）、经 `tauri-plugin-opener` 的 Rust API（`OpenerExt::open_path`，签名收 `Into<String>`）唤起系统资源管理器；从 Rust 侧调用而非前端 `openPath`，故无需放宽 opener capability 的路径 scope，现有 `opener:allow-open-url` 保持仅 http/https 不变。前端 `client.ts` 新增 `openBackupDir()` 类型化封装，失败经统一 `setError` 呈现类型化错误。`DESIGN.md §8` 操作表同步登记。验证：`cargo check` 通过（仅余 `local_state.rs` `kind` 字段既有 dead_code 警告，非本轮引入；构建期间运行中的 dev 实例占用资源致 os error 32，按既定流程 `taskkill` 后编译通过）；`npx vitest run src/App.test.tsx` 12/12 全量连跑三次稳定（含新增「按钮点击即调 `open_backup_dir`」用例）；`npm run typecheck` 对本轮三个文件零错误（`ConfirmSheet.tsx` 的 key 类型错误为用户并行改动既有，非本轮引入）；未打包。

**时间展示统一组件与探针状态色（2026-08-28 完成）：** 按用户指令三件事：①端点验证结果行以颜色承载结论——可达＝安全绿、不可达＝警示红（新类 `asb-ok-text` / `asb-fail-text`，取既有令牌 `--asb-safe` / `--asb-danger`；文字「可达 · HTTP 状态 · 延迟」/「不可达」同时在场所保不只靠颜色可读）。②时间格式去掉 `（UTC+8）` 偏移后缀——`timeLabel` 只输出 `YYYY-MM-DD HH:MM:SS` 本地时区，`offsetLabel` 助手删除；`vite.config.ts` 的 `TZ=Asia/Shanghai` 测试锚定不变。③时间展示收敛为全局组件：新 `src/components/Time.tsx`（`<time dateTime>` 语义元素）是界面时间渲染的唯一所有者，`timeLabel` 降为格式化纯函数；全部消费点统一接入——概览匹配态三例（`matchLabel` 返回类型放宽为 `ReactNode`）、概览上次切换、备份页上次操作、备份历史表格与恢复确认、探针面板、撤回确认（`ConfirmSheet` 的 `details` 类型放宽为 `ReactNode[]`，键改用索引）。新 `Time.test.tsx` 两条契约：渲染无后缀本地时间 + 扫描测试锁定 `timeLabel` 在 `lib/time.ts` 与组件之外零直接消费者（复用 boundary 测试扫描模式）。验证：`npm run typecheck` 零错误、`npx vitest run` 84/84（20 文件）；未打包。

**开发入口单一化——根治 HMR 窗口闪烁（2026-08-28 完成）：** 用户报告「每次热更新都 build、窗口闪」。`tauri dev --verbose` 实测定位根因：非 `npm run build` 参与热更——Tauri Rust watcher 监听 `src-tauri/`、`crates/`，并行改动 `src-tauri\src\*.rs` 时被判定 `Rebuilding application...` 并重启原生窗口（闪烁即窗口销毁重建，是编译型宿主的固有行为，无热加载可言）。先尝试 `--no-watch` 形态，经用户权衡拍板回到 watcher 模式（A）：Rust 改动自动重建重启，前端改动走 Vite HMR 不闪。无兼容收敛：唯一开发入口 `npm run dev` = `CARGO_TARGET_DIR=target-dev` + `tauri dev`（带 watcher；`target-dev` 与并行实例的默认 `target` 隔离，规避 os error 32 文件锁）；Vite 服务收敛为内部脚本 `dev:frontend`，仅由 `beforeDevCommand` 消费、不是用户入口；本轮临时引入的 `dev:tauri` 别名与旧的独立 `vite` 型 `dev`、历史 `npm run tauri dev` 用法同轮删除，全仓 rg 零残留。`target-dev/` 入 `.gitignore`。验证：新入口真实启动成功——日志含 `Watching src-tauri / crates/*`、9 秒增量编译后 exe 自 `target-dev\debug` 运行、期间前端 HMR 更新正常送达；`npm run typecheck` 通过；未打包。

**「撤回上一次切换」按钮改红色填充（2026-08-28 完成）：** 按用户指令把备份页标题行的撤回按钮从 `asb-btn-secondary` 换为既有的 `asb-btn-danger`（`--asb-danger` 填充 + `--asb-text-on-action` 白字，明暗模式自适应，该类原唯一消费者是确认弹窗的破坏性确认钮）。`DESIGN.md §8` 操作表新增撤回行，并修正「删除提供商」行的「仅在确认流程中」表述以匹配红色现在同样出现在撤回入口的事实。验证：`npx vitest run src/App.test.tsx` 12/12（撤回流程用例按名称定位按钮、不受类名影响）；`npm run typecheck` 零错误（上一轮 `ConfirmSheet.tsx` 的用户并行错误已被其会话修复）；未打包。

**旧聚合存储迁移（2026-09-01）：** 首次启动只将旧聚合档案文件作为迁移源；目标是当前的通用、供应商与历史独立文件。迁移前验证完整旧形状，重复键或不可表示数据整体拒绝；成功时旧文件先隔离，所有新文件写入并校验完成后才删除隔离源，任何失败都还原原文件并删除部分新状态。运行时没有旧格式读取、重置入口或兼容分支。

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

**窗口控制：原生标题栏当日回退，定稿电脑管家式自绘（2026-08-31）：** 上午按用户「不要自己实现、用 Windows 原生、最新样式」指令切换 `decorations: true` 并全量删除自绘实现；用户运行后对照微软电脑管家质询按钮差异，确认互斥事实（电脑管家按钮本就是无边框窗口自绘 / WebView2 WCO，真原生标题栏无任何样式接口，wry 不支持 WCO——wry#1650 / tauri#4531 开放）后拍板「恢复电脑管家式自绘」。本轮完整还原上午删除的全部代码：`tauri.conf.json` `decorations: false`、capabilities `core:window:allow-start-dragging`、`commands.rs` 的 `main_hwnd` 与 `window_minimize` / `window_toggle_maximize` / `window_is_maximized` / `window_close` 四命令（`SendMessageW(WM_SYSCOMMAND)` + `IsZoomed` 通道）、`lib.rs` 注册、`windows-sys` 的 `Win32_UI_WindowsAndMessaging` / `Win32_Foundation` 特性、前端 `WindowControls` 组件与测试（五用例与原契约一致：窗口态最大化钮单方块图形、最大化态镜像双方块还原图标、resize 事件同步、三钮点击分发各一次、卸载停听——图标形态断言锁死 DESIGN.md「最大化钮镜像实时窗口状态」契约）、四枚窗口图标、`client.ts` 五封装（含 `getCurrentWindow` 导入）、`base.css` 窗口按钮样式块、顶栏 `data-tauri-drag-region` 三处、`App.test.tsx` 的 `@tauri-apps/api/window` mock 与 `window_is_maximized` 分支。原生实验期间临时引入的 `apply_window_settings` `set_theme`（服务于原生标题栏主题跟随）随原生标题栏一并删除，`apply_window_settings` 回到仅置顶单一职责。最终契约与 2026-08-28 定案完全一致；`DESIGN.md` 顶栏契约恢复并补记当日往返证据。验证：`cargo fmt --check`、隔离 `CARGO_TARGET_DIR` 下 `cargo test --workspace`（src-tauri 33 过 1 忽略、asb-core 86、执行器 5、切换 16，零警告）、`npm run typecheck`、`npx vitest run` 24 文件 107 项全过、`rg` 确认 `set_theme` / `window-theme-failed` 零残留；未打包（按钮外观需用户在运行实例中验收）。

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

**CI 发布链路与首个 Release（2026-08-31 完成，用户指令「配置 github 的自动构建并实际实现」）：** 打通 tag → 自动构建 → GitHub Release 全链路，补上「检查更新」缺失的分发端。`.github/workflows/windows-package.yml`：`permissions` 升为 `contents: write`，新增末步「发布 GitHub Release」——`if: startsWith(github.ref, 'refs/tags/v')` 门控，用 runner 自带 `gh release create`（`GH_TOKEN: github.token`，非草稿非预发布，`--generate-notes` 自动变更日志）上传 `target/release/bundle/nsis/*-setup.exe`；刻意不引入第三方 release action（供应链面更小），触发器保持既有裸 `push`（分支与标签都会构建，仅标签发布）。`.gitignore` 收口构建垃圾：`target-dev/` 泛化为 `target-*/`（覆盖 check / session-check / update-check 隔离目录），新增 `.zcode/`、`.playwright-mcp/`。实际执行：发布前全量验证（`cargo test --workspace` 隔离 target 全过、`npm run typecheck`、`npx vitest run` 128/128——先前两次全量各挂 2 项 ProviderEditor 系并行构建资源挤占的 5s 超时抖动，单跑与最终干净全跑均全绿、`npm run build` 通过）；暂存 141 文件经密钥模式扫描（唯一命中为测试夹具的环境变量名）后提交 `2c4cccf` 并 push master，打注释标签 `v0.1.0`（与 `tauri.conf.json` 的 `version 0.1.0` 对齐）push 触发构建；两条运行（master + tag）执行完整测试与 NSIS 打包，tag 运行发布 Release。版本语义：已安装 0.1.0 的应用检查更新将得到「已是最新版本」；后续发版流程 = 改 `tauri.conf.json` 版本 → 提交 → 打 `vX.Y.Z` 标签推送。

**会话详情复刻 CC Switch 布局与消息目录（2026-08-31 完成）：** 用户反馈会话内容查看区域太小，指令参考 CC Switch UI 尽量复刻（含「按点查看」），设计语言保持自有。研读 `/tmp/cc-switch-ref` 的 `SessionManagerPage` 定案四项结构：定宽列表 + 详情占满、紧凑头部（单行元信息 + 恢复命令条）、长消息折叠、右侧用户消息目录（TOC）点击跳转。落地（结构与交互契约照抄，视觉全部走本系统令牌）：①布局比例 `minmax(260px,0.72fr)/1.28fr` → `320px minmax(0,1fr)`（1440 窗口实测详情占 1011px ≈ 76%）；②头部紧凑化——原 2×2 元数据网格（会话 ID / 最近活跃 / 工作目录 / 恢复命令）整体删除，改为单行元信息（等宽截断的会话 ID · 最近活跃时间 · 目录名按钮，`title` 悬停显完整路径、点击复制）+ 恢复命令条（等宽单行截断）；三个既有操作按钮（恢复 / 复制恢复命令 / 复制工作目录）与全部状态文案原样保留；③消息条目：超 3000 字符折叠为前 1500 + 省略号，「展开完整内容（约 Nk 字符）／收起」切换（`aria-expanded`）；悬停 / 键盘聚焦显现「复制」按钮，经全局 toast 反馈；④消息目录：提取用户消息为编号条目（两行截断预览），>2 条时渲染，点击 `scrollIntoView({block:'center'})` 瞬时定位并以行动蓝描边 + 路由投影高亮 2 秒——实测发现 smooth 动画会被高亮重渲染打断搁浅在半途，改瞬时跳转（注释记录原因）；窗口 ≥1280px 才显示目录（248px 定宽、自滚动），窄窗口不显示、窄屏堆叠模式沿用整面板滚动。明确不采纳参考项目项：删除 / 批量删除（产品边界）、`sourcePath` 展示与复制（本产品契约绝不向前端暴露源路径）、多客户端目录、FlexSearch 消息内高亮、虚拟化列表（本轮不加）。样式全部落在 `base.css` 会话块（元信息行 / 命令条 / 目录 / 折叠 / 复制 / 高亮），删除孤儿 `.asb-session-meta` 网格全套规则；对话区新增「对话记录」标题 + 计数徽章。测试：既有 6 项全过（按钮名与状态文案未动），新增 3 项锁定新契约——长消息折叠往返、目录跳转定位与高亮（含 scrollIntoView mock）、单条消息复制 toast。验证：`npx vitest run` 29 文件 133 项全过、`npm run typecheck` 干净、浏览器实测（Tauri 桩 + 48 条消息 / 含 1 条超长）：跳转 scrollTop 5813 精确居中（目标距视口顶 84px）、折叠展开 4200 字符、页面零滚动。未打包。

**设置页界面字体选择与 Noto Sans SC 默认字体（2026-08-31 完成，用户指令参考 ZiChe 的选择时预览）：** 设置页「外观」组新增「界面字体」行。契约：`AppSettings` 增第六必填字段 `interfaceFont`（默认 `"Noto Sans SC"`，`local_state.rs` 的 `DEFAULT_INTERFACE_FONT` 为唯一所有者）；校验拒绝空串、首尾空格、引号 / 反斜杠 / 控制字符与超 64 字符（`set` 前与文件读取后都验证）；唯一迁移路径 = 恰好上一版完整五字段对象原子补入默认字体，四字段旧形态归入明确拒绝（无第二步兼容）。枚举：新模块 `src-tauri/src/fonts.rs` 用 `GDI EnumFontFamiliesExW`（`DEFAULT_CHARSET`、跳过 `@` 竖排、`BTreeSet` 去重排序）枚举已安装字族，命令 `list_system_fonts`；`windows-sys` 加 `Win32_Graphics_Gdi` feature。前端：`FontPicker` 组件（交互参考 ZiChe、视觉走本系统 select 令牌）——触发器以当前字体渲染字体名，菜单含搜索框，选项 = 打包默认 + 系统字族去重，每个选项以该字体渲染「字体名 + 中文 Aa 012」，当前项右置勾选，方向键 / Home / End / Escape / 外部点击可达；选择经完整对象保存并即时生效。字体应用链：`lib/font-family.ts` 拥有字体名 → 带引号 CSS 值的转换；`tokens.css` 两文字栈前置 `var(--asb-font-user)`（令牌默认即 `Noto Sans SC`）；`useAppSettings` 在设置加载后覆写该变量（与 `data-theme` 同一应用点），代码栈不变。默认字体落地：`@fontsource/noto-sans-sc` 打包 400/600/700 三字重（OFL，`main.tsx` 引入，`unicode-range` 子集懒加载；fontsource CSS 同时引用 woff2+woff，dist 增量约 25MB）。测试：Rust 48 过 1 忽略（新增枚举 1 项、契约 3 项更新）；前端新增 `FontPicker` 8 项、`AppSettingsForm` 1 项、`client` 1 项、`App` 集成 1 项（完整 invoke 载荷 + CSS 变量断言），并修补 6 个用通用兜底 mock `get_app_settings` 的旧集成测试夹具。验证：`cargo fmt`、`cargo test --lib`（隔离 target）、`npm run typecheck`、相关 vitest 单文件全绿；`CodexResetPanel.tsx` 的 3 个类型错误为用户并发进行中代码，本轮未触碰。未打包。

**设置页固定失败态（2026-08-31 完成，用户指令「前端也需要显示固定UI」）：** 起因是用户本机 `state/settings.json` 残留 8-28 的单字段旧形态，被严格契约每次读取拒绝，设置页永远停留「加载中」且原因只在启动 toast 里闪现（该残缺文件已删除，其唯一值等于默认值）。本轮把失败态做成固定 UI：`useAppSettings` 新增 `loadError`（失败原因文本）与 `retryLoad`（重载 nonce 重新拉取，成功即清空）；`SettingsPage` 三态渲染——已加载出表单 / 失败出 `role="alert"` 警示行「设置加载失败：{原因}」+「重试」次级按钮 / 其余才显示「加载中」；全局错误通知照常，页面文字是持久来源。新增 App 集成测试 1 项（先拒后成的重试链路 + 失败时不渲染表单与加载占位）。验证：`npm run typecheck` 零错误（用户并发中的 `CodexResetPanel` 类型错误已由用户自行修复）、`npx vitest run` 全量 151 项通过。未打包。

**设置页 UI 审查与修复（2026-08-31 完成，用户指令「审查研究后无兼容无过渡无技术债修复」）：** 经临时 mock 入口（真实 App + 桩化 `__TAURI_INTERNALS__.invoke`，vite 1421）浏览器实测设置页常规 / 字体菜单开 / 失败 / 深色四态，DOM 实测定窗三缺陷并当场修净（无兼容层，harness 用毕即删，孤儿 vite 进程按命令行归属 taskkill）：①字体行触发器 216px 高——chevron `<svg>` 直接携带 `asb-select-chevron` 类，而尺寸规则 `.asb-select-chevron svg` 只作用于包裹元素内部，svg 无宽高退化 300×150；修复 = 与 `Select` 同构的包裹 span（不新增 CSS）。②四个设置分组间距实测 0px——`.asb-app-settings` 容器在 base.css 中无任何规则，且表单根与「软件更新」是两个并列根、父级 `.asb-panel` 为普通 block；修复 = 给 `.asb-app-settings` 立法（flex column + `--asb-space-4` 24px，§5 结构间距），并收敛 `SettingsPage` 为单一容器根（表单 / 失败行 / 更新组都是其直接子项），三处组间距复测 24px。③失败态裸奔在面板上——改为同款紧凑实心信息行（`role="alert"` + 标签行「设置加载失败：{原因}」+ 说明行默认行为 +「重试」次级按钮）。附带修掉字体选项行的勾选挤占：每行渲染固定宽度勾选槽位（空槽占位），样本列右缘全列对齐（实测 1148/1174 恒定）；菜单底 686px < 800px 视口不再裁切；深色模式抽查正常。视觉模型曾误报「软件更新信息行裸奔 / 按钮孤立」，DOM 实测其行表面与边框俱在，判定为 0 间距挤压下的误读，未改。复测证据：触发器 41px、组间距 [24,24,24]、失败行有 bg+border 且距更新组 24px、样本右缘全列恒定。验证：`npm run typecheck` 零错误、相关 vitest 单文件 43 项 + 全量 151 项全过；DESIGN.md §8 已同步（间距唯一所有者、固定勾选槽位、失败行结构）。未打包。

**设置页布尔行复刻为渐变开关（2026-08-31 完成，用户指令参考 spiralcoder 渐变按钮搬运）：** 「窗口始终置顶」「启用硬件加速」由复选框改为开关。`/tmp/spiralcoder` 参考克隆已被临时目录清理（仅剩 git 二进制索引），渐变按钮实现以本仓库 2026-08-28 的既有 1:1 移植为基准（`--asb-gradient-button` 青→蓝→粉 88° 五停 + `--asb-gradient-button-glow(-hover)` 内嵌蓝光晕 / 悬停白光增强）。新组件 `Switch`（`role="switch"`，隐藏原生 input 保留键盘读屏，结构与 `Checkbox` 同源）：44×26 药丸轨道 + 18px 白滑钮（复用 `--asb-slider-thumb-bg`）滑动 220ms；开启轨道 = 移植渐变 + 蓝光晕、悬停白光增强，关闭轨道 = 画布底 + 发丝边、悬停边框转行动蓝；按下 0.98 缩放；键盘圆环走既有 `data-focus-source="key"` 契约；减少动态效果令 motion 令牌归零自动瞬时。`AppSettingsForm` 两行换装，`Checkbox` 保留通用设置 / 发现页复选框语义不混用；测试角色断言 checkbox→switch 共 5 处，新增 `Switch.test.tsx` 4 项。浏览器实测（mock harness）：开启轨道渐变实测 `linear-gradient(88deg, rgb(82,225,255)…)`、滑钮 18px 位移、点击即时换装；悬停 box-shadow 实测白光 16px + 蓝光 14px；深色下关闭轨道为 `rgb(7,23,37)` 深流蓝（早前误报白色为手动改 data-theme 被设置效果重置的测试伪影，经真实「深色」段路径复测排除）。中途踩中令牌所有权测试（base.css 裸 `#ffffff`），改用 `--asb-slider-thumb-bg` 修复。验证：`npm run typecheck` 零错误、vitest 全量 163 项中 160 过（`ProviderEditor` 2 项单跑全绿，为既有全量负载抖动）+ 令牌测试过；harness 与截图已删净，孤儿 vite 进程按命令行归属 taskkill。未打包。

**开关行布局按操作逻辑定稿（2026-08-31 完成，用户指令「文字在左，按钮在右」）：** 开关行从「[轨道+文字] 左、说明文字右列」翻转为与分段 / 字体行同构的结构——标签与说明在左列（`asb-app-setting-copy`），`Switch` 控件在行右缘。`Switch` 组件不再绘制可见标签：可及名改经输入 `aria-label` 传入，只绘制轨道本体；命中区由组件自身的 40px 最小高度与纵向内边距承担；窄屏（<680px）开关随分段控件 `justify-self: start`。`Switch.test.tsx` 相应改为断言「可及名存在而可见文字不存在」。浏览器实测（mock harness）：两行文字列右缘 689、开关 705–749 与行右缘 765 对齐，说明文字入左列，逐行一致；harness 与孤儿 vite 进程已清理。验证：`npm run typecheck` 零错误、Switch/AppSettingsForm/App 相关测试 31 项全过。未打包。

**设置页移除全部说明性副文（2026-08-31 完成，用户指令）：** 设置行不再携带描述行——「界面主题 / 界面字体 / 动态效果 / 点击关闭按钮时 / 窗口始终置顶 / 启用硬件加速」各行只余标签与控件，`SegmentSetting` 的 `detail` 字段与全部选项描述、字体行 / 开关行 / 软件更新未检查态的说明文字一并删除（无兼容残留）。保留两处非描述性文字：软件更新**检查后**的「当前版本 · 检查于」（结果状态，仅在检查后出现）与设置读取失败态的恢复指引（错误恢复信息）。测试同步删除 3 处副文断言、未检查态改为断言无版本文字。验证：`npm run typecheck` 零错误、AppSettingsForm / UpdateSection / App / Switch 相关 36 项全过。未打包。

**浏览器真实开发入口（2026-08-31 实现完成，待浏览器验收）：** 按用户要求删除 `src/dev/web-preview.ts`、其样例数据与内存 IPC 桥；`npm run dev` 改由真实 `tauri dev` 承载后端并打开 `http://127.0.0.1:1420` 的浏览器页，debug 专属 `src-tauri/src/dev_api.rs` 仅监听 `127.0.0.1:1422`，接受 Vite 代理的 JSON RPC 并逐项派发到既有 Tauri 命令。故配置状态、通用设置、档案、切换、备份、发现、会话和应用设置均复用真实状态、解析和事务；浏览器没有第二实现或模拟数据。浏览器页标注「浏览器开发 · 本机后端」，隐藏原生窗口三钮并保留 F12；`npm run dev:desktop` 保留同一后端的可见原生壳。写入仍按现有确认、锁、备份、原子替换和验证路径执行。验证：`cargo test -p agent-switchboard dev_api` 2 项、`npm run typecheck`、`src/api/client.test.ts` 与 `src/boundary.test.ts` 共 10 项通过。浏览器实际验收暂被并行中的 `src-tauri/src/local_state.rs:107` 编译错误阻断（`'\\t'` / `'\\r'` / `'\\n'` 非法字符字面量），本轮未改动该文件。

**界面点击后材质丢失修复（2026-08-31 完成）：** 保留 `AppSettings.hardwareAcceleration` 作为仅桌面 WebView2 启动前读取的性能设置，不将其改造成浏览器页的自动降级或兼容开关。视觉运行路径改为唯一实体材质：`tokens.css` 删除全部 `--asb-blur-*`、`--asb-glass-*`、饱和度和后备令牌，新增轨道 / 菜单 / 抽屉实体面；`base.css` 删除环境伪层与所有 `filter`、`backdrop-filter`、`@supports` 后备分支，以及路由、菜单、提示、抽屉、通知的缩放 / 位移入退场。顶栏与 `DualRelay` 改消费实体轨道面；按下反馈不再缩放按钮、复选框、开关或滑块，只有表达开关真实状态的滑钮位移保留。Toast 同步移除 300ms 退出状态、计时器和渲染分支，关闭即时移除；令牌测试锁定不存在旧材质、背景采样和退出类名。`DESIGN.md` 已改为同一契约。验证：改动完成时 `npm run typecheck` 与 `Toaster`、令牌定向测试 9 项通过；随后并行工作树新增 `runtimeLogLevel` / `LogsPage` 契约未同步，导致全量构建与全量测试分别出现无关类型错误和超时，未改动这些并发文件。浏览器自动化工具在本机不可用，需在当前 Edge 开发页手动连续点击验收；不声称已完成该黑盒验证。

**浏览器开发页重复打开修复（2026-09-01 完成）：** 根因是 `src-tauri/src/lib.rs` 在每次 Tauri `setup` 都调用系统 opener；Rust / Cargo workspace 热更新会重启后端进程，因此同一 Vite `1420` 地址被反复打开。契约迁移为单一所有者：Rust `setup` 只启动真实 debug 后端并隐藏承载窗口，彻底删除开发页 opener；`src-tauri/src/dev_api.rs` 增加同源限制的无数据 `GET /health`；持久的 Vite 开发进程通过 `src/dev/vite-browser-launch.ts` 直接、安静地探测回环健康地址，返回 `204` 后只调用一次公开的 `ViteDevServer.openBrowser()`。进程内 `idle / waiting / opened + generation` 是唯一启动状态：Vite 配置重载时新 server 接管未完成的等待、旧任务退出，server 关闭则取消等待，已打开后任何重载都不再打开；不写环境 claim、不遗留轮询。Vite 与 Rust 两端均只把精确 `ASB_WEB_DEVELOPMENT=1` 视为 Web 开发，`dev:desktop` 删除该变量，既不探测也不隐藏原生窗口或打开浏览器。端口、业务 API 代理、真实命令桥和 Rust 热重载均保持唯一原路径，无包装脚本、兼容分支或新依赖。验证：`cargo fmt --all -- --check`、隔离 target 的 `cargo test -p agent-switchboard --lib` 89 过 1 忽略、隔离 target 的 `cargo check -p agent-switchboard --release`、`npm run typecheck`、`npx vitest run src/dev/vite-browser-launch.test.ts src/boundary.test.ts` 6/6、`npm run build` 通过；真实 `npm run dev` 中修改 Rust 文件后，后端 PID `31148 → 11072` 且日志新增 `appStarted`，Edge 页面数保持「另外 5 个页面」不变，证明后端确已热重启而浏览器未重复打开；验收后只终止本轮启动的开发进程，`1420` / `1422` 均已释放。

**浏览器开发回环地址收敛（2026-09-01 完成）：** `src-tauri/tauri.conf.json` 的 `build.devUrl` 是浏览器开发入口唯一所有者，规范值为 `http://127.0.0.1:1420`；Vite 从该字段解析监听 host / port 与健康探测 Origin，Rust 从 Tauri 运行时配置取得同一 Origin 并传给 debug API 做精确相等校验，浏览器启动参数、测试和文档同轮迁移，不保留第二主机名、重定向、别名或兼容接受路径。边界测试加载实际 Vite 配置，验证监听值派生自 Tauri 所有者；Rust 测试锁定 Origin 必须精确匹配；全仓源码搜索确认旧主机名零残留。验证：`npm run typecheck`、`npm run build` 通过且无配置加载警告，`npx vitest run src/dev/vite-browser-launch.test.ts src/boundary.test.ts` 7/7，隔离 target 的 `cargo test -p agent-switchboard dev_api --lib` 4/4，`cargo fmt --check --manifest-path src-tauri/Cargo.toml` 与 `git diff --check` 通过；保留用户当前开发进程并实测 `1420` 与 `1422` 均只监听 `127.0.0.1`，经 `http://127.0.0.1:1420/api/health` 代理访问返回 `204`。

**编辑器连通检测改为展开/收回（2026-09-01 完成）：** 按用户指令将供应商编辑器「主模型」按钮区的「检测连通」从「每次点击都重新探测且结果无法收起」改为开合开关：无结果时点击探测并展开；结果（含错误）显示时按钮变「收起结果」，点击收回；收回后再点击重新探测并展开（保留同地址重试能力）。可见性由 `ProbePanel` 本地 `open` 与探测结果的与运算派生，url 变更经 `useEndpointProbe` 既有的结果清空自动隐藏，无兼容分支；按钮补 `aria-expanded` / `aria-controls`（`editor-probe-feedback`），两态文案等宽避免布局抖动；供应商列表行内连通图标按钮未在指令范围内，行为不变。验证：`npx vitest run src/components/ProbePanel.test.tsx src/components/ProviderEditor.test.tsx` 22/22（含新增展开/收回/重跑、错误收回重跑、展开后换地址隐藏用例）、`npm run typecheck` 零错误。
