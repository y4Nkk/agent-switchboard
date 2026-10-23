# Agent Note: 全局折叠原语 Collapsible 统一所有展开收起交互

Status: implemented

## Problem

折叠交互在 15+ 处各自为政：`RowPanelDisclosure`（行面板）、`ClientSettingsPanel` 私有 `ClientSettingsDisclosure`、编辑器两处原生 `<details>` 折叠、客户端配置代码证据的 `<details>`、扩展能力/安装目标的 `<details>`、MCP 元数据 `<details>`、更新日志 `<details>`、四处完全无类名的裸 `<details>`（候选文件、每日数据、上游请求体、扩展资源事实），以及多处无动画条件渲染（官方行重登录/额度、发现警告列表、批量标签面板）。样式层面存在两套 grid 动画语法（`details` 的 `auto 0fr→1fr` 与 div 的 `0fr→1fr`）、两套 CSS 伪元素箭头（`›` 旋转 90°）、Lucide 图标对切换（上/下箭头互换）三套视觉语法，散落在 7 个 CSS 文件；行为层面动画与否、收起后挂载与否、aria 接线方式各不相同。

## Decision

`src/components/Collapsible.tsx` 是软件内全部展开收起的唯一原语，`styles/base/collapsible.css` 是折叠语法的唯一拥有者（高度、位移、透明度、visibility 时序、箭头旋转、reduced-motion 覆盖全部只在此一处）：

- `Collapsible`：受控动画容器，供触发器在别处（行动作按钮、工具栏入口、横幅）的场合使用。挂载契约全局唯一——内容打开时挂载、保持挂载至收起动画结束、随后卸载（`transitionend` + 时长感知定时器双兜底，减少动态效果下立即卸载）。收起后无隐藏 DOM、无残留布局。`onClosed` 在卸载时刻触发一次，供"预览与完整阅读器互斥"的调用方（会话长消息）在收起完成后让截断预览回归。
- `CollapsibleSection`：自带摘要行的折叠区块，取代一切 `<details>`。受控 API（`open` + `onOpenChange`，初始状态由调用方 `useState` 携带，如"已有模型映射默认展开"）；`label` + 可选 `value`（尾随弱化值）+ 唯一旋转箭头。摘要行是无按钮外观的纯文本行——`unstyled` 是零样式标记，行自带完整外观重置（漏掉 `border:0; background:transparent` 会让浏览器 UA 按钮底 chrome 漏出来，渲染成假输入框）。两种几何对应两种既有版式：标准色调为整行折叠行、箭头钉行尾（编辑器折叠模块、扩展能力盒）；quiet 色调为次要证据的紧凑文本行、行宽贴合并箭头紧随标签（代码证据、更新日志、资源事实）。箭头缩进归摘要行上下文所有（`.asb-collapsible-summary >`），不写在箭头本体上——否则横幅等外部触发器里的文字会被 auto 边距推离原位。
- `CollapsibleChevron`：唯一箭头语法（单个 ChevronDown 旋转 180°），取代上/下箭头互换与 CSS 伪元素箭头。

触发器契约：按钮声明 `aria-expanded`，仅在请求展开时声明 `aria-controls`，不引用已卸载的目标。`Collapsible` 从请求收起起立即设置 `inert`，保留挂载的退出动画不再使内容留在键盘导航和辅助技术中；显隐、卸载与可交互状态统一由原语拥有，页面不补第二套禁用逻辑。工具栏驱动的四区块经 `ariaLabel` prop 升级为带标签的 `role="region"`。

已接入：供应商行测试/用量面板（Claude 与 Codex）、两行官方面板（重登录、订阅额度，此前为瞬切条件渲染）、供应商编辑器模型映射（Claude/Codex）与连接测试、客户端配置四区块与逐项代码证据、扩展能力表/安装目标/资源事实、MCP 元数据、发现警告列表、更新日志、候选文件预览、Codex 每日数据表、上游请求体、会话批量标签面板、[会话详情信息](../bug-fix/2026-09-24-session-operations-layout.md)、会话长消息"展开完整内容"（截断预览与分页阅读器经 `onClosed` 互斥换位：展开时预览立即让位、阅读器展开动画；收起时预览在动画结束、阅读器卸载后才回归，两形态绝不并存；展开集合仍由 `useSessionDetail` 持有，收藏页恒展开与目录跳转展开走同一条路）。已删除：`RowPanelDisclosure.tsx` 与 7 个 CSS 文件中的全部专属折叠块（行面板、编辑器 details、连接测试、客户端配置、扩展、MCP、更新日志、代码证据）；provider-editor 折叠引用与 `provider-editor-disclosures.css` 只保留非折叠布局。`DESIGN.md` 组件所有权节新增 Collapsible 条目。

**范围外（非容器折叠，不接入）**：侧栏展开收起（外框 grid 几何，`shell.css` 拥有）、气泡弹层与下拉（GlobalInformation、FontPicker、ModelPicker、Select、Tooltip）、模态弹窗、页内视图/路由切换（运行参数页、发现面板、仓库管理等）、代码预览的高度档位切换（同一滚动元素 16rem↔65dvh 的上限调整，元素从不卸载，不是显隐折叠）——这些是与容器折叠不同的交互家族，强套折叠原语会破坏其交互契约。

同主题笔记审计：`implemented/simplification/2026-10-13-provider-advanced-settings-always-visible.md` 相邻（其"高级设置不重新引入折叠"的边界被本决策保持，互不覆盖）；`2026-10-13-ui-copy-density-reduction` 与 `2026-10-13-linuxdo-pelican-animation` 中被否/声明的"不做折叠"决定不受影响。无旧笔记持有折叠语法决策。

## Alternatives considered

- 统一保留原生 `<details>` 加公共 class：最强理由是零 JS 获得语义、键盘与渐进增强，`details` 收起时内容仍留在 DOM，动画也只需 CSS。否决：无法表达"挂载至收起动画结束即卸载"的全局契约（`details` 内容永远在 DOM 里，行面板的轮询与请求无法随收起停止）；现有代码为控制它已出现 `open={expanded}` + `onToggle` 回写的受控混合反模式；触发器在远处的行面板结构上无法用 `details` 表达。
- `Collapsible` 增加 `keepMounted` 双模式：最强理由是零迁移成本地兼容客户端配置四区块"收起仍挂载"的旧行为。否决：逐项审查后确认四区块内容全部受控或无状态，keep-mounted 不是承重行为；双模式是投机配置面，单一卸载契约才可审计。
- 渐进迁移：新组件与旧 class 并存、按屏逐步切换。否决：双语法并存本身就是本次要清除的技术债；用户明确要求无兼容、无过渡、一次性完成。
- 会话长消息保持瞬时换位、不进折叠原语：最强理由是预览与阅读器是同一文本的两种形态，折叠会带来约 400ms 的双文本或空窗。否决方式不是硬套，而是在原语上补 `onClosed`（卸载时刻事件）并让预览在收起完成后才回归：展开动画保留、互斥严格成立，一行回调消除了"容量切换必须游离在原语外"的借口。

## Consequences

- 收起即卸载成为全局契约，两处行为随之收敛：连接测试面板收起后丢弃上次测试结果（此前收起仍保留）；客户端配置四区块重开时重挂载（内容受控、无数据损失）。行测试/用量面板行为不变（原本就是该契约）。
- 视觉与动效统一：扩展能力/安装目标、MCP 元数据、更新日志、资源事实等从此前无动画的 `<details>` 升级为同一组高度/位移/透明度过渡；Codex 官方行重登录与额度面板获得折叠动画；全部箭头统一为单个旋转 chevron。
- 代价：`CollapsibleSection` 的 8 个原本无状态调用点各自新增一行 `useState`，这是受控单契约的固定成本。
- 新增折叠需求必须经 Collapsible 族实现；修改折叠动画、箭头或 reduced-motion 只改 `collapsible.css`。不得再引入 `<details>`、CSS 伪元素箭头或屏幕私有折叠容器。

## Testing

- `npm run verify-notes` 通过（笔记结构门禁）。
- 按工作区规则未运行 typecheck/test（用户未要求）；以逐文件人工核对（导入、JSX 结构、aria 接线）加全库 grep 验证：旧类名（`asb-row-panel-disclosure`、`asb-provider-disclosure`、`asb-provider-test-disclosure`、`asb-client-settings-disclosure`、`asb-ext-disclosure`、`asb-mcp-metadata`、`asb-update-changelog`、`asb-client-setting-code`）与 `<details>`/`<summary>` 在 `src/` 零残留，`RowPanelDisclosure` 无引用；会话消息仅 `SessionTranscript` 与 `SessionBookmarksPane` 两个消费点，后者 `expanded` 恒真、无切换按钮，走同一条 Collapsible 路径。
