# Agent Note: FlClash 侧栏交互重构

Status: implemented

## Problem

侧栏的大圆角蓝紫底抢过内容层级；已有指示条直接作为列表 li，继承条目横向 padding，声明的 3px 宽度不能代表实际绘制宽度。当前页底色覆盖 hover/active，点击当前页没有反馈；快速切换从上一目标重新开始，ResizeObserver 也可能覆盖正在运行的动画。用户要求参考 FlClash 完整重构。

## Decision

采用 [FlClash 的 prism_ui 侧栏](https://github.com/chen08209/prism_ui/blob/main/lib/sidebar.dart)的中性选中底、较小圆角、独立反馈叠层、320ms 双边拉伸与 250ms 展开节奏。保留现有 224px/56px、40px 点击目标和应用级持久化所有者，不引入 Flutter、动画库或新配置路径。

tokens.css 拥有中性明暗选中底和动效尺度，删除蓝紫渐变变量；设置缩略图同步消费单色。shell.css 统一导航、更新与折叠按钮几何和独立反馈叠层，指示条排除条目 padding。图标交叉淡化 regular/fill，选中时短暂轻弹，初次挂载不播放。

SideNavIndicator 按 aria-current 查找实际目标，帧间只保存当前上下边缘，快速切换从已绘制边缘继续。布局尺寸实际改变时取消并直接对齐；滚动不触发重定位，初始 ResizeObserver 回调不打断动画。运行中减少动效、窗口失焦/隐藏和卸载均结束动画，不留帧循环。

SideNav 通过稳定的 callback ref 发布两个实际列表节点，SideNavIndicator 接受节点而非尚未挂载的父级 RefObject。后代的 layout effect 可能早于祖先 DOM ref 绑定；只读 ref.current 会让首次定位与观察器安装都提前返回且不重试。节点发布触发一次布局内更新，使初次指示条在容器存在后直接落位，不用计时器或等待帧兜底。

定位统一使用列表内部的布局坐标。开发预览的界面缩放由根节点 CSS zoom 表达，getBoundingClientRect 返回已缩放坐标，直接写回 top 会重复放大。通过实际行的边界高度与 offsetHeight 比值消除缩放，再叠加布局坐标的 scrollTop；竖条几何仍只消费 CSS token，不另建缩放状态或分平台路径。

## Related decisions

- [图标状态](../../implemented/feature/2026-10-12-sidebar-icon-states.md)部分重叠：保留 Phosphor、单一 aria-current、固定明暗色板和设置预览；本篇取代蓝紫渐变、20px 导航圆角及瞬时图形切换。
- [指示条](../../implemented/feature/2026-11-12-sidebar-selection-indicator.md)部分重叠：保留 3×16px、320ms、实测定位与每列表一枚；本篇取代上一目标起点、按下覆盖和初始观察回调行为。
- [桌面工作区](../../implemented/architecture/2026-10-13-desktop-workspace-uiux.md)与[连续原生外框](../../implemented/architecture/2026-10-14-unified-native-glass-shell.md)继续拥有外框空间与材质，本篇只将导航展开时长改为 250ms。
- [侧栏持久化](../../implemented/feature/2026-10-23-sidebar-state-persistence.md)不变；检索命中的偏好预览、Tooltip、标题栏和共享折叠原语仍各自拥有独立职责，无同主题待处理提案需要归档。

## Alternatives considered

- 保留蓝紫底，只修指示条：最少改动且保留原视觉身份，但仍有用户指出的大胶囊和选中反馈缺失，不满足完整重构。
- 整体搬入 Flutter/第三方动效框架：可以直接复用参考控件，但现有应用是 React/Tauri，导航的几种交互可由 CSS 与短生命周期动画直接完成，新增框架会形成第二套布局和状态。
- 固定尺寸 CSS 滑条：能由浏览器接管插值且代码少，但不表达参考的先导/后随拉伸；继续保留局部帧动画，只修正实际起点与取消边界。

## Verification

- 源码核对当前页底色与 ::before 反馈分离、按下零延迟、更新成功色的选择器优先级、图标初次挂载不轻弹与 CSS 动效偏好归零。
- 检查指示条排除 li padding、aria-current 实测定位、帧间边缘接续、观察目标几何变化与监听器/帧清理；这不等同于运行时证明。
- 旧渐变端点、radius-navigation、ExpandNavigationIcon 与导航索引消费者在 src 中搜索无残留；差异按 CRLF 规则检查。
- 初次静态预览因浏览器拒绝 file: 协议而未打开。用户随后明确要求去预览环境截图，启动本机 Vite 并直接挂载实际 AppShell、SideNav 与 SideNavIndicator，侧栏状态只在隔离页面内存中持有，没有真实配置读写。预览暴露并修复父级 ref 尚未绑定的首次挂载问题。
- 浏览器实测收起状态指示条 x=8..11、y=300..316（3×16px、padding=0）；当前页图标 x=16..40（24×24px），两者有 5px 间隔。展开与收起截图已保存，窄条与图标分别可见。截图只证明隔离预览组件的静态效果，不证明真实 Tauri 材质、持久化或快速连点的逐帧观感。
- 补充核对深色与根节点 CSS zoom=1.25：主列表竖条屏幕边界为 x=10..13.75、y=375..395，当前行 y=360..410，中心均为 385；收起并以 Enter 进入设置后，底部竖条 y=670..690、当前行 y=655..705，中心均为 680。减少动效时读取到 0ms，控制台 error/warn 为空。修复坐标重复缩放后两种容器均居中，深色缩放截图保存为 sidebar-dark-125.jpg。
- 执行用户要求截图所需的 Vite 开发转换和 npm run verify-notes；未运行类型检查、自动化测试或生产构建。

## Consequences

中性底降低常驻色块权重，独立叠层让当前页也有反馈；图标、文字、按下与移动各有明确所有者，删除渐变端点与重复按钮几何。双层库图标增加每行一个 SVG，列表节点绑定增加一次布局内更新；指示条继续依赖导航按钮类名，观察器和监听均随列表卸载释放。跨列表切换直接就位，避免跨滚动边界的第二套浮层定位。真实 Tauri 和运行时连续切换仍未覆盖，隔离截图不能替代它们。
