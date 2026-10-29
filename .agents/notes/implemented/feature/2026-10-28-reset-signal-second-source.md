# Agent Note: 重置信号第二公开源（codex-reset.com 聚合 + 源页签）

Status: implemented

## Problem

「Codex 重置信号」面板此前只有一个公开来源（Codex Runway 的 status.json feed）。用户要求接入第二个独立公开监测站 codex-reset.com 的四个端点（`/api/forecast`、`/api/timeline`、`/api/status-history`、`/api/feed`），并在 UI 以「源1、源2」区分来源；显示内容由本仓库设计。

## Decision

外部 HTTP 载荷只按 codex-reset.com 当前实际的 snake_case 字段反序列化；camelCase 仅属于应用内部归一化、缓存和界面契约。四个端点使用真实字段形状的固定回归夹具，错误保留端点与具体缺失字段，不接受驼峰别名或双格式读取。外部载荷、归一化状态及其校验按职责各自拥有模块。

codex-reset.com 整体作为**一个来源（源2）**，四个端点是同一监测站的四个切面，在 Rust 侧一次检查内全部读取、全部成功才产出快照；面板内新增共享 `Tabs` 源页签（「源1 · Codex Runway」「源2 · Codex Reset」），页头「刷新重置信号」一个动作并行刷新两个来源，错误与新鲜度按来源独立报告。

- **后端契约**：源2 聚合读取由 `codex_reset/hub.rs` 拥有，外部字段由 `hub/payload.rs` 拥有，归一化状态及缓存校验由 `hub_status.rs` 拥有。四个端点固定 URL，经 `probe::http_get` 读取，归一化为 `CodexResetHubStatus`（deny_unknown_fields + `validate_cached`，与源1 同等严格）；两源使用同一个 `CodexResetRead<S>` 信封，`CodexResetHubRead` 是其源2 实例。缓存独立成对（`load/save_codex_reset_hub_cache`，文件 `codex-reset-hub-cache.json`），命令成对（`get_cached_codex_reset_hub_status` / `check_codex_reset_hub_status`）。
- **源2 展示**（复用源1 的 board/facts/badge/post/warn/source-note 词汇）：预报板（24/48 小时概率与区间、信心、常见到账窗口、节奏、等待对比、监测站自述中文说明、Tibo 最新信号帖）；重置时间线表（最新 40 条 reset/credits 事件，含原帖链接）；OpenAI 平台状态（五个面 normal/degraded 状态胶囊 + 近 90 天故障统计 + 可折叠故障记录表，最新 12 条）；重置卡到账记录表（如有）。
- **前端状态**：`useCodexResetSignal` 返回 `{ runway, hub, loading, readStatus }`，两源由参数化的 `useResetSourceRead` 驱动，行为与原单源 hook 逐字段一致；头部状态行按源显示缓存/实时。
- **裁剪**（只存 UI 展示的事实）：forecast 的 model/backtest/evidence/range_aria/tease 系字段、timeline 的 rhythm HTML 图表与 milestones、feed 的原始推文流（29 条）与 profile 均不入契约；endpoint `stale` 标记折叠为一条 `source_warning`。

## Alternatives considered

- **四个端点各自作为独立来源（源2–源5）**：最强理由是单端点故障不拖垮其他数据，且与用户「加入新源：A、B、C、D」的列举字面一致。否决：四端点同站同源、`updated_at` 同步刷新，forecast 的证据 id 直接引用 timeline 事件，拆开是人为割裂；还会把页签撑到五个、每个都要独立的空/错/新鲜度状态，复杂度翻倍而信息不变。
- **部分成功部分展示（per-endpoint 警告）**：最强理由是单端点失败时其余数据仍可见。否决：四个 Optional 区块组 × 错误态的组合爆炸，且同站端点同时失联是常态场景，全成全败 + 指名端点的错误串与源1 的严格风格一致。
- **在 UI 里平铺两源数据（无页签，叠加展示）**：最强理由是少一次点击。否决：两源对同一问题给出不同口径的答案（Runway 给公告型 yes/no + 置信度，codex-reset 给概率区间），叠加会诱导用户把两者相加或二选一误读，违背 DESIGN.md「不将不同来源的数据相加或推断」。

## Consequences

回归用例覆盖四个端点的下划线载荷、归一化后的 camelCase 序列化、缓存完整往返与校验，以及错误的驼峰外部字段被明确拒绝。外部载荷、聚合读取、内部状态分别归属 `hub/payload.rs`、`hub.rs` 和 `hub_status.rs`，没有旧格式读取分支。用例尚未运行，笔记结构和代码引用另行静态校验。

收益：重置信号有了第二个独立佐证源，概率区间、平台状态与故障史是源1 没有的事实；两源契约、缓存、错误面完全对称，后续第三源按同一模式接入。代价：一次刷新从 1 个请求变 5 个（源1 一个 + 源2 四个），任一源2 端点失败则源2 整体报错；codex-reset.com 字段无 schema 版本号，上游加字段不影响读取，但改字段类型会使源2 报错直到本仓库跟进。验证：本批按工作区规则未运行 typecheck/test（用户未要求），正确性依据为与源1 逐点同构的代码路径与 `npm run verify-notes`；四端点真实载荷于 2026-10-28 实测采样后设计契约。展示与来源归属事实见 [桌面工作区视觉与交互所有权](../architecture/2026-10-13-desktop-workspace-uiux.md) 与 DESIGN.md 用量监控条目。
