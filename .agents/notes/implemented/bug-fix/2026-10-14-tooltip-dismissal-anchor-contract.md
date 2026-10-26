# Agent Note: Tooltip 关闭契约由 Tooltip 组件自持 anchor

Status: implemented

## Problem

共享 `Tooltip`（Radix）的受控 `open` 只能靠 DOM 事件归零：trigger 的 `pointerleave`、`blur`、按下，以及 content 的外部按下与 Esc。两条真实路径会让这些事件永不发生，tooltip 停留不消失：

1. Chromium 不向 disabled 控件派发指针事件，连冒泡都不给祖先。悬停期间按钮被 `busy`/`querying` 翻成 disabled 后，移开鼠标不产生 `pointerleave`，`open` 永远为 true；此时点击该按钮本身也被吞掉，连「外部按下关闭」都收不到。约十四处调用点是 `<Tooltip><Button disabled={busy}/></Tooltip>` 裸包，只有置顶/更新两处手工包了 `.asb-tooltip-anchor` 并配 `pointer-events: none` 规则，契约靠记忆执行已被证明失效。
2. Radix 1.2.x 没有 window blur 关闭路径（上游 radix-ui/primitives#2102）。tooltip 开着时 alt-tab 或焦点离开窗口，回来后标签仍挂在原位，直到应用内发生下一次点击或滚动。

## Decision

`Tooltip` 组件自持 anchor：内部固定渲染 `<span class="asb-tooltip-anchor">` 作为 Radix Trigger 的 asChild 目标，children 是它的子节点。anchor 是 tooltip 触发边界的唯一 owner：

- `tooltip-feedback.css` 把原生 `:disabled` 和共享复合控件的 `[data-disabled="true"]` 直接子节点移出命中测试，anchor 替它接收全部指针事件；选择器不耦合 `.asb-btn`。Checkbox / Switch 保留原生 input 的禁用语义，通过现有外层标识接入同一命中契约；[扩展共享控件](../architecture/2026-10-24-extension-shared-controls.md)使用这一路径。
- 组件内监听 `window blur` 立即 `setOpen(false)`，补上失焦关闭。
- 调用点不得再手工包裹 anchor span；`UpdateButton` 与 `PinTopButton` 的旧包裹已删除。`dismissedAfterAction` 的抑制语义不变（激活后到指针离开 anchor 前不重开）。
- 侧栏是唯一的拉伸布局消费者：`shell.css` 把导航条目的轨道内边距宽度转移到 `.asb-sidenav-* > li > .asb-tooltip-anchor` 上，`.asb-sidenav-item` 改为填满 anchor。
- `UsageTrendChart` 在数据更换时清零 `activeIndex`，悬停读数不跨刷新存续。

被取代的事实：归档笔记[侧栏导航](../../archived/feature/2026-10-12-sidebar-navigation.md)记载的「共享 Tooltip 直接绑定可转发 ref 的 Button」不再成立，Trigger 现在绑定组件自持的 anchor span；可转发 ref 的控件仍在 anchor 之内。

## Alternatives considered

- 全局 `button:disabled { pointer-events: none }`：最强的理由是一行 CSS 覆盖全应用、不插入任何 DOM；但事件会穿透到父层而不是 trigger 本身，trigger 是按钮时 `pointerleave` 照样丢失，卡住机制原样保留，还会改变依赖 disabled 控件拦截命中的其他交互。不采用。
- 用 `aria-disabled` 加点击守卫替代 `disabled` 以保留指针事件：最强的理由是从根上消灭「disabled 吞事件」，不需要 anchor；但它改写全应用禁用控件的交互与可访问语义（保持可聚焦、可点击、由调用方各自守卫），触及每一个禁用控件，远超本缺陷的边界。不采用。
- 逐个调用点补 anchor span（延续现状）：零组件改动，但契约靠记忆执行，本次审查已清点出十四处遗漏，新增 Tooltip 用法还会继续裸包。不采用。

## Verification

组件检查确认：所有 `<Tooltip>` 调用点的直接子节点由组件统一包裹进 anchor，`UpdateButton`/`PinTopButton` 无残留 span；disabled 翻转后 anchor 仍接收指针事件，移开即关；窗口失焦即时关闭。侧栏条目宽度改由 anchor 承担，展开与收起几何不变。最终差异与笔记结构校验随改动执行；遵循项目限制，不运行编译、类型检查或自动化测试。真实桌面的悬浮、禁用翻转与 alt-tab 场景未运行验收。

## Consequences

任何 `<Tooltip><Button disabled/></Tooltip>` 自动获得正确的开闭行为，调用点不再需要知道 Chromium 的 disabled 事件语义；窗口失焦不再遗留标签；disabled 按钮现在也能显示说明性标签（与置顶/更新按钮既有形态一致）。代价是每个触发控件多一层 inline-flex span 盒（固有尺寸控件不受影响），侧栏宽度规则从按钮转移到 anchor，改侧栏几何时两处一起看。anchor 上的 `aria-describedby` 由按钮移到 span，与既有两处形态一致。
