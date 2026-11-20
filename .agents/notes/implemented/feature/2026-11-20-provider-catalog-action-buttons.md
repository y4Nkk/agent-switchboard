# Agent Note: 供应商目录动作按钮契约——dangerText 变体、「操作」列头与主模型胶囊

Status: implemented

## Problem

供应商编辑器的模型目录表格里，三个行内动作（用作主模型 / 修改 / 删除）共用同一个 ghost 变体：同色、同重、零分隔，删除紧贴修改，而删除并不是只删一行——它连带清空该模型的路由映射，误触后果完全不可见。列头又复用 `modelRoutes.edit`（文案「点击配置」），拿交互指令当内容名，Claude 目录里那一列只有删除按钮，头与内容对不上。运行参数行在自动模式下把同一事实说了三遍（当前设置：自动 / 值「客户端默认」/ 下方 caption「客户端默认」）。主模型状态有两种表达（Codex：禁用按钮 + 模型列下小字；Claude：主模型列文本），再加一个禁用按钮的「用作主模型」，同一概念三种呈现。ghost 的悬停面 padding 为 0，高亮紧贴文字。

## Decision

- `buttons.css` 拥有危险文字变体的全部几何：`asb-btn-danger-text` 在 ghost 几何上叠加危险色文字、`--asb-diff-remove-bg` 悬停底、以及 `:not(:first-child)::before` 的发丝分隔线（行首不显示）。`Button` 的 VARIANT_CLASS 沿用 dangerIcon 的「基础变体 + 修饰类」模式：`dangerText = "asb-btn-ghost asb-btn-danger-text"`。三张表（Codex 目录、Claude 目录、Codex 映射）的删除一律 dangerText。
- ghost 增加 `calc(space-unit + space-hairline)`（6px）横向内边距作为悬停呼吸；文字原位靠内边距实现，不用负外边距。折叠触发行是唯一的 flex-start 拉伸 ghost，由 `collapsible.css` 在触发器上 `padding-inline: 0` 保持贴边原几何，折叠行视觉零变化。
- `providerSetup.actions`（「操作」）是动作列表头的唯一键；`modelRoutes.edit` 删除，四个消费方（Codex 目录、Codex 映射、Claude 目录、路由面板）全部换用。
- `asb-provider-primary-pill`（provider-editor-sections.css 拥有）是「主模型」状态的唯一表达：Codex 动作列与 Claude 主模型列共用同一胶囊（行选中蓝底、行动蓝混色文字、全圆角），该行不再渲染禁用的「用作主模型」按钮，Codex 模型列下的小字「主模型」删除。主模型仍只有一个编辑位置。
- 运行参数行的模式 caption 只在显式模式下显示「自定义」；自动模式的来源事实由值本身（「客户端默认」）承载，DESIGN.md 运行参数行条款（名称、保存含义、当前值、来源、修改入口）不受影响。
- DESIGN.md `Button` 条款同步：dangerText 语义、ghost 呼吸、折叠行例外。

## 契约归属

变体几何与悬停面归 `buttons.css`，胶囊归 `provider-editor-sections.css`，折叠行复位归 `collapsible.css`，列头文案归 `provider-setup.ts`。页面只声明变体与键名，不自带任何样式；与折叠原语的边界见[全局折叠原语笔记](../architecture/2026-10-14-unified-collapsible-primitive.md)。

## Alternatives considered

- 方向 B（动作收敛：主模型独立徽章列 + 单一编辑入口 + 删除收进 ActionMenu）。它最强的理由是动作位从 3 个降到 1 个，删除天然与修改隔离，且 ActionMenu 契约在 DESIGN.md 已存在，无需新造组件。不用它：模型目录通常只有几行，「设为主模型」是高频动作，收进低频菜单每次多一击；表格多一列，窄容器更容易触发横向滚动；ActionMenu 的既有先例（扩展、会话、收藏）承载的都是低频动作，频率模型与这里不匹配。删除确认弹窗（A/B 皆可叠加）留待后续拍板：草稿态可重新添加，但连带清路由不可见——若将来加确认，归属切换事务侧而非表格。
- 负外边距实现 ghost 呼吸（内联文字绝对零位移）。它最强的理由是文字位置分毫不动。不用它：多个模块 CSS 以 `width: 100%` 拉伸 `.asb-btn` 容器，负外边距会让未来进入这些容器的任何 ghost 溢出父级，制造隐性布局陷阱；内边距方案只让文字右移 6px，肉眼不可辨，且只需在折叠触发行一处复位。

## Consequences

- 收益：删除的危险性从「猜」变成「看得见」（危险色 + 分隔 + 红染悬停）；列头语义正确且四表一致；主模型表达在两个编辑器收敛为一种；自动模式行少一行冗余文案；ghost 悬停不再贴字。
- 代价：ghost 文字全局右移 6px；删除仍然无确认——危险只是可见了，不是被拦截了；dangerText 的分隔线依赖 DOM 顺序（`:first-child`），调用方若把危险动作放在动作组首位，分隔会静默消失，这是有意的行为而非缺陷。
- 验证缺口：无既有测试断言这些文案或类名（rg 全量检索确认），本次不新增测试；样式效果待应用内目验。
