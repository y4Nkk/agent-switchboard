# Agent Note: 两级供应商编辑器翻级滚动由编辑器 hook 复位

Status: implemented

## Problem

三处供应商编辑器（通用/Claude、Codex 第三方、Codex 官方登录）共享同一个两级结构：编辑表单 ↔ 运行参数页在常驻 `EditorFrame` 内切换。「配置运行参数」按钮位于表单最底部的高级设置区，点击时滚动视口已接近底部；翻级只切换 `hidden` 并交接标题焦点，而标题在固定页头、不在滚动容器内，聚焦不产生滚动。常驻的 `.asb-scroll-viewport` 因此保留旧 scrollTop，参数页从中间或底部开始呈现，用户看不到页首说明。

## Decision

两级导航的滚动与焦点同属编辑器 hook。`useProviderEditor`、`useCodexProviderEditor`、`useCodexOfficialEditor` 各创建 `viewportRef`，经 `ProviderEditorFrame` → `EditorFrame` 透传到 `ScrollArea` 的实际滚动视口；进入参数页时在既有翻级 effect 里先 `scrollTo({ top: 0 })` 再聚焦标题。`ProviderEditorFrame` 的 `viewportRef` 为必填——全部三个会话都传值，可选性只剩「静默漏接」一条软路，必填让新两级会话漏接在类型层面即失败；`EditorFrame` 服务其余九处单级表面（探针历史、用量查询、扩展表单、导入页等），保留可选，与 `titleRef` 同形。返回表单维持现状：聚焦 `triggerRef` 让浏览器把视图带回「运行参数」行，不额外记录表单滚动位置。`.asb-scroll-viewport` 的滚动归属与键盘停靠契约不变。

## Related notes

当场审计活跃笔记：[表格纵向滚动按工作面归属](../../implemented/architecture/2026-10-16-table-scroll-ownership.md)继续拥有 ScrollArea 视口机制，本篇只在其上新增翻级复位时机，互链不重叠；[客户端页签切换动画](../../implemented/architecture/2026-10-16-client-route-transition-owner.md)拥有页签路由动画并在二级视图取消动画，与编辑器内部翻级滚动无关；共享返回顶部笔记不受影响。

## Alternatives considered

- `EditorFrame` 监听 `title` 变化自动回顶：调用方零改动，但把标题文本变化当翻级信号是隐式耦合，今后标题因其他原因变化会误触发滚动，且焦点交接本就归 hook 所有，滚动分离会造成双所有者。
- 给 `ScrollArea` 加 key 在翻级时重建视口：scrollTop 天然归零且无需新 ref，但每次翻级重建整棵表单 DOM，放弃既有 `hidden` 常驻设计与其支撑的触发按钮焦点恢复。
- 在「配置运行参数」按钮 `onClick` 内复位：按钮拿不到视口节点，需透传回调或全局查询，把导航职责从 hook 散落回表单组件。
- `ProviderEditorFrame.viewportRef` 留可选：与 `titleRef` 同形且改动最小，但三个会话全数传值后，可选性不再表达任何真实分支，只保留漏接不报错的兼容口；按无兼容残留准则收紧为必填。

## Consequences

- 收益：三处编辑器行为一致，进入参数页固定从标题下方帮助文案开始；hook 是两级导航（焦点 + 滚动）唯一所有者，无第二实现。
- 代价：`EditorFrame` 增加可选 `viewportRef` 透传，`ProviderEditorFrame` 收紧为必填；三个 hook 返回面各多一个 ref。
- 边界：返回表单不精确恢复表单滚动位置，由触发按钮聚焦近似带回点击处，保留原焦点交接语义。

## Verification

按项目规则未运行编译、类型检查或自动化测试；静态复核透传链（hook 创建 → 两级 frame → `ScrollArea` 的 `useImperativeHandle`）与三个会话组件接线，确认无测试 mock 这三个 hook。`npm run verify-notes` 校验笔记结构。真实滚动行为待用户 dev 实例验收。
