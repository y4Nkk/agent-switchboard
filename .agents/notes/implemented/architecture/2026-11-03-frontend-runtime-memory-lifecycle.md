# Agent Note: 前端运行时资源回收与主窗口低内存模式

Status: implemented

## Problem

会话分页、有界队列和托盘闲置销毁已有明确所有者，但雷达访问后隐藏页面仍持有动画 iframe 与完整批次，后台每秒状态读取还会携带回答全文。网关读取只判断路由，扩展来源的累计结果跨工作区常驻。直接卸载整页会丢失领域组件中的未保存输入、筛选和阅读位置，因此回收落实到可重建载荷及原生引擎目标。

## Decision

各领域继续拥有草稿与操作，统一使用现有工作区、原生窗口及 document 可见性触发停止读取和载荷释放。没有新增全局草稿仓库、缓存注册表、页面 LRU、持久化格式或依赖。

| 状态 | 载荷与读取 | 必须保留的内容 | Windows 主窗口 |
| --- | --- | --- | --- |
| 所属工作区和子视图可见 | 按现行规则读取并显示只读预览 | 原有编辑与导航状态 | Normal |
| 离开工作区或雷达正文被历史覆盖 | 停止所属读取，移除预览和可重建详情 | 题目输入、筛选、页码、详情身份与选择 | 窗口可见时保持 Normal |
| 窗口隐藏或最小化 | 页面专属读取停止；全局检测观测继续 | 未保存草稿、已提交操作、任务关注与通知去重 | Low |
| 恢复显示 | 原生目标处理后发布可见性；按当前身份重读 | 使用原组件状态，不从第二个草稿库恢复 | 显式设置 Normal |
| 托盘浮层闲置五分钟 | 原有 destroy/rebuild | 后端事实继续存在 | 不改变浮层生命周期 |

失焦本身不触发回收；窗口隐藏也不等于退出业务路由，不能关闭编辑器或把已确认的事务当成取消。资源页的展开、内容与滚动保留规则保持原有所有权。

### 轻量观测与完整详情

src/api/codex-probe.ts 唯一拥有检测 TypeScript 契约；检测内容从原 usage 模块移出，client 统一导出。get_codex_probe_observation 替换旧全文观测入口，直接生产者、命令登记、浏览器开发分发、通知与进度组件同步使用 ProbeObservation。旧命令、包装函数及当前批次全文读取函数已经移除，没有别名、双读或把 finalAnswer 置空伪装成完整详情的路径。

后台响应只含批次身份、判分规则、题目标签、运行及判分统计、持久化与状态异常以及原有单调时钟进度，不含题目全文、配置、回答或运行明细。ledger/observation 的 SQL 直接选择这些字段和小型运行状态，不读取 final_answer、question_text 或配置字段。registry 持有原写入锁，SQLite 事务提供批次与运行同一快照；当前运行槽优先于系统时间排序。待保存结果仅借用并投影统计，不克隆回答，原始结果仍由原 registry 独占并支持保存重试。

可见详情复用唯一的 get_codex_probe_batch。use-codex-probe-detail 由当前可见读者持有全文，probe-detail-reader 复用现有一个执行中加一个最新待执行的队列；退出先失效代次、取消未开始任务，迟到结果不写回。更换批次立即隐藏其他批次的旧载荷；同一批次重读期间保留上次内容与明确错误。

probe-observation 的详情修订只包含身份、轮次、完成数、终态和保存异常，以及显式重读 epoch；elapsedMs 和阶段时钟刷新不触发全文读取。题目输入仍由 UsageWorkspace 的原表单所有者保存。雷达正文不可见或被历史覆盖时，参考 iframe 和整个只读结果子树移除，派生 HTML/SVG source 随子树释放，完整详情状态清空。历史拥有自己的可见详情，不再借用根组件的完整实时批次。

历史隐藏时清空可重读列表、筛选选项和详情，保留筛选、页码、detailId 与所选 ID；新读数重新核对选择。删除确认冻结所选记录的摘要，确认或删除期间暂留所需上下文，隐藏时不显示 portal。已确认删除继续完成，失败后可恢复原确认；结束后根据当前可见性释放。后台完成、失败和保存重试通知继续使用相同去重所有者，空闲停止轮询。

### 网关和扩展

GatewayPage 复用 useWorkspaceVisible；路由 active 单独拥有端口弹层退出，可见性只控制读取和相对时间显示。定时读取在前次完成后再调度，进行中读取合并并拒绝旧代次响应；隐藏清除定时器并失效旧读取。已确认变更继续由原执行器完成并更新事实。

ExtensionsWorkspace 将 active 传至原扩展工作区。无编辑、写入或进行中扫描依赖时，隐藏释放可重建发现快照；轻量 scanId 和选择、诊断定位保留，真正重扫才失效。started 标志不因回收重置，恢复显示不自动重新扫描。

Skill 来源浏览器保留来源、查询、筛选、路径与管理表单；不可见且没有编辑、导入或目录安装依赖时，通过稳定的 clear 同时释放 result state 与 latest ref，并先使在途请求失效。目录的累计结果与逐条解析缓存一并清空；仅 cancel 不承担回收。目录安装从解析候选开始即同步占用锁并保留载荷，避免解析与真正导入之间被隐藏回收。原导入、保存和安装继续完成，返回不隐式查询外部目录。

### 原生目标

src-tauri/src/webview_memory.rs 只控制 Windows 主窗口；workspace_visibility 继续拥有可见性事实。一次主线程任务重读最新可见性，使用当前依赖已有的 ICoreWebView2_19/SetMemoryUsageTargetLevel，再发布实际可见性。with_webview 回调只调用原生接口，必须退出回调后才能发布 Tauri 事件；当前 Wry 在主线程同步派发时仍持有 WebView 分派锁，回调中 emit 会重入同一锁，真实最小化测试已复现界面与后续 IPC 一起卡住。成功目标去重；失败不确认成功状态且保留具体诊断，同一失败不随重复事件刷日志。过期的隐藏请求不能在恢复后再次施加 Low。

Low/Normal 允许脚本继续运行，不混用 TrySuspend/Resume。浏览器开发模式不控制承载后端的隐藏壳窗口；其他平台使用前端生命周期回收，没有伪造原生支持。没有强制 GC、工作集清空、定期主窗重载或自动删除业务历史。

内存目标切换写入现有封闭运行日志，前后端动作与双语标签由同一契约同步维护；普通模块 log 调用会被当前 logger 的目标过滤器丢弃，不能作为切换证据。设置后读取原生目标属性，只有读回一致才确认成功；失败代码保留接口、设置、读回阶段及 HRESULT，重复失败抑制日志但仍允许重试。验证只使用隔离目录及虚构业务数据。

## Related decisions

- [工作区状态所有权](2026-10-13-desktop-workspace-uiux.md)、[有界本地资源](2026-10-14-bounded-local-resources.md)和[托盘闲置销毁](2026-10-28-tray-idle-teardown.md)均部分重叠并互链；原草稿、会话、队列、磁盘维护与浮层生命周期继续成立，没有完全被替代或需要归档的旧篇。
- [进程资源监控](2026-10-23-native-process-resource-monitor.md)继续拥有进程归属和指标口径；其隐藏停采样的资源页不能作为后台回收实验采集器。
- 真实测试同时确认 PDH 格式化数组省略同名实例的 #N 序号，按名称建表会覆盖 WebView2 进程。采集器修订由上述进程资源监控笔记拥有；没有将少算进程的旧读数当作回收收益。
- [会话读取索引](2026-10-22-incremental-session-read-index.md)与[用量快照读取](../bug-fix/2026-10-29-usage-snapshot-first-refresh.md)的并行改动不由本篇覆盖。其他安装器、磁盘归档、构建缓存和协议笔记不拥有本次运行时回收；没有同主题待否决提案。

## Alternatives considered

- **整页卸载或顶层页面 LRU** 的最强理由是一次性移除整个 DOM 和所有子组件状态，策略集中。但当前 Skill/MCP 编辑、供应商编辑、来源上下文和滚动位置分散在明确的领域所有者中；直接卸载会丢状态，另建全局草稿仓库又产生第二所有者。七页的固定集合也不足以支持先引入通用缓存框架，因此选择保留状态所有者、定点释放载荷。
- **统一改用 React Activity** 可以保留状态并自动清理隐藏子树的 Effects，当前 lockfile 和本地 React 均为 19.3.0，版本不是障碍。但它保留 DOM 和状态，不代替大数据清空，也不会自动停止 iframe 内部活动。已有一些释放逻辑写在 active=false 时的 Effect 主体中，整个包进隐藏 Activity 会使这些主体不再运行。当前实现继续显式可见性接入；Activity 只适合另行核对 cleanup 契约后的局部用途。
- **TrySuspend/Resume** 的最强理由是连脚本、计时器和渲染活动一起暂停，闲置成本可能更低。但当前完成通知与状态同步位于渲染器，冻结会暂停这些行为；该接口还要求 controller.IsVisible=false，原生窗口最小化不能直接当成这个条件。当前选择允许脚本运行的 Low/Normal；Microsoft 也不建议混用两组接口。
- **JS 堆阈值或操作系统内存压力触发** 能比固定闲置规则更贴近机器状态。`performance.memory` 非标准、已弃用且不覆盖全部进程；替代测量 API 有跨源隔离和运行时支持约束。Windows 提供正式的系统级内存资源通知，但那不是本应用自身的堆占用，接入还增加跨平台政策和抖动处理。当前以可见性和实际业务寿命触发回收；没有实测依据时不接入系统压力事件或选择 MiB 阈值。
- **强制 GC、EmptyWorkingSet 或定期重载主窗口** 最容易让任务管理器数字暂时下降，或一次移除全部前端状态。但工作集裁剪不删除对象，后续访问会重新调页；重载则破坏当前草稿与通知生命周期。它们不构成有界持有方案，不作为生产路径。
- **React.lazy、虚拟列表或 memo** 分别可以延迟首次代码加载、减少大列表 DOM 和减少计算。它们有各自价值，但 lazy 会缓存加载结果，虚拟列表不消除底层全文载荷，memo 也不释放隐藏缓存；因此不作为本问题的主要解法。

## Consequences

隐藏预览的文档、派生源和完整详情不再由长期挂载的页面持有，后台同步载荷随运行状态而不是回答长度增长。领域草稿和进行中操作保持原所有者，回收不会产生第二个事实库。重新进入预览会从头播放动画，重读历史会产生本地读取延迟；来源结果回收后需要用户再次明确搜索或扫描。

Low 是尽力而为的引擎目标，可能将内存换出并增加恢复成本，不等于 JS 对象已经回收，也不提供固定 MiB 收益或硬上限。iframe 的成本可能不在主页面 JS 堆中；多个 state/ref 指向同一对象时也不能按引用次数重复计算内存。

用户明确要求完整内存测试后，完整 CI 已通过：839 项 Rust 测试、62 项前端测试、类型检查、前端生产构建及笔记校验。4 个 ignored 项是由所属测试启动的子进程夹具入口。前端覆盖详情迟到抑制、千次刷新有界队列、跨详情恢复、失败重试及不使全文失效的时钟；Rust 覆盖轻量观测与详情的时钟/待保存一致性、大回答不进入观测、目标读回与失败恢复、PDH 重名和真实同名进程。

19 项真实组件浏览器场景与 10 项 Windows 原生场景已通过。Windows 内核 10.0.26300 x64、WebView2 156.0.4314.58、1180×780 主窗，使用生产前端资源和桌面 E2E/custom-protocol 的 debug 程序；所有用户目录、SQLite、会话和更新响应均隔离。包含 45 个历史批次、实际分配 16 MiB 数组和 30,000 个字符串的 1.61 MB HTML 回答，以及两个各 70 条长消息的会话。真实关闭到托盘、最小化、恢复、100 次/10 ms 间隔窗口请求、迟到响应、删除与导入保留均验证。

浏览器 40 轮切页后文档/节点/监听器稳定为 1/234/632；第 10 到第 40 轮堆变化约 0.25 MiB。Windows 25 轮真实预览后文档/节点稳定为 3/381，第 5 到第 25 轮堆变化约 0.28 MiB；停止操作 12 秒后临时引擎进程从 22 回到 9。等待实际 320 秒后托盘页面目标消失，完整进程树私有工作集从后台初期约 194.38 MiB 到约 71.60 MiB，随后恢复与预览读取正常。生产 PDH 与独立按句柄采样的 9 个进程逐项一致，最大采样差约 0.39 MiB；没有把其他应用的 WebView2 计入。显式 GC 仅用于部分可达性诊断，五分钟后台样本没有强制 GC，生产也没有加入 GC 或工作集清空。

证据保存在 `.tmp/memory-verification-20261103/ACCEPTANCE.md`、各浏览器/原生 report.json 和 `.tmp/ci-C3D9lL/`。这些是本机受控负载验收，不构成固定节省值、硬上限、数小时稳定性、发布安装包、其他平台或真实供应商验收；未提交、发布或部署。

## Primary references

- [WebView2 MemoryUsageTargetLevel：脚本继续运行、显式恢复与不混用休眠](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2_19)
- [WebView2 TrySuspend：可见性前提与暂停脚本](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2_3#trysuspend)
- [WebView2 性能与内存管理建议](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance)
- [PDH 重复实例与名称边界](https://learn.microsoft.com/en-us/windows/win32/perfctrs/handling-duplicate-instance-names)
- [独立按句柄读取专用工作集及系统版本要求](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex2)
- [React Activity：状态和 DOM 保留、Effects 清理与 iframe 边界](https://react.dev/reference/react/Activity)
- [React lazy：延迟首次加载并缓存结果](https://react.dev/reference/react/lazy)
- [Windows 内存资源通知：系统级低内存信号](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-creatememoryresourcenotification)
- [Windows SetProcessWorkingSetSize：工作集与物理页驻留](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-setprocessworkingsetsize)
- [Performance.memory 的弃用、统计局限与非标准状态](https://developer.mozilla.org/en-US/docs/Web/API/Performance/memory)
- [measureUserAgentSpecificMemory 的支持与安全前提](https://developer.mozilla.org/en-US/docs/Web/API/Performance/measureUserAgentSpecificMemory)
