# Agent Note: 现有协议转换边界的定向补强

Status: implemented

## Problem

Chat 推理方言把 `reasoning_split` 作为思考开关，关闭请求会让推理混入正文；没有 effort 的请求又不会启用已声明的独立推理输出。Moonshot 的 schema 修补仅从 Codex 请求入口调用，Claude 转成相同 Chat 工具形状时遗漏处理。空 assistant 历史与 Anthropic 工具 schema 的根联合没有明确的转换边界，错误直到上游才暴露。

## Decision

- 保留现有中间表示、流式转换、档案和能力配置格式。在原转换模块内修正参数语义和输入边界，不迁移到 magpie 的实现。
- `reasoning_split` 只声明独立推理输出，始终发送 `true`，且不依赖请求是否携带 effort。关闭推理必须有真实开关或可表达 `none` 的显式 effort 方言；格式拆分本身不能确认关闭。
- 两个客户端共用按目标协议与端点判定的 Moonshot schema 修补；xAI 的 Codex 私有请求载体与响应还原继续由 Codex 路径拥有，不把它们推广到 Claude。
- Chat 转换只省略不含推理、工具调用或媒体的空 assistant，跨越该空轮的相邻用户消息合并。仅含推理的 assistant 明确拒绝，不丢弃推理或伪造正文；有效工具轮及已有媒体搬移保持原有处理。
- Anthropic 目标的工具 schema 根部 `anyOf`、`oneOf`、`allOf` 在发送前报告工具名和约束名，不通过合并分支放宽参数约束。嵌套联合继续原样传递。

`reasoning_split` 的含义按 [MiniMax 参数契约](https://platform.minimax.io/docs/api-reference/text-openai-api)处理，修正错误语义而不变更已保存档案的枚举格式。路由说明按上游与档案解释缓存，不再笼统断言所有跨协议请求都不转发缓存键。

## Related decisions

本篇保留旧思考参数枚举、避免改变档案格式的决定，由[当前协议契约与请求单一所有者](2026-10-28-current-protocol-contract.md)部分取代，实现中遗留的双 Moonshot 路径同时收敛。新篇拥有严格的 `separateOutput` 配置、v4 推理来源、原生服务字段与唯一发送入口；本篇继续拥有空轮、媒体顺序和根联合 schema 的转换边界，故不归档。

检索现有 proposed、implemented 与 rejected 笔记后，[跨供应商路由一致性](../../implemented/bug-fix/2026-10-23-cross-provider-route-consistency.md)与本篇部分重叠，继续拥有会话历史隔离、已确认路由快照和原生 Responses 响应还原。本篇拥有请求转换的参数语义和输入边界，两篇均保留。[图形化模型路由](../../implemented/architecture/2026-10-26-graphical-model-routing.md)继续拥有界面与保存事务；其余网关拓扑和协议标签笔记不涉及本次决定，不取代或归档。

## Alternatives considered

- magpie 的运行时拒收记忆和自动改写可让部分请求继续执行，也能减少重复遇到相同拒收；但其调整思考、工具选择和 schema 的行为改变调用约束。本次直接修复已定位的边界，不增加学习台账、协议协商或自动降级。
- 把缺失推理补为空格、丢弃仅含推理的历史或折叠 schema 联合，可以绕过部分供应商校验；但不能恢复原推理，也不保留联合分支约束，因此使用明确的预检错误。
- 把整个 Codex 供应商适配无条件用于 Claude，调用点最少；但会连带推广 Codex 私有工具载体与响应还原，故仅共享确实与客户端身份无关的 Moonshot 修补。

## Regression coverage

- 回归用例覆盖 split 的开启、无 effort、关闭请求、明确 effort 关闭及真实思考开关，格式拆分不再输出 `false`。
- 请求修补用例覆盖两个客户端、命中与不命中端点，以及 xAI 的 Codex 边界。
- 请求转换用例覆盖空轮省略与用户合并、仅含推理的拒绝、有效工具轮的推理回放和媒体顺序，以及根联合拒绝和嵌套联合保留。
- 同步界面参数说明与中英文 README。用户未授权运行编译或自动化测试，只添加用例并审查差异；笔记结构运行 `npm run verify-notes`。

## Consequences

空 assistant 的规范化只在实际发现空轮时复制历史，已有完整历史继续借用原数据；仅跨越空轮合并用户输入，工具结果之间不插入合成文本。连接、能力与推理控件词条独立归属 `codex-connection.ts`，复用原消息目录聚合，原有消息键与两种语言均保留。

仅含推理的历史和根联合工具更早得到明确错误，需要客户端提供可表达的内容或工具定义。该范围不承诺解决未实测端点的 DeepSeek 缺失历史、Azure 模型字段或 MiniMax 原文标签解码；不改变其供应商协议或用户配置。新增用例在用户授权执行前没有运行结果。
