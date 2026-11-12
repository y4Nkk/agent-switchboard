# Agent Note: 侧栏当前页指示条与按下染色

Status: implemented

## Problem

侧栏的当前页只有条目自身的渐变底、品牌蓝图标与加粗文字三个层次，条目之间没有任何跨越条目的选中标记；鼠标按下与悬停共用同一个 `--asb-chrome-hover` 染色，点击缺少独立的中性反馈层次。用户提供 FlClash 的桌面侧栏作为参考：其选中项除自身底色外，在条目左缘常驻一枚品牌色竖向指示胶囊，切换页面时指示条以「先导后随」两段缓动拉伸到新条目，按下反馈也使用比悬停更深一档的中性叠层。侧栏需要补齐这两个点击动效层次。

## Decision

当前页指示条与按下染色由 tokens.css 与 shell.css、`SideNavIndicator` 三处共同承载。[FlClash 交互重构](2026-11-12-flclash-sidebar-interaction.md)修正行 padding 继承、连续切换起点和观察回调取消边界，并以独立叠层取代按下背景覆盖；本篇继续拥有指示条几何、两段拉伸和每列表独立定位：

- `tokens.css` 是唯一几何与尺度事实：`--asb-sidebar-indicator-width`（3px）与 `--asb-sidebar-indicator-height`（16px）定义胶囊几何；`--asb-motion-indicator`（320ms）定义拉伸时长，并进入两个减少动效映射块归零；`--asb-chrome-active`（正文色 10% 染色，悬停的两倍）定义按下中性染色，与 `--asb-chrome-hover` 同样派生自 `--asb-text`，深色主题随正文角色自动加深，不设第二份明暗色板。
- `shell.css` 唯一拥有指示条绘制语法：绝对定位在列表内、左缘对齐条目的 li 内边距、`--asb-radius-pill` 全圆角、底色取 `--asb-sidebar-selected-icon` 品牌蓝、`pointer-events: none` 且 `aria-hidden`；指示条 li 明确排除行 padding。两个导航列表分别为相对定位容器；导航、更新与展开按钮的 `::before` 统一拥有中性 hover/active 叠层，与当前页底色独立。
- `SideNavIndicator`（src/app/SideNavIndicator.tsx）唯一拥有拉伸动画：切换目标时以 requestAnimationFrame 在 `--asb-motion-indicator` 时长内驱动，前缘在时长前 60% 内以 easeOutCubic 就位，后缘自 35% 起以 easeInOutCubic 追上——下行时下缘先行、上行时上缘先行。帧间保留实际上下边缘，连点从已绘制形状继续，挂载与时长归零时直接落位。运行中减少动效、窗口失焦或隐藏同样结束帧循环。

两个列表各持一个 `SideNavIndicator` 实例，指示条只在包含当前页的列表内出现；跨列表切换时一枚消失、一枚直接落位。条目位置用 getBoundingClientRect 相对滚动容器实测（含 scrollTop 折算），直接查询 aria-current，不从索引推算。ResizeObserver 观察列表与导航按钮，只有实际目标上下边缘改变才取消并复位，初始回调和仅宽度改变不打断拉伸。指示条不改变布局，不参与可访问树，当前页仍只有一个事实来源。

## Alternatives considered

- **用纯 CSS 过渡近似拉伸**（top 与 height 分别配不同时延和缓动）：不增 JS，但 height 需要的缓动是两段缓动之差，CSS 表达不了，胶囊圆角也会在拆成两半时断裂；拉伸保真度是这次的需求本体，故由 JS rAF 驱动。
- **单枚指示条横跨两个列表**：与 FlClash 的单列表模型一致，但本侧栏页脚是独立非滚动容器，跨容器拉伸需要浮动层与第二套定位事实；拆成每列表一枚后「另一枚直接落位」语义自然，代价仅为切换容器时少一次拉伸。
- **按下复用当前页渐变**：反馈更强，但按下尚未完成导航，会出现两个当前页样式，[侧栏图标状态笔记](2026-10-12-sidebar-icon-states.md)已否决该方向；按下保持中性，仅加深一档。
- **新建独立的指示条色板**：指示条与当前页图标同属品牌蓝事实，直接消费 `--asb-sidebar-selected-icon`，不为同一颜色设第二个名字。

## Consequences

- 当前页在底色、图标形态、文字与跨条目指示条四个层次可辨，切换页面时有明确的位移引导；按下、悬停、当前页三个状态染色各居一档且互不冒充。
- tokens.css 的动效档位增加 320ms 一档并进入两个减少动效块；JS 动画经计算值读取同一 token，偏好切换即时生效，无第二份时长事实。
- 指示条位置实测依赖 `.asb-sidenav-item[aria-current="page"]`，不再接受导航索引；条目类名仍是选择器契约。
- 布局归属不变：指示条绝对定位、零占位，[桌面工作区笔记](../architecture/2026-10-13-desktop-workspace-uiux.md)拥有轨道几何与滚动；设置缩略图绘制同色静态指示条。
- 验证覆盖：笔记结构经 `npm run verify-notes`；指示条几何、染色档位与声明顺序按源码核对。动画时序与拉伸观感未跑自动化——类型检查、测试遵循仓库显式授权规则，视觉结论需在运行实例中人工确认。
