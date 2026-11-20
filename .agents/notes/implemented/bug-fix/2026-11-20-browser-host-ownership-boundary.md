# Agent Note: 浏览器宿主的归属边界

Status: implemented

## Problem

资源监控沿父链向上归认任务子进程。应用经 plugin-opener 打开供应商官网等链接时，Windows 的 ShellExecuteEx 会把默认浏览器拉起为应用的直接子进程；浏览器此后作为用户自己的会话长期存活，其整棵树（渲染、GPU、网络、音频，以及挂在浏览器上的原生消息宿主）都沿父链计入 Agent Switchboard。2026-11-20 本机实测：安装实例自身 7 进程约 450 MiB 工作集，界面却显示 26 进程 / 1.2 GiB——多出的 19 进程约 1.7 GiB 全部是当天 09:07 从应用打开 `https://anyrouter.top/` 后存活的 Edge 树（msedge.exe 的父进程即应用主程序）。软件没有变重，监控数字失真。

## Decision

`resource_monitor` 的扫描把用户浏览器宿主映像设为归属边界：`executables::browser_host` 按进程名精确匹配 msedge、chrome、chromium、firefox、brave、opera、vivaldi（大小写与 `.exe` 后缀归一）；`Scan::inspect` 在读取身份前命中即插入 `boundaries` 并返回，不观察、不展开。浏览器子树因此不进入观测集合，`ownership::resolve` 与 `affected_group` 的父链回溯在边界处停止——复用既有边界语义，只新增一类边界成员。

边界只按映像名判定，平台无关地应用于三平台（Linux `xdg-open` 同样会把浏览器挂为应用子进程）。浏览器主程序与 `msedgewebview2` 是不同映像，界面进程的 Webview 根识别不受影响。名单是精确枚举，是这条边界的唯一数据：名单外的宿主映像不构成边界，按既有父链规则归属——这是作用域而非兼容路径，扩展只改这一处名单。

## Alternatives considered

- 从打开侧断开父子关系（detached 启动或经 `cmd /c start` 中转）：最强理由是浏览器从一开始就不是应用子进程，监控无需持有这份特殊知识。但 Windows 无法可靠摘除 ShellExecuteEx 的直接父子关系，中转壳引入额外进程，对已经建立的浏览器会话也无效，归属事实终究由父链决定，否。
- 按命令行区分用户浏览器与客户端驱动的自动化浏览器：最强理由是保留未来客户端自动化任务里浏览器进程的计数。但命令行启发式脆弱，且当前产品没有浏览器自动化能力，按名称截断足以覆盖真实场景，否。
- 只在 Windows 处理：最强理由是实测失真发生在 Windows。但 Linux 的 `xdg-open` 链路产生同样的父子关系，名称检查本身平台无关，统一应用比按平台分叉更简单，否。

## Consequences

- 从应用打开过的浏览器不再计入任何软件组；浏览器内的原生消息宿主（如 Codex 的 `codex-chrome-native-host.exe`）随浏览器一起出局，不再沿父链计入。客户端自身派生的真实任务子进程不受影响。
- [原生进程资源监控](../architecture/2026-10-23-native-process-resource-monitor.md)的身份与归属边界从「未核实候选与排除外壳」扩展为含浏览器宿主，该篇已同步互链；链接打开方式仍由[官网列契约](../feature/2026-09-22-route-card-website-link.md)拥有，本篇不改打开行为。
- 边界是扫描期事实，没有数据迁移、兼容层或旧路径残留；发布后新采样即按新边界计算。
- 验证边界：诊断以只读进程实测完成（父链、映像、命令行、启动时间、工作集逐项核对，7 + 19 = 26 与界面吻合）；按项目规则本次未运行编译与自动化测试，`discovery/tests.rs` 与 `executables/tests.rs` 新增的边界用例尚未执行。
