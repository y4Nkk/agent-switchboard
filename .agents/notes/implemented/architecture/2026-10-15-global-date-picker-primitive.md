# Agent Note: 全局日期选择原语 DatePicker 统一时间选择

Status: implemented

## Problem

时间选择此前两处各自用 `<Input type="date">`（存储归档截止日期、会话库起止筛选）：外观由浏览器 UA 决定（WebView2 的原生日历弹层不可定制、与 Frosted Relay 设计语言脱节）、行为不可控（弹层状态与键盘行为随平台 UA 走），且无法保证只产出合法日期——会话筛选因此携带一整套手输垃圾的防御分支：`dateBoundary` 的格式与历法校验、`sessions.time.invalidDate` 文案、`aria-invalid` 接线。项目没有日期选择原语，`DESIGN.md` 也没有其所有权条目。

## Decision

日期为空是可由用户产生的合法值。共享 `DatePicker` 在弹层内提供清空动作，发出空串并归还触发器焦点；存储归档与会话起止筛选共同消费这条路径。会话范围切为「全部」时同时清空起止草稿和实际筛选，不再恢复隐藏的旧范围。

`src/components/DatePicker.tsx` 是软件内全部日历日期选择的唯一原语，`styles/base/date-picker.css` 是选择器语法的唯一拥有者：

- 行为拥有者：react-aria / react-stately 的日历行为 hooks——`useCalendarState`（选中与聚焦状态机）、`useCalendar`（容器 aria、上月/下月、本地化标题、可见范围与选中变化的播报）、`useCalendarGrid`（grid 角色、网格键盘导航：方向键逐日、PageUp/Down 换月、Home/End 换周）、`useCalendarCell`（roving tabindex、按压/选中 aria）；`I18nProvider locale` 绑定界面语言（`useI18n`），星期/月份/按钮文案用内置翻译。选择一个日期即关闭弹层并经 RAC `Popover` 归还焦点到触发器（项目既有弹层惯例）。
- 结构与外观拥有者：本仓库。触发器、月历表格、单元格的 DOM 全部由本原语构建（hooks 只回传 DOM-ready props，无高层组件、无隐式 context 接线）；触发器复用 `.asb-select-trigger` 字段形（与 Select/FontPicker 同一材料），所选日期文本走共享 `dateLabel`（时间显示体系）；弹层复用菜单材质（`surface-menu`、hairline、`shadow-modal` + `inset-highlight`）；单元格状态反馈由本原语显式写出的 data 属性表达（`[data-selected]`、`[data-today]`、`[data-outside-month]`、`[data-focus-visible]` 等，today/outside-month 由 `@internationalized/date` 判定），减少动态效果偏好由专属 reduce 块关闭过渡；弹层打开即聚焦 roving-tabindex 的当前日期格。
- 值契约：`value`/`onChange` 用 `YYYY-MM-DD` 字符串，空串 = 未选（触发器显示 `format.date.unset` 占位）。弹层提供清空按钮，发出空串并关闭、归还触发器焦点；无 min/max、不选时刻，不增加手输旁路。
- 直接依赖：`react-aria`（行为 hooks）、`react-stately`（日历状态机）、`@internationalized/date`（日期数学与 `parseDate`）。后两者此前只是 react-aria-components 的传递依赖，声明版本与其范围一致；`react-aria-components` 仍被 RAC `Popover`/`Dialog`（弹层惯例）与其他既有消费点使用。

消费点包含存储归档截止日期、备份清理截止日期、会话库起止筛选与「按日期处理」。随选择器只产出合法日期，会话筛选的手输防御同步删除：`dateBoundary` 收敛为纯换算器（仅保留 9999-12-31 的 +1 天溢出守卫，防 `toISOString` 抛错，此时按无上界处理），`sessions.time.invalidDate` 文案与 aria-invalid 接线删除；`from > to` 的范围错误（`sessions.time.invalid`）仍然可达，保留。`DESIGN.md` 组件清单新增 `DatePicker` 条目。

[会话批量操作弹窗](../feature/2026-11-24-session-batch-dialog.md)复用带日期范围的会话筛选。日历使用[AppDialog 提供的字段弹层容器](2026-11-24-session-dialog-control-boundaries.md)，沿用对话框的遮罩与焦点边界。

同主题笔记审计：[Time date 模式](2026-10-15-time-date-mode-storage-cutoff.md)「范围边界」段随本决策原地更新（会话筛选不再是 Input 原语消费点）；无其他笔记持有时间选择契约。

## Alternatives considered

- 继续原生 `<Input type="date">` 加全局 CSS 定制：最强理由是零依赖、零行为风险。否决：WebView2 的原生日历弹层无法定制外观，打开体验与设计体系脱节；弹层的打开方向、外点关闭、焦点归还不可编程，键盘行为随平台 UA 变化，无法形成可审计契约。
- 直接组合 react-aria-components 的 `DatePicker`/`Calendar` 高层组件（初版实现）：最强理由是弹层开关、焦点归还、选择后关闭全部由组件内建，组合代码最短。否决：高层组件为完整 DateField 场景服务——携带 Label/Text/FieldError/Group context、HiddenDateInput 与表单校验管线（我们全部不用，需 `validationBehavior="aria"` 压制）；触发按钮靠 ButtonContext 隐式接线（放进 DatePicker 树的任意按钮自动变成弹层开关）；表格 DOM 由组件生成，结构不属于我们。下沉到行为 hooks 后这些机器整体消失，每个接线（触发器按压、弹层开关、单元格状态）都是显式 props，DOM 完全自有——行为同样是 Adobe 维护的同一套实现，无损失。
- 在 Radix 体系上自建日历（项目已有 react-select/tooltip）：最强理由是与既有 Radix 依赖同源。否决：Radix 没有日历原语，键盘网格导航、周起始、本地化星期全要自己写；而 react-aria 已是项目依赖，行为成熟度（完整 a11y 与 ICU 本地化）不可比拟，引入第二套弹层行为来源反成技术债。
- react-day-picker 等第三方完整日历组件：最强理由是开箱即用且含 UI。否决：自带样式体系与 tokens 冲突，覆盖成本大于收益；行为层（焦点管理、locale）仍不如 react-aria 完整，且违背「行为一个拥有者」。
- 触发器旁保留手输输入框（picker + input 组合）：最强理由是高级用户键入更快。否决：双输入源意味着恢复全套格式校验和两条状态同步路径——正是本次要删除的技术债；月历内的键盘导航已覆盖键入需求。

## Consequences

- 收益：两处日期选择获得同一外观、同一键盘与 aria 行为；非法日期在输入侧不再可能出现，会话筛选防御分支删除；日历随界面语言本地化（含周起始与内置文案）。
- 代价：新增 3 个直接依赖（`react-aria`、`react-stately`、`@internationalized/date`）；月历表格 DOM、打开聚焦与 today/outside-month 判定由本原语拥有，是自担的维护面；升级 react-aria 需关注 hooks 返回的 props 形态。
- 后续任何日期选择场景必须经 `DatePicker`；不得再引入 `<Input type="date">`、第三方日历或平行月历结构。修改触发器、弹层材质或单元样式只改 `date-picker.css`。

## Testing

- 笔记结构随本批改动校验；日期清空和切换「全部」的消费链路以源码核对，未运行桌面行为验收。
- 按工作区规则未运行 typecheck/test（用户未要求）；人工核对：全库 `type="date"`、`sessions.time.invalidDate`、旧格式校验分支、`.asb-date-group` 死选择器零残留；hooks 接线逐项对照 node_modules 源码——`useCalendar` 经 hookData 把 aria-label 传给 grid、`useCalendarGrid` 自带网格键盘导航与本地化 `weekDays`、`useCalendarCell` 返回 DOM-ready buttonProps（内部 usePress）与 roving tabindex、`useButton` 转换上月/下月 AriaButtonProps、单选 `onChange` 签名为非空 `CalendarDate`；`getDatesInWeek` 的 null 占位（历法边界）渲染空 td。
