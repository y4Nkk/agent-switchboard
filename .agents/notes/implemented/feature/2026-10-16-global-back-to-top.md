# Agent Note: 共享滚动面的一键返回顶部

Status: implemented

## Problem

主工作区、全页编辑器、弹窗、扩展主列表及会话双栏分别滚动，长内容缺少统一返回顶部入口。窗口本身不滚动，调用 window.scrollTo 无法覆盖这些界面；按钮若跟随内容或覆盖固定操作栏，也不能稳定使用。

## Decision

ScrollArea 统一拥有固定定位边界、原生滚动视口及 BackToTop。按钮只呈现向上箭头，使用共享 Button、Tooltip、图标与双语名称；仅当前视口离开顶部时显示，点击保留横向位置并返回该视口顶部，焦点回到视口，减少动效时立即完成。BackToTop 读取真实 scrollTop，通过本地 scroll、ResizeObserver 及组件提交后的实际 overflow 更新显隐，不维护页面位置副本或全局目标注册表。滚动事件只更新一个显隐布尔值，卸载移除监听与观察器；手动滚到顶部时，若按钮仍有焦点，先交给视口再移除按钮。

主工作区、[二级工作区](../architecture/2026-11-23-workspace-subpage-scrolling.md)、AppDialog、Table、扩展列表状态面、会话/收藏列表、详情、正文与大纲全部消费同一组件；配置文件预览、配置字段代码证据、请求回复、上游诊断正文和全局信息列表也通过原语接入。原有表格 horizontal / viewport 契约保持成立，横向模式不出现回顶按钮；窄窗口正文继续跟随详情滚动。菜单、输入框、纯横向代码块与侧栏不属于页面返回顶部入口。

原有布局类留在固定外层，实际内容布局、overflow、gutter、scroll-padding、键盘停靠、阅读引用和 onScroll 指向内层 `.asb-scroll-viewport`；页头、底栏仍是滚动面的兄弟。原直接子级选择器同批更新，旧 overflow/gutter 不留在外层。useReadingScroll 从实际视口恢复和记录位置，取消恢复的手势监听覆盖整个 ScrollArea，包含作为视口兄弟的回顶按钮，避免回顶被延迟恢复覆盖。动效选择只读取现有 `--asb-motion-fast`，不建立第二套偏好。

## Related notes

- [滚动条显隐](../../implemented/feature/2026-10-14-autohide-ink-scrollbars.md)拥有滚动条材质与 data-scrolling，部分相关，本改动保留其事件入口。
- [备份历史随页面滚动](../../implemented/feature/2026-10-14-backup-history-page-scroll.md)拥有备份滚动位置，部分相关，备份继续由主工作区承载。
- [Table 滚动与展开行](../../implemented/feature/2026-10-15-table-owned-scroll-region.md)拥有表格生产契约，部分相关，Table 复用滚动原语但仍拥有表格模式与表头。
- [表格工作面归属](../architecture/2026-10-16-table-scroll-ownership.md)拥有同批并行变更的 horizontal / viewport 契约，部分相关，本篇在当前契约上组合视口，不恢复旧滚动壳或私有高度。
- [会话布局](../bug-fix/2026-09-24-session-operations-layout.md)拥有双栏和阅读结构，部分相关，本篇只收归实际滚动与回顶入口。
- 检索未发现既有返回顶部决定；上述笔记均不被本篇取代。

## Alternatives considered

- 在 AppShell 放一个按钮，寻找最近滚动的 DOM：新增标记少且只有一个全局入口，但会话列表、正文和大纲同时存在时目标不明确，隐藏页和模态还需要维护目标选择、失效与回退规则。
- 各页面自己监听并定位按钮：保留 DOM 最容易，但滚动、显隐、焦点和减少动效形成多份事实，后续页面会漏接。共享滚动面直接拥有这些行为更明确。

## Consequences

页面级和阅读级滚动面复用同一实现；顶部、短内容及纯横向面没有按钮。所有界面均无常显按钮文字或额外装饰。固定操作栏独立于按钮所在边界，窗口与全局滚动条标记的原契约保持；重复 overflow/gutter 及旧直接子级选择器删除，没有临时 DOM 目标猜测、回退或并行实现。

代价是滚动面区分固定容器与实际视口，并维护本地轻量监听与观察器；后续同类工作面直接消费 ScrollArea，不在页面中自行接线。回顶只操作所属视口，不改其他阅读区、文本输入的编辑位置或其他页的业务状态。

## Verification

完成组合差异、滚动归属、旧引用与 React hooks/焦点生命周期静态复核；14 份 CSS 静态解析通过，受影响源文件的相对导入、冲突标记和 500 行限制检查通过，旧直接子级选择器检索零命中，相关 `git diff --check` 通过。`npm run verify-notes` 通过：52 篇活跃笔记与相对链接、7 篇归档及封存摘要通过。

按 AGENTS.md 不运行未明确要求的编译、类型检查与自动化测试，实际 WebView 的滚动与焦点效果不作为本次静态检查的已验证结果。
