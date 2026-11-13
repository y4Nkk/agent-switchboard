# Agent Note: Codex 与 Claude 供应商双向转换为独立新档案

Status: implemented

## Problem

用户需要选择 Codex 中的一条供应商，转换后在 Claude 中新增档案；也需要选择 Claude 中另一条供应商，在 Codex 中新增档案。两次来源可以完全不同，不要求同名、同域名或已有对应记录。当前供应商编辑器切换客户端只打开空白新建会话，无法复用来源连接。

两端持久化结构刻意不同：Codex 第三方使用完整模型目录、能力声明、模型别名和专用存储；Claude 使用通用档案、用途路由与专属片段。直接改 app、复制 JSON 或复用编辑身份会造成错误契约、悬空引用或误更新活动供应商。

## Decision

增加供应商级的“转换并新增到 Claude / Codex”，以一次性快照生成目标客户端的新建草稿。保留原上游协议与连接事实，由现有网关承担实际请求协议转换；不替用户把 Responses 接口猜成 Anthropic 接口。

普通静态 API Key 的 Responses、Chat Completions、Anthropic Messages 档案纳入双向交付。需要补全的字段留在编辑态，补全后才生成当前合法保存契约。官方登录、托管账号、Claude 原生云 SDK 和 Codex 不支持的 Gemini Native 明确返回不可转换原因。

来源和目标是独立档案：目标编辑会话分配新 UUID，保存沿用该身份并追加到目标端末尾；没有持久来源绑定、自动同步、同名覆盖、自动启用、自动入故障转移队列或递归复制依赖。现有客户端配置和源档案保持不变。

核心只读转换和两端编辑器已接入同一个新建保存入口。真实数据副本、聚焦模型解析与隔离浏览器验证覆盖当前业务边界；真实供应商联网与原生客户端交互是独立证据，不由配置转换或保存成功推断。

## Current evidence and ownership

| 事实 | 当前所有者 |
| --- | --- |
| 切换编辑客户端创建空白会话 | `src/app/useProviders.ts::newEditorFor`，由 `ProvidersWorkspace.tsx` 传入两端编辑器 |
| Codex 必需目录、正整数限制、能力及别名合法性 | `crates/asb-core/src/contracts/codex.rs`、`contracts/codex/validate.rs` |
| Codex 别名解析和直连/网关判定 | `CodexProviderProfile::resolve_model`、`contracts/codex/routing.rs` |
| 客户端专属连接字段和认证约束 | `validate/codex.rs`、`validate/claude.rs`、`claude_auth.rs` |
| URL 含义、完整请求 URL、独立 models URL | `crates/asb-core/src/endpoint.rs` |
| 请求协议转换与不支持语义的显式拒绝 | `src-tauri/src/gateway/transform/request.rs`、`transform/mod.rs` |
| 新建和编辑身份、现有保存与切换 | `src/app/useProviderSaves.ts`、`commands/profiles.rs`、`commands/switching/profile_save.rs` |
| 模型引用只指向所属客户端的合法目标 | `gateway/claude_targets.rs`、`gateway/codex/routes.rs` |
| Claude 普通模型名可能回到主模型 | `gateway/transform/claude_model.rs::map_model`、`server/claude/attempt.rs::prepare` |
| 用量查询刷新是后端调度，非编辑器副作用 | `src/api/usage.ts`、`contracts/usage.rs` |

## Supported boundaries

| 来源 | 目标 | 方案结果 |
| --- | --- | --- |
| Codex 静态 Key，Responses / Chat / Anthropic | Claude | 可转换；丢失或需调整的专属行为逐项显示 |
| Claude 静态 Key，Responses / Chat / Anthropic | Codex | 可转换；模型目录、能力、限制等通常需补全 |
| 两端官方原生登录 | 另一端 | 阻止；登录身份与凭据缓存不属于可复制供应商连接 |
| Claude Copilot / Codex OAuth / xAI 托管账号，Codex xAI 托管卡 | 另一端 | 阻止；不把账号绑定、占位 Key 或会话令牌转成静态 Key |
| Claude Bedrock / Vertex / Foundry 原生 SDK | Codex | 阻止；SDK 认证与普通 HTTP Key 不是同一契约 |
| Claude Gemini Native | Codex | 阻止；当前 Codex 上游枚举、校验和转换器均不接受该组合 |

阻止项只指当前显式转换功能的边界，不代表同一服务商永远无法使用另一客户端。用户取得该服务商独立支持的 API URL 和 Key 后，可按目标端现有新建流程创建。

上游协议与目标客户端协议相同也不能仅凭协议宣称直连：完整 URL、自定义头和请求体、多端点、模型别名、minimal 模式等仍交给既有路由判定。典型普通连接中，Claude 使用 Responses / Chat 必经网关，Codex 使用 Anthropic / Chat 必经网关；目标实际为普通原生协议时按现有规则判定。

## Field conversion rules

| 字段 | 规则 |
| --- | --- |
| 名称、备注、官网 | 原样预填；同名时建议加目标客户端或序号，用户可改；不按名称或 endpoint 匹配覆盖 |
| id、position、创建时间、活动身份 | 不复制；目标编辑会话持有独立新 ID，排序由目标存储分配，现有记录不被修改 |
| API 根地址、完整 URL 标志、协议 | 保留原语义，使用现有 endpoint resolver 验证最终请求 URL；不拼替换 `/v1`、`/responses`、`/messages` 来猜接口 |
| API Key、认证头类型 | 只复制源档案明确保存的静态事实；默认认证先按源上游解析，再显式映射到目标；不按目标客户端重置 Bearer / x-api-key |
| 自定义端点、User-Agent、合法 headers/body overrides、models URL | 按通用白名单复制并重新验证；端点 lastUsed 不继承；body 中显式 model 覆盖在转换摘要中突出显示 |
| Responses standard/minimal | 仅保留于 Responses 上游，两端均复用现有类型和处理器 |
| `connection.codex`、Claude billing / prompt cache / apiKeyField / native | 不跨端透传；有已实现的一一对应语义才显式映射，否则列出不继承项并让用户检查 |
| `authBinding`、`providerType` | 使用现有认证解析识别静态与托管身份；不因 apiKey 非空就判为静态，不静默移除未知类型来绕过校验 |
| 供应商运行参数、Claude fragment、输出风格、权限及共享设置 | 不跨端机械复制；新建使用目标端现有默认值，列出来源专属覆盖；不修改全局共享设置 |
| 用量查询 | 可选复制，默认不选；选择时保留脚本/声明内容，刷新间隔置 0 并明确显示“仅手动查询”；源查询不变 |
| 历史、缓存、请求统计、额度、测试结果、故障转移队列 | 不复制、不重新归属，不自动运行余额或模型请求 |
| 图标/分组等显示元信息 | 只复制目标当前契约已有的字段；Codex 无 display 字段时在摘要中说明，不为此扩展持久结构 |

专属行为的“不继承”必须可见。目标缺失必填字段属于 needsInput；用户查看摘要后可接受的行为差异不伪装成等价转换。任何未知、无可表达目标的字段不能靠运行时兼容分支接收。

## Codex to Claude models

1. 主模型调用来源 `resolve_model(default_model)`，使用实际上游模型 ID。例如 `fast -> vendor-model` 转换后的 Claude 主模型为 `vendor-model`。
2. 遍历来源目录，按同一别名规则解析成真实上游模型并去重，作为 Claude 可用模型候选；原别名关系仅在临时转换摘要展示。不同别名指向同一真实模型时不复制成多条虚假模型。
3. 主模型选定后可将其余候选带入 `availableModels`。Claude 的 Haiku / Sonnet / Opus / Fable / 子代理不按名字或能力猜分档；使用现有默认语义，用户需要分档时从候选明确选择本连接的 Primary 路由。
4. 不复制 Codex 子代理的外部 profileId。摘要显示源依赖名称与模型；用户在目标端另行配置。模型目录不能把其他供应商的模型混入当前 Key 的本连接。
5. Codex 的目录能力、专用压缩、服务端搜索、推理方言、上下文投影不宣称转成 Claude 等价能力。Codex Chat 特有 thinking/effort/separateOutput 不可直接塞入 Claude；依赖它的档案须检查目标思考设置和实际请求。

多模型交付有一个直接前置改动：`map_model` 目前在普通名字未命中用途时使用主模型，不能将 `availableModels` 当成已经完整实现的多模型透传。调整同一个解析所有者，顺序为保留的内部用途角色、当前连接明确声明的可用模型精确匹配、既有用途与主模型规则。只对本连接声明的 ID 精确透传，不引入按名称猜测外部供应商的规则。对应消费点和碰撞用例同步更新。

## Claude to Codex models

1. 从主模型、同连接 Primary 用途以及明确的可用模型列表收集候选。默认选主模型；主模型为空时由用户选择，不凭空代选一个用途模型。保留当前解析后代表真实本连接的模型，Claude 的内部用途标识和 `[1m]` 标记不能直接变成上游模型名。
2. 指向其他 profileId 的 Provider 用途只显示为未继承依赖，不用当前连接 Key 调用该模型，也不递归转换目标供应商。自引用仅在可证明指向当前连接时作为本连接候选处理。
3. 每个选择进入 Codex 的模型必须有合法目录行：正整数 contextWindow / maxOutputTokens、工具、图片、推理、推理档位与 compact 等能力。能力不能从模型名、协议或 `/models` 存在推断。
4. Claude 的 1M 模式与 compactionWindow 不是完整的 Codex 模型声明。compactionWindow 是压缩阈值，不能当作 contextWindow；当前转换目录的上下文与输出上限统一留待用户明确填写。
5. 未知值保留为待填。普通 Codex 编辑器为留空限制生成的 128000 / 8192 不用于转换草稿；新增目录和获取模型后新增的行也保持未知限制、关闭未声明的图片能力。所有选中模型的限制须为明确正整数，并单独确认能力，才能物化为完整当前契约。改变能力或目录声明使之前的确认失效；不改变普通新建表单的默认含义。
6. Anthropic 上游的 Codex 输出预算复用目录 maxOutputTokens 的现有投影，不能把 Claude 顶层 maxOutputTokens（该场景为 null）直接照搬。Chat 上游若需要推理，必须按 Codex 当前 chatReasoning 契约配置，否则显式关闭对应能力并让用户确认。
7. 初始 `modelRoutes` 为空，clientModel 与真实上游 ID 相同；用户需要别名时在现有编辑器配置。初始 `subagentRoute` 为空，不复制 Claude 的跨档案引用。

## Typed flow and change surface

核心增加 `provider_conversion` 纯转换模块及临时请求/响应类型。输入是已验证的源类型；输出为 `ready | needsInput | blocked` 的带标签结果，包含目标端编辑种子、保留/变更/未继承清单、待填字段和结构化原因。编辑种子可以表达未知值，不能作为持久 ProviderDraft 被存储层直接接受。

Tauri 增加一个 `prepare_provider_conversion` 入口，参数为 sourceApp、sourceProfileId、expectedSourceFileHash、targetApp。后端在既有配置门禁下读取指定存储并核对修订，拒绝同端、错端、源删除、过期或不支持来源。前端不上传一份冒充源事实的完整档案。该调用不访问网络、不保存准备令牌、不创建档案。

响应返回内存快照和临时来源摘要。密钥只在已有编辑边界内流转；可视摘要、错误和日志均脱敏，不能序列化整份草稿到普通日志、浏览器持久存储或额外临时文件。

前端两端行操作共享入口文案，复用目标端 EditorFrame 和字段组件。在 `ProviderEditorSession` / `CodexEditorSource` 中明确区分 existing 与 conversion-seed，新建种子不伪造 ProviderRecord、id 或 fileHash；React 会话 key 使用独立编辑会话身份，避免多个新草稿间复用旧 state。

编辑器显示来源、目标、保留的连接、目标将用到的网关、需要补全字段及未继承行为；不支持来源在入口展示具体原因。提交动作名为“新增到 Claude / Codex”，只保存新档案。取消不产生写入；返回原列表并恢复操作焦点；迟到的准备响应不能覆盖新打开的编辑会话。

保存准备和完成同时检查唯一会话 setter 同步维护的版本以及当前工作区版本，不能仅等待 React 下一次 render 才失效。旧准备返回后释放而不提交；已开始的创建或提交完成后刷新已保存数据，但不关闭替换草稿、不选中过期目标。同一事件中的重复提交由同步门闩拦截。触摸设备不隐藏行操作，长文本转换按钮所在操作组可收缩换行，不能被行边界裁切。

保存复用当前 `create_codex_profile(profileId, draft)` 或 `prepare_profile_save({kind:create, profileId}, draft)` / commit 创建分支；profileId 来自目标编辑会话，不能来自来源，草稿字段本身不包含旧 ID。继续经过目标当前校验、配置门禁、原子写和目标端修订管理。转换不新增真实客户端文件写入路径。后续“启用”仍走现有预览、确认、备份与恢复事务。

来源快照在准备时固定。若用户编辑期间源档案改变或删除，已经脱离来源的目标草稿仍可保存；界面说明“基于打开时的配置，不随原档案更新”。重新读取来源是用户显式动作，不能覆盖用户未保存编辑。准备调用发现 hash 已变化时则不产生草稿，要求刷新来源。

实现涉及：core 新模块与契约；Tauri 命令、注册及 dev_api 同源调度；前端 API、供应商行、两端页面、会话和种子适配、模型补全及双语文案；Claude 模型解析这一必要消费点；README / DESIGN 的功能与交互事实。复用现有组件与路由判定，不新增依赖、数据库或持久 schema 版本。

## Related decisions

本次检索覆盖 active proposed / implemented 及协议、路由、供应商存储关键词；当前没有该双向转换的同主题笔记，rejected 目录当前不存在。

- [按档案保存用途路由](../../implemented/architecture/2026-11-12-profile-owned-purpose-routing.md)：部分重叠，保留其保存与启用分离、同端引用及依赖事务所有权，本提案不取代。
- [当前推理契约](../../implemented/bug-fix/2026-10-28-current-protocol-contract.md)：部分重叠，保留其明确声明、统一转换入口与拒绝无法表达语义的边界，不扩展旧字段兼容。
- [供应商资料保存](../../implemented/bug-fix/2026-09-22-usage-query-metadata-save.md)：部分重叠，现有编辑默认值不能被误当源事实；本提案使用独立种子，不改已有保存身份。
- [供应商存储修复](../../implemented/feature/2026-09-23-provider-store-repair.md)：相邻范围，继续拥有当前严格存储与修复；新增转换不承接历史格式迁移。
- [统一供应商编辑器](../architecture/2026-11-13-provider-routing-editor-integration.md)：部分重叠，编辑上下文、创建目标身份和分区导航由统一编辑器拥有；本篇拥有转换规则、独立编辑种子和真实副本验收边界。

没有既有决定被完全替代，不归档、不改写其它笔记。

## Alternatives considered

- 前端直接复制字段最省代码，来源数据当前也已在页面内存中；但两端契约不对称、认证和模型解析不能由 UI 再写一套，因此后端纯转换模块唯一拥有规则，前端只适配编辑种子。
- 统一供应商实体、两端引用同一凭据，能减少重复维护并天然同步改 Key；但用户要的是独立新增，统一实体会改变持久身份与更新含义，还要迁移两端存储和依赖，超出本需求。
- 自动切换到目标原生协议可以减少网关依赖；但原 URL 和 Key 不证明服务商提供另一种协议，改路径也不能增加协议支持，因此保持来源上游协议。用户另填并验证服务商明确支持的另一接口是目标编辑动作。
- 直接使用 Codex 导入默认能力和通用模型限制，能让反向转换一次点击即保存；但默认开启搜索、压缩和完整推理并非来源事实，静默生成限制会掩盖关键缺失，因此把确认限制和能力作为反向转换的必要输入。
- 只迁移默认模型可以完全避免 Claude 多模型消费点变动；但会丢掉来源已保存的可用模型，且仅复制列表会造成表面成功，因此包含有限的精确模型解析修正并检查最终上游 ID。
- 单独建立“转换并写文件”执行器可统一返回新记录；但现有新建已经具备严格校验和原子保存，这会增加第二条写入路径，因此只增加只读转换准备，目标新建仍使用现有所有者。

## Testing

- Core 转换聚焦 11 项通过，覆盖三种普通协议、真实模型别名去重、Anthropic `[1m]` 的类型化映射、可证明本连接的 Claude 别名、外部引用隔离、SDK / 托管 / Gemini 阻止、完整 URL 与覆盖选项、手动用量复制和诊断脱敏。
- Claude 模型解析聚焦 11 项通过；两个已声明模型分别转换到 Chat / Responses 的上游 payload，模型 ID 保持各自身份。测试覆盖内部用途优先级、档位名字碰撞、外部同名目标与未知模型处理；没有将 payload 检查冒称实际供应商 HTTP 调用。
- `scripts/test-provider-conversion.mjs native` 复制真实供应商 JSON 到受控忽略目录，在重定向 HOME / APPDATA / CODEX_HOME / CLAUDE_CONFIG_DIR / TEMP 的子进程运行显式启用的隔离用例。7 个来源包括 2 个 Codex 第三方、4 个 Claude 和 1 个 Codex 官方记录；2 条转 Claude、4 条转 Codex、官方记录阻止。6 组来源各新建两个同名独立目标并读回，目标保留指定编辑身份；源副本与全部既有文件保持字节一致，客户端配置哨兵不变，只新增目标档案文件。
- 真实副本的反向目录使用明确的测试选择：上下文 32768、输出 2048、保守能力。这些数值不属于供应商能力发现，也不会写回真实档案。所有真实来源在复制前后及测试结束后比较 SHA-256，原件保持不变；含真实 Key 的来源副本和测试临时环境已清理，证据只有协议、计数和哈希。
- 前端转换、路由与双语聚焦 21 项通过。隔离真实组件浏览器 49 项断言通过，覆盖两端入口、独立新 ID、可选手动用量、缺失值拦截、失败保留、重复提交、迟到转换、三类同事件延迟保存、取消焦点、640px 中英文深浅主题和 420px 触摸按钮边界；无外部请求。
- `npm run build` 通过，包含 TypeScript 和生产 Vite 构建。笔记结构通过 `npm run verify-notes`，改动差异、冲突标记和当前契约引用另作静态检查。

## Consequences

协议适配已有基础不代表所有工具、推理、图片、服务端搜索和压缩等价；保存成功、直连测试、模拟网关和真实客户端是不同证据。转换后仍遵循现有显式不支持错误。

反向转换的主要成本是用户补全能力与限制；收益是没有伪造事实或第二个持久契约。若后续需要托管账号转换，应单独设计账号所有权与授权，不通过本功能“放开字段”。

多模型精确匹配调整改变已声明模型在 Claude 网关的行为；聚焦回归守住内部用途优先级、与档位名字碰撞、跨供应商同名模型及未知模型处理，转换不重写整个路由模块。

本轮没有写真实客户端配置、读取或复制凭据缓存、调用真实供应商、运行原生客户端交互验收或发布应用；真实供应商文件副本的隔离转换与读回属于本功能已执行的验收边界。
