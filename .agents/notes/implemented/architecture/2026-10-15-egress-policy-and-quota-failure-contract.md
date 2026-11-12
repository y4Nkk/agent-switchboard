# Agent Note: 出口策略可操作契约与官方额度失败原因贯通

Status: implemented

## Problem

官方额度读取 `https://chatgpt.com/backend-api/wham/usage`。在该网络环境下直连被 DNS 污染黑洞（解析到假 IP，TCP 秒连但永远无响应），唯一可行出口是本机代理。出站代理模块自称 egress 契约唯一 owner，但 `save` 是 `#[expect(dead_code)]` 死代码：没有命令、没有 UI，用户无法把应用指向代理；默认 `None` 模式强制 `no_proxy()` 直连，应用在架构上就不可能读到额度。同时 `fetch()` 用 `Err(_)` 把已分类的传输错误整类丢弃，`CodexOfficialQuota` 契约没有失败原因字段，前端只能显示「请重试」；`quota_from_http_response` 把服务器应答异常（429/5xx）和 2xx 响应形状漂移混进同一个 Unavailable，与网络故障不可区分。传输分类枚举（原 `probe::FailureKind`）是 `pub(super)` 内部类型，五类中文文案硬编码在 `failure_message`，游离于 i18n 之外。托管账号路径甚至把传输失败伪装成 HTTP 503 再解析。

## Decision

网络客户端都经 `outbound_proxy` 应用策略，包括模型列表、Claude 认证/额度、扩展来源和 HTTP 检测。短生命周期请求直接应用策略，长期缓存按同一个不可变快照的修订重建。System 模式再次保存产生新修订，同值保存仍可刷新系统代理发现。

[启动准备与自动修复](../feature/2026-11-11-automatic-application-data-preparation.md)部分取代无效文件重置直连的决定。原手动重置的最强理由是完整备份且能迅速恢复可用性；自动场景不能据此证明改变出口正确，因此无效文件原样保留，策略构建器与缓存明确拒绝请求。当前只有读取视图和显式保存，没有手动修复 IPC 或默认直连回退。

`state/outbound-proxy.json` 继续独立拥有网络策略，不并入桌面偏好或云备份。视图返回有效设置或结构化错误及同一份原始字节的摘要修订；编辑器在文件无效但可读取时仍可明确保存有效连接，写入前复核修订并保留无效原件。不可读取的文件拒绝覆盖。内部 UUID 修订继续控制连接缓存，原始字节摘要控制编辑并发，两者不形成第二份配置事实。

缺失文件仍有明确的直连初始契约，不静默继承环境代理。代理 URL 可含凭据，独立文件限制暴露面。System 模式仍不监听系统代理开关，需同值保存或重启按原规则重新发现。

传输分类上移为跨边界契约：`asb-core::contracts` 新增 `CodexTransportClass`（dns/timeout/connect/tls/other），`probe::FailureKind` 删除并由它取代；`http_request`/`http_get` 及两个 `_with_options` 变体的错误类型由 `RequestFailure` 拥有，语法错误保留指导文案、传输错误携带 `TransportFailure { class }`（`Display` 与 `From<RequestFailure> for String` 保持调用方的错误传播）。`ProbeResult.error: Option<String>` 改为 `error_class: Option<CodexTransportClass>`，探测结果文案归 i18n。`failure_message` 后端文案删除，额度与探测两个表面的文本 owner 统一为 i18n（`errors.transport.*`）。

`CodexOfficialQuota` 新增 `failure: Option<CodexQuotaReadFailure>`，不变量为：发起过网络请求且失败 ⇒ 必有值。`CodexQuotaReadFailure` 是平铺结构（`kind` + `transportClass` + `httpStatus`），`kind ∈ transport/httpStatus/invalidResponse`，两个载荷字段按 kind 互斥由构造函数保证；`Available`/`SignInRequired` 必无 failure（前者成功、后者未尝试）。`quota_from_http_response` 三分：非 2xx 且非 401/403 → `httpStatus`，401/403 → `ReauthenticationRequired` + `httpStatus`，2xx 但无有效窗口 → `invalidResponse`。`fetch` 的传输错误 → `transport(class)`，不再丢弃。托管账号路径删除伪装 503 的做法，直接携带真实传输分类。`retain_last_success` 语义不变：窗口与 `at` 保留自上次成功，`status`/`failure`/`stale` 取自本次。TS 镜像（`src/api/usage.ts`、`src/api/providers.ts`）同批更新；`clientConfig.quota.errorUnavailable` 键删除，不可用渲染改按 failure 分类取 `clientConfig.quota.failure.*` 文案。

明确排除：额度读取不加超时重试（probe 的重试为延迟分级服务，后台调度器自会重跑）；不做 PAC、不分域路由；不改默认模式；`codex_auth::quota` 托管路径自身的 warning 通道保持原契约，仅修复其失败分类来源。

## Alternatives considered

- 默认改为跟随系统代理：本机实测系统代理关闭（`ProxyEnable=0`）时 System 模式仍直连，修不了本次故障；且让凭据流量默认跟随环境代理是安全负债。直连默认 + 可操作的手动模式才是完整契约。
- 把出口代理并入 `AppSettings` 获得单一设置文件与现成 UI：省一个卡片组件，但 `AppSettings` 是桌面行为契约且暴露面更宽，代理 URL 凭据不应进入该面；独立 owner 文件是既有边界，收编它反而制造一个更大的异常面。
- 保留 `http_get` 现签名、另加 `http_get_classified` 给额度用：diff 最小，但同一传输出现两条错误路径，分类 owner 永远分裂，属于持续性技术债，拒绝。
- 失败原因走独立诊断命令而非契约字段：UI 要轮询两个来源拼装状态，`retain_last_success` 的保留窗口与原因会在两处失配；字段随读数走是唯一不失真的位置。
- 给额度读取加超时重试：与 probe 的重试动机不同，后台调度与手动刷新都会再次发起，重试只加倍最坏持锁时间。

## Related decisions

[查询结果与额度状态分离](../bug-fix/2026-10-13-usage-availability-contract.md)确立官方订阅额度是独立契约、不进供应商脚本链；本决策只扩展该契约的失败维度，百分比展示与 availability 归一不受影响。[供应商诊断的档案解析与修复写入边界](../bug-fix/2026-10-13-provider-diagnostics-contract.md)拥有诊断表格 UI；`ProbeResult` 的 error 字段改分类枚举只影响单次探测结果呈现，表格结构不变。其余命中「额度/出口」关键词的笔记（托盘告警、文案密度）主题无关。

## Consequences

不可达出口环境可以在应用内修复：手动模式填 `http://127.0.0.1:7890`（HTTP CONNECT 与 socks5h 都由代理解析域名，绕开 DNS 污染；纯 socks5 仍本地解析，污染环境下依旧失败——失败分类现在能区分并指出）。任何一次额度失败都显示具体原因（域名解析失败/连接超时/连接被拒绝/TLS 握手失败/网络请求失败/HTTP 状态/响应无效），不再只有「请重试」。出口文件无法解析时，原生准备报告具体原因并阻止相关请求；明确连接编辑先保留原件，再通过同一严格校验和原子发布写入，不保留第二套重置投影。本次契约的其余落盘数据不受升级影响：usage-history 与重置基线都存自有扁平结构、不序列化 `CodexOfficialQuota`，`failure` 字段只存在于进程内与命令边界。代价：`http_request` 错误类型变更波及 9 个调用方（多数经 `From<String>` 零改动）；探测结果前端消费方从字符串改为分类渲染；i18n 新增三组键、删除一个键；`outbound-proxy.json` 可能含代理凭据，它只落本机状态目录（云备份仅覆盖供应商档案库，已核实不波及）。System 模式的热感知边界如上，不新增机制。

## Verification

传输分类构造、quota_from_http_response 三分与 retain_last_success 的 failure 传递以源码内联测试覆盖；前端静态复核确认 OfficialQuotaSummary、ProbeFeedback 只消费分类键、`errorUnavailable` 与 `ProbeResult.error` 全局无残留引用。原始传输分类工作当时只核对调用链与字段，没有真实网络写入验收。[自动准备](../feature/2026-11-11-automatic-application-data-preparation.md)补充当前隔离证据：无效出口不重置、客户端拒绝、显式编辑、原件保留和修订冲突已验证；真实网络与真实代理配置不在本次验收范围。
