# Agent Note: 修复流程解锁版本前缀基址死锁

Status: implemented

## Problem

[版本前缀拒绝](2026-10-13-anthropic-version-prefix-base-rejection.md)落地后暴露出一个死锁：含 `/v1` 结尾 Anthropic 基址或自定义端点候选（CC Switch 导入的 OpenAI 风格地址，如 `…/compatible-mode/v1`）的存量档案会让整个配置快照校验失败，UI 全部显示「未读取」，用户在界面里没有任何改这个地址的入口；而 `repair` 只补缺失的 automatic 键、不修非法值，暂存后跑同一套全量校验，被同一条规则拦下。结果是「一键智能修复」永远修不了由校验规则本身造成的锁死——旧笔记预设的出路「用户改为服务根地址」在产品里不存在。

剥除函数还必须遵守普通保存校验的 URL 输入边界：直接调用 `Url::parse` 会归一化空白、控制字符与反斜杠，不能据此把这些格式错误当成单纯的版本前缀问题。修改 URL 前也必须取得独立路径，不能在持有 `url.path()` 切片时可变借用同一个 URL。

## Decision

主地址规则继续由本篇拥有；触发方式和候选处理由[启动准备与自动修复](../feature/2026-11-11-automatic-application-data-preparation.md)部分接管，不增加兼容路径。

- **产生源已封死（无兼容）**：所有落盘边界执行与读取一致的校验——`write_provider_file`（Claude/官方通用存储）、`import_provider`（CC Switch 与 SQL 导入）、Codex 严格存储的 `write`/load 都先 `validate()` 再写盘，非法 URL 无法再进入存储。用户的坏文件是 10-13 规则落地前写入的存量，不存在新的产生路径，因此也不需要任何写入侧兼容或宽限分支。
- **修复路径（`repair_stage`）是唯一允许改写存量 URL 的地方**，且只在语义无歧义时动手：
  - 服务基址以 `/v1` 段结尾（Anthropic 上游 + 非 `isFullUrl`；Claude 通用档案与 Codex 严格档案同规则）：只剥除一个末尾版本段，保留代理前缀，并确认结果通过 Anthropic 服务根校验；请求构造随后追加 `/v1/messages` 或 `/v1/models`。剥除逻辑归属 `asb-core::endpoint::strip_anthropic_version_prefix`；它与 `parse_base` 共用 `parse_service_base` 的原始输入校验和 `version_prefix_root` 的末段判定，不另建宽松解析路径。剥除后的路径先复制成独立字符串，再修改 URL。
  - `customEndpoints` 中带 `/v1` 段的候选此前采用删除以解锁整个存储，理由是候选不能安全重定向且完整备份可找回。自动执行后该删除决定由新篇取代：整次修复拒绝并保留全部候选，不能在无人确认时改变连接含义。主地址的可证实转换仍由端点所有者校验。
  - `isFullUrl` 模式与 OpenAI 系上游不碰——`/v1` 是其声明的 API 根的一部分。
  - 含首尾空白、控制字符、反斜杠、凭据、查询或片段的地址，以及剥除一次后仍非法的地址，不自动归一化或丢弃；原有暂存校验返回具体错误，整个真实存储与原文件保持不变。
  - 修复实现走类型化契约（`ProviderFile` / `CodexProviderFile` 解析→改字段→回写），路由字段的线格式拼写只有 serde 一个所有者；两套文件契约都是 `deny_unknown_fields`，类型化往返不丢字段。
- 保存、导入、诊断、请求路径仍然拒绝、不猜测：旧笔记的核心决定不变，本篇只给 repair 这一条已经备份、已经报告（`repaired_files`）的路径开例外。

## Alternatives considered

- 读取时跳过或放宽坏档案校验：最快解锁 UI，但违背 fail-closed 决定，坏数据静默留在库里，后续切换/请求照样炸。
- 对 custom endpoint 同样剥除 `/v1`：能够保留已保存的候选，并与主地址采用同一种操作；但候选可能来自其他协议的 API 根，`…/compatible-mode/v1` 剥成 `…/compatible-mode` 不能证明其支持 Anthropic 请求，修完即悄悄改路由。
- 放宽 `validate_connection` 对端点候选的校验：候选会经 `endpoint_variants` 变成 `upstream_base_url` 拼协议路径，放宽等于复活 `/v1/v1/messages` 双重版本请求，正是旧笔记要防的。
- 只复制路径解决借用冲突：改动最少，也能解除编译阻塞；但保留了 repair 独立调用 `Url::parse` 的宽松入口，格式错误仍会被静默归一化，因此同时复用普通保存的原始 URL 校验。

## Consequences

- 收益：仅含已允许主地址差异的档案由原生准备自动恢复读取（Claude 与 Codex 两套档案同规则）；修复动作有备份、有文件级报告，用户可审计；非法 URL 自 10-13 起无法再写入，本修复只消化存量。
- 代价：含义不明的端点阻止整个目录自动启用，全部原件保留；「不在请求路径猜测」的立场边界现在依赖「仅 repair 且必有备份」这条纪律维持，后续给 repair 加新规则时必须同样满足语义无歧义 + 可恢复。
- 验证：`repair/upgrade_tests.rs` 集成测试覆盖 Claude base 剥除、候选拒绝与保留、备份保留、二次修复空报告、full-url 与 OpenAI 反例不动、Codex 严格档案同规则，以及混有不可修复 URL 时全部原文件不变；`endpoint.rs` 契约测试同时断言普通入口拒绝旧地址、修复结果可生成请求与模型列表地址、其他格式错误仍拒绝。本次用户授权的 0.3.3 构建已通过前端类型检查、生产构建与 Rust release 编译；未执行自动化回归测试，测试用例与真实客户端修复仍待执行验收。

## Related decisions

- [Anthropic 服务根地址拒绝版本前缀](2026-10-13-anthropic-version-prefix-base-rejection.md) 拥有 URL 语义与拒绝规则本身；本篇是其在修复路径上的唯一例外，不改变其余各入口的行为。
