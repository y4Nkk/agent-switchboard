# Agent Note: 当前推理契约与供应商请求处理的单一所有者

Status: implemented

## Problem

第一轮边界补强仍把独立推理输出放在思考开关枚举中，配置无法同时表达真实思考开关和输出格式。Moonshot 的两个 schema 遍历器覆盖不同关键词，普通请求与压缩请求又经过不同处理入口。仅修正其中一个调用点会继续产生分叉。

## Decision

- 保留现有网关和中间表示。Chat 推理配置使用唯一当前格式：思考开关、独立输出、力度参数分别声明。删除内部 `thinkingParameter=reasoningSplit`，不设置别名、双读或缺失新字段的默认填充；真实档案不由开发过程自动改写。
- 所有 Codex 模型请求通过已有 `codex::request::prepare` 完成客户端参数处理后，再统一执行供应商方言处理。Claude 在账户和覆盖参数准备完成后使用同一供应商处理入口。普通 HTTP、WebSocket、辅助 Responses 和压缩共用该入口。
- Moonshot 的 schema 遍历、主机判定和 `$ref` 改写只保留一份，保留所有已支持的 schema 关键词与原有 allOf；非法 allOf 明确失败，不能覆盖约束。
- 已确认的 Azure/Bedrock Chat 输出长度字段及 MiniMax 独立推理输出在同一发送入口处理。DeepSeek 思考工具历史缺失原始推理时明确诊断，不填空格、不删除推理来制造成功。
- 回放区分不存在的推理与上游确实返回的空推理。加密记录必填来源协议，续接载荷和认证附加数据统一使用 v4；只允许原始 Chat 空推理回放为空字段，其他无可读内容的推理仍按其不透明类型处理。删除旧版本的接收路径，不双解码。流式只记录一次空字段存在性，重复空 delta 不增造续接块。

字段规则依据 [MiniMax](https://platform.minimax.io/docs/api-reference/text-openai-api)、[DeepSeek 思考模式](https://api-docs.deepseek.com/guides/thinking_mode/)、[DeepSeek Anthropic](https://api-docs.deepseek.com/guides/anthropic_api/)、[Azure](https://learn.microsoft.com/en-us/rest/api/microsoft-foundry/azureopenai/chat?preserve-view=true&view=rest-microsoft-foundry-v1)与 [Bedrock](https://docs.aws.amazon.com/bedrock/latest/userguide/model-parameters-openai.html)的原生接口契约。只匹配已知服务主机，不猜测中转站能力或模型名字。外部 CC 导入字段仍按其来源协议解读，将 `reasoning_split` 翻译为当前 `separateOutput`，不作为内部旧格式别名。

## Related decisions

[协议转换边界补强](../../implemented/bug-fix/2026-10-28-protocol-boundary-reinforcement.md)继续拥有空轮、媒体顺序和 schema 约束边界；其保留旧思考参数枚举的决定由本篇取代，实现中遗留的双 Moonshot 入口同时收敛。其余推理保真、显式错误、无自动学习降级的理由沿用，不归档该篇。[跨供应商路由一致性](../../implemented/bug-fix/2026-10-23-cross-provider-route-consistency.md)继续拥有会话与路由快照隔离。

## Alternatives considered

- 保留旧枚举并在运行时识别其不同语义，可以少改档案；但同一字段仍混合开关与格式，还阻止用户同时配置两者，故直接分开当前契约。
- 在每个缺失调用点补一次供应商改写，补丁较小；但压缩、WebSocket 和账户路径会再次遗漏，因此把现有发送准备函数收敛为唯一入口并删除重复遍历器。
- 对旧配置补默认值、对旧续接再试旧解码器，可以保持现有会话连续；但与用户明确要求的无兼容、无过渡冲突，因此只接受当前结构，旧结构保留可诊断的读取失败。

## Regression coverage

- 生产者、消费者、导入转换、界面、序列化和夹具只写当前推理配置；用例覆盖当前结构往返、旧枚举及缺失新字段的拒绝、外部导入与思考开关和格式的组合。
- 删除 `codex/request/moonshot.rs` 和旧 `apply_request_compat` 导出。普通 HTTP 与 WebSocket 通过同一 attempt 准备，辅助 Responses 与压缩通过 `codex::request::prepare`；Claude 在覆盖和账户准备后调用同一供应商处理。
- 用例覆盖 JSON 与两种流式客户端的空推理往返、工具历史、重复空 delta，v3 续接拒绝，输出字段、主机与客户端边界、非法 schema 不修改原请求，以及档案覆盖后再执行供应商字段处理。工具历史从 Chat 上游响应转换给 Responses 客户端，再回到 Chat 上游请求；不把 Chat 请求中的推理冒充 Responses 上游原生项。
- 本地遵循 AGENTS.md 的编译和自动化测试约束；发布流水线在隔离客户端目录中执行源码门禁。笔记结构使用 `npm run verify-notes`，差异、引用、文件与函数边界另作静态审查。

## Consequences

旧 Chat 推理配置需要按新格式明确保存，旧版加密续接在版本切换后失效。开发过程不写真实配置或会话。无法恢复的历史仍会失败，不通过猜测或丢弃语义绕过。
