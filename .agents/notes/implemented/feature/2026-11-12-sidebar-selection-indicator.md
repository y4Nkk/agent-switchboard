# Agent Note: 侧栏当前页指示条与按下染色

Status: implemented

## Problem

侧栏的当前页只有条目自身的渐变底、品牌蓝图标与加粗文字三个层次，条目之间没有任何跨越条目的选中标记；鼠标按下与悬停共用同一个 `--asb-chrome-hover` 染色，点击缺少独立的中性反馈层次。用户提供 FlClash 的桌面侧栏作为参考：其选中项除自身底色外，在条目左缘常驻一枚品牌色竖向指示胶囊，切换页面时指示条以「先导后随」两段缓动拉伸到新条目，按下反馈也使用比悬停更深一档的中性叠层。侧栏需要补齐这两个点击动效层次。

## Decision

当前页指示条与按下染色由 tokens.css 与 shell.css、`SideNavIndicator` 三处共同承载，事实归属如下：

- `tokens.css` 是唯一几何与尺度事实：`--asb-sidebar-indicator-width`（3px）与 `--asb-sidebar-indicator-height`（16px）定义胶囊几何；`--asb-motion-indicator`（320ms）定义拉伸时长，并进入两个减少动效映射块归零；`--asb-chrome-active`（正文色 10% 染色，悬停的两倍）定义按下中性染色，与 `--asb-chrome-hover` 同样派生自 `--asb-text`，深色主题随正文角色自动加深，不设第二份明暗色板。
- `shell.css` 唯一拥有指示条绘制语法：绝对定位在列表内、左缘对齐条目的 li 内边距、`--asb-radius-pill` 全圆角、底色取 `--asb-sidebar-selected-icon` 品牌蓝、`pointer-events: none` 且 `aria-hidden`；两个导航列表（工作区组与页脚设置组）获得 `position: relative`。条目的 `:hover` 与 `:active` 拆分为两条规则，`:active` 使用 `--asb-chrome-active`，声明顺序保证 `[aria-current="page"]` 的渐变底在悬停与按下时都保持不变；共享更新入口 `.asb-updatebtn` 获得同一按下规则，且置于 is-available / is-checking 之前。
- `SideNavIndicator`（src/app/SideNavIndicator.tsx）唯一拥有拉伸动画：切换目标时以 requestAnimationFrame 在 `--asb-motion-indicator` 时长内驱动，前缘在时长前 60% 内以 easeOutCubic 就位，后缘自 35% 起以 easeInOutCubic 追上——下行时下缘先行、上行时上缘先行，胶囊先拉伸再收缩；快速连点从上一目标重新出发，挂载、读取失败与时长归零（减少动效）时直接落位。它每次触发时从 tokens.css 读计算值，因此减少动效映射同时约束这条 JS 动画，与 CSS 过渡共用同一个偏好事实。

两个列表各持一个 `SideNavIndicator` 实例，指示条只在包含当前页的列表内出现；跨列表切换（工作区页与设置页互跳）时一枚消失、一枚直接落位，不做跨容器拉伸。条目位置用 getBoundingClientRect 相对滚动容器实测（含 scrollTop 折算），不从索引乘步距推算，文字缩放与列表内边距变化不会使它失准；列表上的 ResizeObserver 在布局变化时将静止胶囊瞬时复位，其初始回调只复位挂载已放置的胶囊，不打断运行中的拉伸。指示条不改变布局，不参与可访问树；aria-current 仍是唯一页面事实，指示条与常驻底色、品牌蓝、加粗文字一样只消费它。

## Alternatives considered

- **用纯 CSS 过渡近似拉伸**（top 与 height 分别配不同时延和缓动）：不增 JS，但 height 需要的缓动是两段缓动之差，CSS 表达不了，胶囊圆角也会在拆成两半时断裂；拉伸保真度是这次的需求本体，故由 JS rAF 驱动。
- **单枚指示条横跨两个列表**：与 FlClash 的单列表模型一致，但本侧栏页脚是独立非滚动容器，跨容器拉伸需要浮动层与第二套定位事实；拆成每列表一枚后「另一枚直接落位」语义自然，代价仅为切换容器时少一次拉伸。
- **按下复用当前页渐变**：反馈更强，但按下尚未完成导航，会出现两个当前页样式，[侧栏图标状态笔记](2026-10-12-sidebar-icon-states.md)已否决该方向；按下保持中性，仅加深一档。
- **新建独立的指示条色板**：指示条与当前页图标同属品牌蓝事实，直接消费 `--asb-sidebar-selected-icon`，不为同一颜色设第二个名字。

## Consequences

- 当前页在底色、图标形态、文字与跨条目指示条四个层次可辨，切换页面时有明确的位移引导；按下、悬停、当前页三个状态染色各居一档且互不冒充。
- tokens.css 的动效档位增加 320ms 一档并进入两个减少动效块；JS 动画经计算值读取同一 token，偏好切换即时生效，无第二份时长事实。
- 指示条位置实测依赖 DOM 查询（`:scope > li .asb-sidenav-item`），条目类名是该选择器的契约；改动导航条目 DOM 结构时必须同步 `SideNavIndicator`。
- 布局归属不变：指示条绝对定位、零占位，[桌面工作区笔记](../architecture/2026-10-13-desktop-workspace-uiux.md)拥有的轨道几何与滚动不受影响；设置缩略图（偏好预览）描绘的是静态窗口骨架，不含指示条。
- 验证覆盖：笔记结构经 `npm run verify-notes`；指示条几何、染色档位与声明顺序按源码核对。动画时序与拉伸观感未跑自动化——类型检查、测试遵循仓库显式授权规则，视觉结论需在运行实例中人工确认。
