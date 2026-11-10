# Agent Note: 会话操作按客户端归属并支持 Codex 原生删除

Status: implemented

## Problem

会话页头同时显示 Claude 来源、Codex 归属和仅处理 Claude 的清理操作，全部及收藏视图也暴露无关按钮。Codex 原生登记会话的删除被直接禁用，用户无法从已有单条或批量删除入口处理这些记录。

## Decision

SessionManager 持有视图、筛选和唯一的 SessionAction 弹窗状态，移除三个可同时为真的打开标记。SessionPageActions 按历史页签与客户端筛选共同渲染操作入口，通过一个类型化动作回调选择来源、清理或归属：Claude 仅显示来源和清理，Codex 仅显示归属，全部及收藏不显示客户端专属操作。客户端筛选与刷新继续使用原位置。

切换历史客户端筛选时同时清空详情、待确认项与批量选择，避免 Claude 页保留 Codex 详情或跨客户端删除目标；再次点击当前筛选保持上下文。从收藏打开原会话时，如当前历史客户端筛选不包含该会话，则先切到该会话的客户端。

所有 Codex 会话统一通过当前已验证的本机 Codex CLI 执行 `delete --force <UUID>`，不按是否登记保留直接删除文件的分支；原生登记、正文、文件及子代理清理由 Codex 拥有，应用不维护另一套 SQLite 删除 SQL。前端单条及批量删除共同使用 delete_sessions，移除没有产品消费者的 delete_session / deleteSession 命令链。前端仍只提交客户端与会话 ID，现有确认弹窗先说明不可恢复及 Codex 子代理的连带删除。应用持有现有客户端启动租约，拒绝未恢复的供应商整理；命令固定本次读取的 CODEX_HOME 和 sqlite_home，在本地配置目录启动，并移除 exec-server URL 与 Noise 环境地址及凭据覆盖，不传远程入口。

SessionSource 的 native_database 表示原生元信息来源及读取时的登记位置，不决定删除执行方式。codex_deletion 唯一拥有命令范围、执行、结果核验及已删除会话的整理数据清理；执行前核对登记位置，未登记原件必须仍位于当前 Codex 的活动或归档目录，来源变更先刷新。每次执行后只核验一次登记、分页正文和已发现原件，结果直接交给批量协调器，不再用 source_removed 的布尔推断或 unwrap_or(false) 隐藏核验错误。已知关联会话只在确认移除后清理整理数据、移出详情和批量选择，仍存在的独立分叉保留；收藏不级联。启动进程仍复用 client_software 的路径及有界输出函数，只向其他模块暴露实际需要的函数。

原生命令契约对照 [Codex CLI](https://github.com/openai/codex/blob/rust-v0.155.0/codex-rs/cli/src/main.rs)及[会话删除命令实现](https://github.com/openai/codex/blob/rust-v0.155.0/codex-rs/tui/src/session_archive_commands.rs)：--force 只接受 UUID，原生子代理随主会话删除。应用确认替代原生命令的交互确认，不新增 CLI 参数或数据库删除契约。

## Related decisions

- [操作层级与长内容布局](../bug-fix/2026-09-24-session-operations-layout.md)：部分重叠，保留更多菜单、批量栏与确认入口，补充页头操作归属。
- [完整本地会话读取](../architecture/2026-10-16-complete-local-session-usage.md)：部分重叠，普通读取继续只读，本篇取代原生删除被禁用的边界。
- [会话供应商归属](2026-11-09-codex-session-provider-transfer.md)：部分重叠，仅收窄其入口到 Codex 历史筛选，并沿用未恢复操作阻止其他原生修改的边界。
- Claude 保留库、workflow 转换及派生读取索引的所有权不变；其余关键词命中没有同一原生删除决定。

## Alternatives considered

- 直接事务删除原生 SQLite 行，可不依赖 CLI 安装；但同时承担原生外键、分页正文、引用和写入者锁的演进，容易出现原文已删而原生列表残留，因此交由原生删除命令。
- 仅解除按钮禁用并删除原件，改动最小，原始文件也能立即释放；但登记和持久正文仍会重新显示会话，且可能破坏引用，因此不采用。
- 为未登记文件单独保留直接删除，可处理当前 CLI 不认识的记录且不依赖安装；但同一 Codex 身份由两条写入路径处理，原生引用、登记变化及结果核验会继续分叉，因此统一使用当前原生命令，不支持时明确拒绝。
- 全部视图继续显示两端按钮，发现入口最直接；但仍要求用户辨认无关操作，且清理容易被误认为覆盖全部客户端，因此只在明确客户端范围显示。

## Verification

用户明确授权构建、测试及隔离删除验收后，`npm run build`（含 TypeScript 检查）通过；`cargo build --offline --locked -p agent-switchboard --features desktop-e2e --bin agent-switchboard` 通过。会话模块 Rust 测试 145 项通过、0 失败，1 项 process_driver 保持忽略并由黑盒父测试在独立进程中调用；读取队列 Node 测试 4 项通过。构建发现转换测试仍调用被移除的 remove_source，已更新为唯一删除链并复核结果；移除测试与非测试构建各一处无用导入。

浏览器页面连接真实 Tauri 本地后端，使用独立端口和应用标识；复制源码仅调整测试端口、标识及 Vite 文件访问和监听范围，业务文件与工作区一致。CODEX_HOME、CODEX_SQLITE_HOME、Claude、应用状态及浏览器配置全部位于 `.tmp/session-acceptance-20261110-6oMdCV/desktop`。当前官方 Codex CLI 0.155.0 迁移生成真实原生库，并执行实际删除，没有模拟删除命令。未连接真实客户端配置或真实会话。

六种历史/收藏与全部/Codex/Claude 组合的按钮及三种操作弹窗通过。验收复现并修复切换客户端残留详情和批量目标，复测清空通过；从收藏定位跨客户端原会话时切换到匹配的客户端。Codex 和 Claude 单条、批量取消不发删除请求；两端单条和批量删除、Codex 仅原生正文记录、未登记原件、主会话与子代理连带删除均通过。锁住隔离文件后，批量一项成功一项失败，失败项保留选择与错误，解锁重试成功。

16 条夹具实际删除 12 条，刷新后剩余 4 条保留会话；核验原件、原生登记及四种分页正文表，Claude 保留库同步删除。两条收藏仍可阅读；4 条保留会话原件及虚构凭据哨兵共 5 个文件哈希保持一致。没有页面 JavaScript 异常。报告与截图见隔离目录中的 ACCEPTANCE.md、browser-tabs.json、browser-delete.json、browser-delete-errors.json、browser-final.json。原生 WebView2 画面没有直接自动化验收；本次覆盖浏览器开发桥到真实 Tauri 后端及原生 CLI，不代表真实会话或其他平台验收。

## Consequences

页头不再同时暴露两端的操作；在全部视图发现客户端专属动作需要先选择客户端。Codex 删除复用已有单条及批量确认，CLI 拥有原生结构演进，代价是所有 Codex 删除均依赖当前 CLI 支持该命令；缺失、过旧或原生不认识的记录显式失败。删除不可恢复，相关子代理一并删除；原生引用或活动写入限制由 CLI 报告。直接删除 Codex 原件、重复的单条命令链、重复结果核验与吞掉核验错误的旧路径全部移除；没有兼容别名、第二套 SQL 或失败回退。隔离执行覆盖当前 Windows 环境，不替代原生窗口及其他平台验收。
