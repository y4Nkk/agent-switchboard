# Agent Note: 桌面工作区加载与诊断读取

Status: implemented

## Problem

工作区按访问顺序挂载，但首屏静态依赖所有工作区。应用日志每次读取都重新解析保留文件，当前页以外的记录也跨越进程边界。

## Decision

- 工作区模块在首次访问时加载。导航恢复前只显示骨架，已访问工作区由原页面继续持有筛选、草稿和滚动状态。首个工作区提交并经过绘制机会后，记录从 WebView 导航起点算起的本地 User Timing。该时间对应工作区绘制，业务数据就绪由各领域的读者决定。
- 诊断日志按文件身份和完整行偏移维护有界摘要。轮转后的同一文件复用读数，截断或重写重新读取受影响文件，未终止的尾行随下一次刷新重新判定。构建快照时合并当前级别的候选和计数，后端只传当前 20 行及分页信息。最近八份快照支持稳定翻页，后续日志追加在刷新后进入新快照。

## Related decisions

[前端运行时资源回收](2026-11-03-frontend-runtime-memory-lifecycle.md)负责页面已挂载时的载荷寿命。[本地资源边界](2026-10-14-bounded-local-resources.md)负责日志记录范围、保留与清理；本篇接管日志解析复用及分页。

## Alternatives considered

- 静态加载所有工作区可以避免首次进入某个页面时等待模块执行。默认入口无需共同承担其他工作区的模块执行。
- 读取日志尾部可以迅速展示最新记录。应用需要准确报告每个级别的总数和无效行数，文件轮转也可能使物理行次序和事件时间次序不同。

## Consequences

首次访问工作区时加载相应分包。日志读取器依赖应用日志写入方约定的追加和轮转，冷读仍统计全部保留文件。页快照在后端有数量上限，过期的页从界面刷新。当前改动通过静态检查，桌面首屏和日志读取时延需要在当前构建实测。

## Primary references

- [React lazy](https://react.dev/reference/react/lazy)
- [Vite 异步分包](https://vite.dev/guide/features.html#build-optimizations)
- [Vector 文件身份及偏移](https://github.com/vectordotdev/vector/blob/master/src/sources/file.rs)
