# Agent Note: 请求性能统一使用客户端原生 OpenTelemetry

Status: implemented

## Problem

原请求性能只读 Codex、Claude 网关账本，遗漏官方和第三方直连调用。用户明确要求全部切换到官方 OpenTelemetry，完整移除兼容、过渡和技术债。原网关供应商归属、协议映射、计价和最终记账状态不能伪装成客户端原生事件；传输、逐流事件及完整生成的耗时也不能互换。

## Decision

`native_telemetry` 唯一拥有本机 OTLP 接收、原生事件解码、允许字段、去重及性能事实存储。`request_usage` 只投影该所有者的数据，不再读取、导入或回退到网关账本。旧 `request_usage/sources.rs`、两端专供性能查询的访问器，以及旧的供应商归属、协议映射、网关状态、计价异常与尝试列表契约一并移除。

网关账本及其有效计费、预算、故障转移、记录和诊断消费者保持原职责。上方会话消费仍由原所有者汇总，性能 Token 不重复累加。没有新增依赖、ASB 对外分析遥测、常驻独立守护进程、自动配置写入或隐式提供商切换。

## Receiver and current wire contract

- 接收器随桌面应用启动，在主窗隐藏或留在托盘时持续运行，退出应用时释放；固定只监听 `127.0.0.1:4318`。端口冲突及接收异常可见，提供显式重试，不静默换端口或把导出交给占用该端口的其他进程。
- 当前协议是 OTLP/HTTP JSON：`/codex/v1/logs`、`/claude/v1/logs` 和 `/claude/v1/traces`；只接受 POST、JSON 及未压缩或 gzip 请求，编码体和解压体各限 4 MiB。有效空 OTLP 消息按当前规范接受。错误区分方法、路径、内容类型、大小、数据和持久化故障，不记录原始导出包。
- Codex 消费 `codex.api_request`、`codex.websocket_request`，以及 `codex.sse_event` 中带用量的完成事件和带错误的流事件。没有用量的普通 `response.completed` 流通知不重复计入完成用量观测。完成事件的 `duration_ms` 不代表完整生成，不能用于计算速度。
- Claude 网络范围消费原生 `api_request` / `api_error` 日志，生成范围只消费 `claude_code.llm_request` span。原生 `duration_ms`、`ttft_ms`、`first_content_ms`、成功状态、状态码、错误分类、尝试数及 Token 作为允许字段；缺失耗时不从 span 边界另算。无关工具 span 不进入模型性能。
- 日志使用原生事件时间；OTLP 时间未设置时使用 SDK 明确提供的 observed timestamp，并将时间来源保存在记录中。span 使用原生结束时间。两者都不以 ASB 接收时间或相邻事件补造操作时间。trace/span 身份必须是有效非零十六进制。
- 原始提示词、回答、工具内容、凭据、用户邮箱及整个原始载荷均不持久化；只保留有界的性能字段、原生模型/会话/请求标识及原生错误类别。未知 OTel 事件正常忽略，并在本次运行计数中说明。

## Persistence and replay

唯一性能库为 `state/request-performance.sqlite3`，以独立 application ID 和版本 1 标识。关系索引与严格当前 JSON 记录必须一致；写入与读出使用同一份字段及派生速度校验。未知版本、外来对象、损坏数据和身份冲突明确拒绝，原文件保留，不当成可丢弃缓存重建。只有没有对象、没有版本且没有其他所有者的空库可以完成被中断的首次初始化；没有业务数据升级或网关历史转换路径。

原生 span 以客户端、trace ID 和 span ID 定义不可变身份；日志以原生时间、trace/span 上下文及归一化允许字段计算观测指纹，身份指的是原生观测，不声称是去重后的用户请求。相同身份重发幂等；同一不可变 span 内容冲突拒绝整批事务，已有数据不覆盖。乱序按记录时间及身份索引查询，接收顺序不改变报告排序。不按年龄或数量自动删除原生性能事实。

## Configuration and recovery

导出配置在性能页按客户端分别预览与确认。原生配置候选只调整该导出的字段，经已有 `ConfigWriteGate`、源/候选/设置意图哈希、`commit_rendered_client_configuration`、切换执行器及配套备份/恢复事务应用，不建立第二条真实客户端文件写入路径。

Codex 配置 JSON 日志导出与关闭 prompt 内容记录，保留模型、供应商、其他 OTel exporter 和非归属键。Claude 启用原生 OTel 日志与请求 trace，配置各信号的本机 JSON 端点，关闭对应内容字段，并显式清除会重定向日志/trace 的 Beta endpoint。已保存通用片段中重叠的 env 意图在同一事务更新，避免下一次切换覆盖本次确认；不新建第二份启用状态。停用明确关闭当前本机导出，恢复以前目的地使用现有备份恢复。

页面状态只说明实际用户配置文件，不冒充进程的有效配置。客户端需要按其原生规则重启，启动环境及管理员策略可以覆盖用户文件，实际采集以收到事件为准。监听故障时不允许启用新导出，仍允许停用已经指向本机的导出。整个开发过程不写真实客户端配置或凭据。

## Statistics and presentation

「生成结果」与「网络尝试」有独立查询范围和分母。速度只接受同一成功生成观测的原生输出 Token 及明确报告的完整请求耗时；Codex 未报告完整耗时时保持未知，原生输出可能包含推理。首 Token 和首内容各自只使用原生字段，不将二者与旧网关首输出、服务端推理时间或 HTTP 成功互换。成功比例仅描述已导出原生观测；重试可能产生多条 Codex 事件，不称为全部实际调用的去重完成率。

比较按客户端、模型和原生操作分组。不同操作的比例或时延分位数不在总览及趋势中混合，界面给出分开查看的说明；各指标仍披露实际字段覆盖数。缓存按 Token 加权，Claude 的新输入与缓存读写只在所需计数均有报告时构成完整输入；缺失不补零。费用仅使用客户端原生微美元估算，不用本地网关价格补齐，不代表实际扣费。重试/恢复只使用原生明确报告的尝试数，不根据错误序列猜测。

原有有界展示原则保留：后端计算逐条样本的精确分位数，明细每页 25 条、分组每页 20 条、趋势最多 96 桶，不把完整事实载荷交给前端。日期、客户端、计量范围和模型筛选在后端完成，来源覆盖先于模型筛选；按日志时间或 span 结束时间过滤，不投影或猜测请求开始时间。报表可见时每 30 秒更新，隐藏停止读报表并卸载图表与明细，后端接收不受页面挂载控制。读取失败保留同范围旧报告和原时间，迟到响应不能覆盖另一个查询。

页面继续沿用共享 ModuleHeader、Tabs、Table/Pagination、FactList、CollapsibleSection、ConfirmSheet 和 DiffView。监听状态、导出时间及错误常显；采集设置中显示实际文件状态和本次运行保存/忽略计数，写入前展示完整差异与警告。生成范围显示速度、首 Token、观测成功比例和缓存；网络范围显示原生耗时 P50/P95、观测成功比例及记录数。模型比较和原生记录替代旧供应商比较；不根据当前活动档案归属历史。空数据不挂载四项未知卡片、空图和表格，辅助统计默认收起。

## Retained gateway invariants

原先为网关记账修正的行为保持：206 是部分输出，502 可表示逻辑/协议失败，499 是明确客户端断连；这些不是原生 OTel 的通用状态。206 保持网关路由健康所用的现有 2xx 语义。HTTP 与 WebSocket 沿用同一原生响应失败解释器；非空文本、推理及工具内容才构成网关首输出，创建/心跳/结束事件不补造；原始 SSE 字节仍保留。这些网关运行事实不进入当前原生性能库。

## Related decisions

- [完整本地会话与用量](../../implemented/architecture/2026-10-16-complete-local-session-usage.md)：部分重叠，继续拥有会话消费、去重和覆盖。
- [消费快照与合并刷新](../../implemented/bug-fix/2026-10-29-usage-snapshot-first-refresh.md)：部分重叠，保留可见性、错误与同范围刷新原则，不触发会话扫描。
- [本地资源边界](../../implemented/architecture/2026-10-14-bounded-local-resources.md)：部分重叠，保留有界 IPC 和按需界面数据。
- [原网关性能决定](../../archived/feature/2026-11-04-gateway-request-performance.md)：被本篇完整接管并归档。其有效统计、展示、资源边界、取舍、网关修正与验证限制已经在本篇保留；现行来源以本篇和代码为准。
- 同主题审计没有其他活跃 OTel 方案；会话日期范围提案无关，保留原样。

## Alternatives considered

- 保留网关账本作为唯一来源可以复用完整的路由身份、计价、计时和重试，改动最小；但无法覆盖绕过网关的调用，不满足本次指定来源。
- 原生优先、网关补齐能保留更多历史与指标；但无法总是证明同一调用身份，会产生重复计量、第二份事实和混合计时边界，不满足用户要求的完整切换。
- 只接收 OTel metrics 可以直接用官方计数器和直方图，接收与存储简单；但无法重建逐条明细、精确分位数或确定同一请求的用量，不从聚合指标伪造记录。
- 用总耗时减首 Token 更接近部分流式输出阶段；但输出 Token 可能包含首 Token 前的推理，且原生边界并不统一，因此只保留同一原生完整耗时的平均速度。
- 在前端读全量观测再算统计最直接；但 IPC 和常驻内存随历史增长，因此在后端查询，仅发送分页与有界趋势。统一生成/传输或跨原生操作的中位数可以得到更多总览数字，但改变了测量含义，因此未知不补算，按操作分组查看。
- 沿用原方案的纯派生查询可以省去新存储；其前提是事实已经在网关账本中。当前原生事件没有既有持久所有者，必须由一个独立原生库保存；不另建派生快照或并行历史来源。

## Consequences

性能采集与模型路由解耦，官方和第三方可以通过相同的原生机制提供事件。旧网关历史不进入新报表，缺失计时不能回补；完整请求耗时、首 Token、费用和原生 trace 的支持取决于客户端版本与原生开关。Claude 请求 span 使用其官方增强 OTel 开关，不能把文件已配置说成该客户端实例已生效。

代价是新增本机端口和原生事实库的生命周期，应用关闭期间不会采集，也不承诺未导出或 SDK 丢弃的数据可恢复。固定端口冲突需要明确处理；只接收本次配置使用的当前 JSON 协议，不增加其他协议、兼容形状、源切换或回退。实际长历史查询耗时和桌面布局仍需要运行验收，未凭源码宣称性能改善。

## Verification

沙箱验收使用仓库内隔离临时目录，既有测试只选择 `native_telemetry` 与 `request_usage`，不启动桌面或浏览器，不写真实客户端配置。HTTP 头/路径和二进制请求体解码保持单一生产路径；进程内请求直接验证双端接收、持久化、分页和错误响应，真实回环套接字测试单独保留。沙箱禁止回环 TCP 时明确报告阻塞，不降级网络策略或把进程内验证称为客户端真实导出验收。

代码内隔离夹具覆盖原生完成事件与普通流事件区分、传输不产生生成速度、完整请求字段与缓存归一化、原生时间/观测时间来源、未知值、无关 span、重发幂等、身份冲突与事务保留、外来库及中断空库、当前配置候选与重叠通用意图、分位数/缓存权重、测量边界不混合、分页和旧网关查询结构拒绝。写入夹具全部使用临时目录和合成配置。

原生 `response.incomplete` 终止事件保留其类型，按未成功观测计数，不补造 HTTP 206 或完整生成耗时。夹具还包含临时回环端口的 gzip HTTP 接收、真实解码/存储调用链，以及乱序落库后的稳定分页。`server::tests` 通过内存请求直接调用生产接收器，覆盖两端日志及 Claude span 三个入口、重复导出与客户端/模型筛选；二进制正文走同一个解码函数，覆盖 gzip、编码和解压后大小限制、协议错误、失败后的数据保留和整批冲突回滚。

用户随后明确授权沙箱测试。`cargo test -p agent-switchboard --lib --offline --locked -- native_telemetry:: request_usage:: --skip loopback_http_gzip_export_uses_the_real_decoder_and_store --nocapture` 在仓库内隔离 TEMP/TMP 下实际执行，19 项通过，包括临时 SQLite 读写和查询。首轮包含真实回环测试时为 15 项通过、1 项失败，原因是沙箱禁止回环 TCP（Windows 10013 / PermissionDenied）；该夹具保留，后续进程内验证不代表真实网络已通过，也不通过提权绕过用户要求的沙箱边界。

`npm run typecheck` 与 Vite 生产打包实际通过，打包输出仅在 `target/otel-sandbox-verification/web-dist`。测试日志位于同一忽略目录；未启动开发服务器、桌面或浏览器，未访问真实账号、凭据或写入真实配置。双端真实客户端 HTTP 导出、配置写入/恢复及桌面视觉仍未验收。官方文档、Codex 原生实现和生产读取链、直接消费者、React 可见性/异步清理、变更差异与笔记结构的静态检查保留为辅助证据。

## Sources

- [OpenAI Docs: Observability and telemetry](https://learn.chatgpt.com/docs/config-file/config-advanced#observability-and-telemetry)
- [Claude Code: Monitoring](https://code.claude.com/docs/en/monitoring-usage)
- [Codex native telemetry events](https://github.com/openai/codex/blob/main/codex-rs/otel/src/events/session_telemetry.rs)
- [Codex native response handling](https://github.com/openai/codex/blob/main/codex-rs/core/src/session/turn.rs)
