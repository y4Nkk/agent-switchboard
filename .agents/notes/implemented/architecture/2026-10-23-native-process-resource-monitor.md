# Agent Note: 编码进程资源监控的原生采样与唯一归属

Status: implemented

## Problem

模型用量、额度和检测结果不能说明本机软件的 CPU、内存和进程规模。设置中的运行环境只拥有应用元数据，存储管理只拥有磁盘容量和维护；向这些读取器追加进程采样会混合生命周期和事实来源。

按进程名称或全部后代直接求和，会误认 Node/WebView、把桌面外壳计入编码引擎，或者将本软件发起的 Codex 检测同时计入两款软件。PID 复用、exec、更换父进程、权限限制和平台界面进程差异又会让基线、归属及零值失真。

## Decision

资源占用归用量工作区的独立 `resources` 页签，顺序为消耗、额度、资源、雷达。固定三组为 Agent Switchboard、Codex、Claude Code；用户确认的边界是编码进程和任务子进程，包括桌面界面中运行的本机编码进程。桌面/编辑器/终端外壳和整个 WSL、容器或远端环境不整体计入。

`src-tauri/src/resource_monitor/` 是身份、归属、差分和指标完整性的唯一所有者。三平台原生采集共用一个契约，前端只保留当前阅读选择、最新三行汇总和最多 50 条明细。运行配置、原始会话、凭据和业务数据库不参与采集或写入。没有资源历史数据库、常驻采样线程、旧 API 别名、旧客户端 JS 兼容表或第二条采样路径。

### 原生实现与覆盖

| 平台 | 当前采集来源 | 事实边界 |
| --- | --- | --- |
| Windows | ToolHelp；进程用户 SID、映像路径、完整创建 FILETIME；GetProcessTimes；GetProcessMemoryInfo 工作集；全部处理器组的在线逻辑核数 | WebView2 Environment8 关联当前应用环境，合并主窗和托盘并去重；API 不列 crashpad，只有可证实父链才补入 |
| macOS | libproc 的进程表、BSD 身份、路径和 task 资源；Mach 时间基准换算 | 公开接口不能证明全部 WebKit 辅助进程归属，界面覆盖明确为 partial；不调用私有 selector |
| Linux | 当前 `/proc` 的用户、exe、stat 身份/父子关系、CPU tick 和 RSS 页数 | 核验 `/proc/self` 与应用 PID namespace 一致；权限、namespace 边界和近似 RSS 明示；可证实的 WebKit 后代正常计入，界面整体仍标部分覆盖 |

采集复用现有 `windows-sys`、`windows`、`webview2-com` 和 `libc`。Windows 清单明确声明 ProcessStatus feature，没有新增 crate 或包版本变动。原生计数不通过 PowerShell、CIM、`ps` 或 `tasklist` 子进程执行。

界面线程只取得 WebView 关联的进程键，包含 PID 与当时的原生创建标识；阻塞采集再次比对身份和 WebView 映像。一个进程身份读取失败不丢弃已核实的其他界面进程。原生枚举与资源计数在阻塞池运行，不持有采样状态锁等待 UI 返回。

### 身份与归属

- 系统拓扑只包含 PID、PPID 和候选名称。身份和资源只读取候选编码程序、自身界面、必要的已确认后代及上轮记录，不读取全机命令行、环境或内存映射。
- 客户端根由规范运行路径与当前 PATH 原生入口、官方平台包、Claude 原生版本目录或实际编码 bundle 路径相互印证。PATH 只提供候选，不能限制为第一份安装；可执行内容必须是原生格式。当前 Claude npm 分发是原生程序，Codex npm 的 Node 启动外壳不向上并入编码引擎。
- 已核实的相同进程身份和映像直接复用根归属；只有新的或改变的候选才创建入口目录。目录扫描不成为每两秒重复的固定成本。
- 进程键为 PID 加原生出生标识：Windows 完整 FILETIME、macOS 秒/微秒、Linux starttime。跨 IPC 使用不透明字符串，避免 JavaScript 整数精度损失。
- 最近的已识别软件根拥有进程。Agent Switchboard → Codex → 构建任务的链中，后两者只属于 Codex；同组多实例和嵌套根取并集，一个进程键在一轮中只有一个所有者。
- 无法核实的候选编码根和明确排除的外壳形成归属边界，其子树不能继续向上计入 Switchboard。边界内部另有已确认编码根时，仍由该根独立拥有任务。
- 父子连接核验身份和出生顺序，断裂、循环和被复用父 PID 不建立新归属。重新托管的进程只有出生标识和映像仍相同时才保留此前证实的归属；新的更强软件根优先。
- 归属、映像和 CPU 基线保存在同一份当前进程记录中；每轮替换并释放已退出记录。软件归属变化使 CPU 区间失效，不能把旧软件的累计时间记给新软件。
- 共享服务不因名称或连接关系被认领。两轮间完全启动并退出的短命任务不在当前占用快照的完整性承诺中。

### 指标语义

CPU 按 `100 × Δ(userCpu + kernelCpu) / (ΔmonotonicTime × onlineLogicalCpuCount)` 归一化到整机容量。Windows 时间单位是 100ns，macOS task 计数按 Mach timebase 转纳秒，Linux 按系统 tick 单位转换。累计时间、真实每进程区间和在线核数均由后端拥有；前端不硬编码两秒分母或另存计数器。

首次、重新进入、间隔异常、睡眠或墙钟变化、计数回退及 CPU 拓扑变化均显式重建区间。成功测得的零值才是 0%；预热、拒绝访问、已退出和不可用不序列化为零。调度计时粒度造成接近满载的微小舍入允许收敛到 100%，明显越界则拒绝本轮 CPU。内存和进程数不因 CPU 预热一并失败。

驻留内存按字节在后端汇总，界面按 MiB/GiB 显示；包含共享页，不等于去重物理内存或关闭可释放容量。Linux RSS 使用内核近似计数。每个指标独立表达 measured、partial、unavailable，包含已测/缺失进程数量及具体原因；未知全量不能变成一个无条件总量。

相关枚举和识别成功且没有进程才显示“未检测到”；候选身份受限显示“无法确认”。平台或权限限制存在时显示“已识别进程合计”，局部读取失败保留正常指标。

### 单一读取与界面生命周期

`read_resource_usage` 接收明确的 `resetInterval` 和可选明细 `{ group, page }`；请求与明细拒绝未知字段，页码从 1 开始，页大小固定 50。Rust 类型位于 `resource_monitor/types.rs`，TypeScript 映射及调用位于 `src/api/resource-usage.ts`，统一由 `client.ts` 导出。原生命令注册与浏览器开发桥接共用同一读取实现。

响应拥有单调 sampleId、展示时间、nextReadInMs、平台覆盖、固定三组汇总和选中明细；汇总与明细来自同一不可变样本，越界页码由后端收敛。原生命令在进入阻塞池前合并在途读取，重叠调用等待同一原始结果，再按各自明细选择投影；期限内复用当前样本。超时只报告采样仍在继续，底层任务真正结束前保持唯一在途所有权。

`use-resource-usage.ts` 的可见生命周期复用共享 `useReadQueue`。`resource-usage-reader.ts` 统一入口、手动刷新、明细变更、约两秒期限和失败五秒重试；一个执行中任务、一个最新待执行请求。隐藏或离开立即取消未开始的请求和定时器，代次拒绝迟到结果；重新进入建立 CPU 基线，明细翻页和手动刷新不重置基线。失败保留带原时间的上次结果，不把它计入新样本。

原生主窗口可见性由 `workspace_visibility.rs` 读取实际 visible/minimized 状态并发布失效事实；显示、托盘隐藏、关闭吸收、快捷键和窗口事件均接入同一入口。`use-workspace-visible.ts` 合并原生状态与 document 可见性，所有原有工作区共用一个订阅来源；失焦不等于隐藏。初始查询与事件按顺序核验，过期订阅不能写入当前来源。资源命令还在原生侧拒绝隐藏窗口的读取。浏览器开发独立使用 document 可见性，明确只计原生后端、不认领用户整个浏览器。

主表及明细复用共享 Table、Collapsible 和 Pagination；一次展开一款软件。动作位于页头导航右侧，采样时间为次级状态，不将每两秒数值变化逐次播报给辅助技术。读取受限在所属指标和行说明，不向全局信息持续刷通知。README 两种语言与 DESIGN 同批更新当前事实。

### 相邻决定

- [本地资源的有界读取与显式回收](2026-10-14-bounded-local-resources.md) 继续拥有既有读取边界、磁盘容量和维护，本篇增加当前进程度量，不接管或归档该决定。
- [桌面工作区 UI/UX](2026-10-13-desktop-workspace-uiux.md) 继续拥有导航与页面状态，本篇补充共享可见性来源，不建立独立导航或可见性副本。
- [会话与用量读取索引](2026-10-22-incremental-session-read-index.md) 继续拥有持久调用索引；资源占用不接入其数据库，不混合计费口径。
- 提案转入本实现笔记；没有其他同主题旧实现被保留，没有需要迁移的资源历史或旧资源 API。

## Alternatives considered

- `sysinfo` 提供成熟的跨平台枚举和指标接口，能减少 FFI；但研究时核实的 Windows 内存读取失败可能留下初始零或旧值，公开数值接口不足以表达本轮逐指标错误。外加原生校验会形成重复采集，因此维护三个窄平台实现，保留错误及出生标识。
- PowerShell/CIM/`ps` 易于独立验证和快速接入；但进程启动、服务权限、文本格式、超时和单位差异会进入常态读取路径，因此不作为产品采样入口。
- 常驻后台采样再广播事件适合连续性能历史和离页曲线；当前目标是可见时查看占用，因此使用拉取，避免常驻扫描、租约、订阅和历史数据库。
- 按名称和全部后代求和最容易覆盖常见场景；但会误认通用运行时、共享服务并重复计算检测进程，因此只使用可证实身份和最近软件根。
- 各平台分别使用 private bytes、footprint、PSS 更接近各自系统工具；但数值含义不相同，会掩盖跨平台口径差异，因此使用驻留内存并常显共享页和 Linux 近似性说明。
- 私有 WKWebView selector 可得到部分进程 ID；但它是非公开契约，也不能证明所有共享辅助进程的独占归属，因此无法确认时明确展示子集。

## Consequences

- 当前源码拥有三平台采集、唯一归属、指标有效性、同源分页、窗口隐藏控制、命令与开发桥接和双语页面；旧的运行环境及存储管理不承担第二套资源事实。没有新依赖版本、设置迁移、持久化历史或真实客户端写入。
- 原生 FFI 的维护成本高于单一跨平台库；平台 API、安装布局或 OS 计数变化必须更新对应实现和夹具，不能增加猜测或双读路径。
- macOS/Linux 界面覆盖当前是可证实子集，不能把它包装成整款软件全量。权限、命名空间和短命任务也限制观测范围；扩展到持续追踪或跨 WSL/远端采集会改变产品范围。
- 驻留内存含共享页，Linux 计数近似；页面数值不形成释放空间承诺，也不承诺等于系统工具默认私有内存列。
- `resource_monitor/tests.rs`、`sampling/tests.rs` 及平台内测试记录嵌套软件根、PID/映像变化、父 PID 复用、断裂/循环父链、错误不变零、严格请求、并发单槽、实际 CPU 区间、软件归属改变、墙钟/睡眠、明细分页和 Mach 单位的回归用例。
- `tests/resource-usage-reader.test.ts` 记录可见进入、周期读取、隐藏停止、最新明细选择、迟到响应、失败保留与重试的用例。测试不发起模型请求，也不写真实配置。
- 原生读取烟测在各平台直接枚举并读取当前测试进程，核实稳定的创建身份、映像路径、驻留内存、在线处理器数和不倒退的 CPU 累计计数；它补足模拟观测夹具无法覆盖的 FFI 与权限边界，不写真实配置。

### 验证边界

Windows 本地隔离 CI 通过笔记门禁、44 项前端测试、723 项 Rust 测试、类型检查与生产前端构建；资源归属、分页、采样有效性和当前真实进程读取用例均通过。两个被忽略的 Rust 用例是由所属测试显式调用的子进程入口。配置、凭据和本地应用目录均指向隔离临时路径，不运行真实模型请求。

最初受限执行环境的本机 HTTP 用例不能连接，停止该次隔离任务后在允许本机连接的执行环境重跑共用门禁通过；没有为环境限制放宽产品逻辑或测试断言。源码审查、自动化测试与安装交互分别报告，不互相替代。

尚未实测的运行验收包括：三平台真实原生安装识别；Windows 主窗/托盘 WebView 去重与检测跨软件归属；macOS/Linux 覆盖提示；最小化、托盘隐藏、快捷键恢复和失焦仍可见；同口径系统数值对照；重复进出不累积句柄、任务或快照。受控负载仅使用隔离夹具，不自动调用模型或写客户端文件。

性能目标仍待实测：约 1000 个系统进程、两秒刷新时，采样 p95 ≤200ms，监控增量 CPU <整机 0.5%，额外常驻内存 <16MiB。当前没有声称这些目标已通过，也不通过放宽阈值隐藏缺口。

### 外部研究依据

- [Tauri 进程模型](https://tauri.app/concept/process-model/)、[WebView2 Environment8](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2environment8)：原生核心、环境进程集合及 crashpad 边界。
- [GetProcessTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes)、[GetProcessMemoryInfo](https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getprocessmemoryinfo)：创建时刻、累计 CPU、工作集与失败。
- [Apple proc_info.h](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/proc_info.h)、[bsd_kern.c](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/kern/bsd_kern.c)：进程身份、task 累计 Mach 时间及驻留计数；换算使用当前 SDK timebase。
- [Linux proc 文档](https://www.kernel.org/doc/html/latest/filesystems/proc.html)：身份、PID 复用、权限和 RSS 近似性。
- [Codex npm 入口](https://github.com/openai/codex/blob/main/codex-cli/bin/codex.js)、[Claude Code npm 分发](https://code.claude.com/docs/en/setup#install-with-npm)：当前原生启动边界。
- [sysinfo Windows 实现](https://github.com/GuillaumeGomez/sysinfo/blob/master/src/windows/process.rs)、[WKWebView 私有头文件](https://github.com/WebKit/WebKit/blob/main/Source/WebKit/UIProcess/API/Cocoa/WKWebViewPrivate.h)：被否方案的错误保真与非公开接口约束。
