# Agent Note: 表格纵向滚动按工作面归属

Status: implemented

## Problem

共享 Table 对所有使用方创建 `overflow: auto`，没有区分自然展开的模块表格与占满工作区的主列表。扩展库在 Table 外再次纵向滚动；弹窗主体是可收缩的 flex 容器，直接子表格可能收缩出第二条纵向滚动条。日额度明细的 18rem 上限和用量统计的私有滚动、sticky 规则也绕开了统一归属。

## Decision

Table 唯一拥有表格标记、行内展开和横向平移，默认 `scrollMode="horizontal"`：`flex: none` 保留内容高度，`overflow-x: auto / overflow-y: hidden` 只提供横向平移，随页面、WorkspaceSubpage 或 AppDialog 主体自然展开。只有以表格为主体的有界工作区显式选择 `scrollMode="viewport"`，由 Table 包裹层占满剩余空间并统一拥有两轴滚动、固定表头、键盘焦点和稳定槽位；模块不再私自给包裹层限定高度或固定表头。viewport 的 scroll-padding 为键盘焦点留出表头空间，滚动边界不向外层传递，内收焦点轮廓避免被工作区裁切。

扩展主列表直接选择 viewport，移除外层滚动壳；加载、失败和空态在同一个位置使用状态滚动面，结果数量留在列表下方。多模块管理面使用 AppDialog 或 [WorkspaceSubpage](2026-11-23-workspace-subpage-scrolling.md) 提供的视口。用量表与日额度明细按默认模式展开，移除旧私有高度与 sticky 规则。

Table 和列表状态面均组合 ScrollArea：外层负责布局，内层 `.asb-scroll-viewport` 承担原生滚动与键盘停靠，返回顶部按钮固定在外层右下角；表格模式对应的 overflow、overscroll、gutter 与 scroll-padding 应用于内层，sticky 选择器包含这层实际视口。

## Related notes

- [共享返回顶部](../feature/2026-10-16-global-back-to-top.md)补充滚动原语与按钮，保持本篇的模式与纵向归属决定。
- [Table 的滚动面与展开行](../../implemented/feature/2026-10-15-table-owned-scroll-region.md)部分重叠：继续拥有唯一表格 DOM、横向平移和 renderExpansion，本篇替换其无差别两轴滚动及模块私有高度、sticky 的决定；两篇保留独立理由并互链。
- [备份历史页面滚动](../../implemented/feature/2026-10-14-backup-history-page-scroll.md)决定保持，默认模式落实其自然展开契约。
- [滚动条显隐](../../implemented/feature/2026-10-14-autohide-ink-scrollbars.md)保持，无需改动视觉所有者。其他检索命中为网关宽度或扩展操作事实，与纵向归属无关。

## Alternatives considered

- 保留通用 auto，仅删除扩展库外壳：改动最小，但弹窗 flex 收缩、日额度高度限制仍能引入模块内纵向滚动，无法确立统一契约。
- 所有表格都随页面展开：契约最简单，但主列表会带走搜索与分类上下文，不能满足已确认的扩展列表工作面。
- 从祖先类名推断滚动模式：业务调用零改动，但把布局意图藏在选择器中，用量页这类结构变化会留下失效规则，因此使用一个显式类型化模式。

## Consequences

默认表格在 flex 主体内不再收缩为独立纵向滚动区；直接消费者无需逐个声明自然展开。扩展列表没有外层滚动壳，状态面与表格互斥，过滤结果数量位于固定注脚。扫描使用二级工作区，来源浏览使用市场结果区。旧 data-view 滚动分支、日额度高度上限与用量私有 sticky / flex 规则删除。

代价是长日额度明细增加页面高度。viewport 只适用于已有有界 flex 高度链的主列表；如果今后出现多个并排独立工作区，应明确各自工作面而非给模块表格添加任意高度上限。非溢出表格仍保留共享键盘停靠点。

## Verification

`npm run verify-notes` 通过：52 篇活跃笔记的结构与相对链接、7 篇归档及封存摘要通过。相关差异 `git diff --check` 通过；源代码检索确认旧扩展滚动壳、data-view 分支、用量私有表头与高度规则移除，viewport 仅由扩展主列表显式使用，其滚动和 sticky 规则只在共享样式定义。全库数据表格仍只有 Table 一处标记，另一个 `<table>` 属于 DatePicker 日历。React 静态复核确认 hooks 无条件调用、无新增状态或副作用、滚动面有键盘停靠与可访问名称。

按项目规则不运行当前请求未授权的编译、类型检查和自动化测试；真实 WebView 的长列表、窄窗、矮窗、键盘及横向平移效果尚未验收。
