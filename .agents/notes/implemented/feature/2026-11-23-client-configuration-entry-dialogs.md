# Agent Note: 客户端配置四入口由互斥内联展开改为 AppDialog

Status: implemented

## Problem

客户端配置页头的「官方设置目录」「通用配置文件」「全局指令」「恢复为客户端原生默认值」是四个互斥的内联展开区：打开目录、通用配置或恢复时整张设置表单被 `hidden`，入口按钮在「展开 / 收起 / 返回 / 正在生成」之间切换文案，Escape 由面板自己接管并把焦点还给入口。官方设置目录在页面里以自动换行列铺开几十条路径，通用配置文件与三列差异表被压在内联盒层里，读长配置、核对较长的差异都要在设置表单的原位置上下翻找；每个展开区各带一层灰底盒层，套在页面模块卡里形成嵌套表面。四项都是「进入—查看或确认—离开」的独立任务，却和常驻的设置表单争夺同一个纵向滚动面。

## Decision

四个入口各打开一个 `AppDialog`，同一时刻只存在一个；`ClientSettingsPanel` 仍以单一 `openSection` 状态拥有「哪个弹窗开着」。

- 页面表单不再因弹窗打开而隐藏。入口按钮只保留名称（通用配置文件在读取期间显示「正在读取配置」并禁用）；`aria-expanded`、`aria-controls`、四个区域 id、面板自管的 Escape 与触发按钮 ref 全部删除，焦点恢复交给 AppDialog。
- 弹窗渲染在 `.asb-client-settings-panel` 之外、与 `ClientConfigurationConfirmation` 同级。`routeContentRef` 所指的正文容器在客户端切换期间带 `transform`，固定定位的弹窗只要落在带 transform 的祖先里就会被它收成包含块。
- `ClientSettingsPanel` 新增 `active` prop（页面可见且当前客户端路由匹配）。`AppWorkspace` 对访问过的页面只 `hidden` 不卸载，残留在隐藏页里的 ModalOverlay 会让页面之外的整个应用保持 inert，所以所属路由不可见时由 effect 关闭弹窗。
- 生命周期与忙碌不变：恢复弹窗每次打开都重新 `prepareNativeReset("nativeDefaults")`，确认写入永远基于当前文件的预览；弹窗 `busy` 是面板忙碌叠加 `configuration.applying`，通用配置文件弹窗再叠加模块内的手动写入。提交成功经既有 `onCommitted` 关闭弹窗，结果仍由页面状态行报告。
- 未保存关闭确认走共享 `useUnsavedChanges`。全局指令草稿在指令模型中跨弹窗存活，所以确认后的动作必须是「放弃草稿并关闭」，工作区为此向面板传入 `globalInstructionsDirty` 与 `onDiscardGlobalInstructions`。通用配置文件的手动编辑草稿是 `CurrentConfigurationEditor` 的局部状态，组件经 `onEditStateChange({ dirty, working })` 上报，弹窗据此锁定与守卫；写入路径的 `useCurrentConfigurationEdit` 本身不改。
- 标题只出现一次。目录弹窗复用入口键 `clientConfig.toolbar.directory`，组件内的 `<h3>` 删除；恢复预览的 `<h3>` 与区域 aria-label 上移为弹窗标题，并随 `resetView` 在「恢复为客户端原生默认值」与「清空 ASB 管理的额外通用配置」之间切换。
- 内容组件原样挂在弹窗主体里，不另造固定 footer：指令的动作行、通用配置文件的三态模块动作、恢复预览的确认行都紧跟各自的作用对象。宽度取 `wide`（目录、通用配置文件、恢复，分别承载多列条目、配置正文和三列差异）与标准档（全局指令）。
- 内联形态的盒层与变量随之下线：`.asb-client-configuration-review`、`.asb-client-settings-instructions`、`.asb-official-directory-title`，以及 `.asb-client-settings-panel` 作用域下的四处模块表面覆盖；`--asb-settings-module-bg` 失去全部消费者后一并删除（`-border` 仍被 Claude 模型偏好行使用）。词典删除 9 个只服务于展开 / 收起的键，两处「收起通用配置文件后」改为「关闭」。

折叠原语的边界不变：设置行自己的代码证据继续使用 Collapsible，四区块只是不再属于「工具栏驱动的内联展开」这一消费场景，见[全局折叠原语](../architecture/2026-10-14-unified-collapsible-primitive.md)。路由切换控制器与「配置组件隐藏但不卸载」见[客户端页签切换](../architecture/2026-10-16-client-route-transition-owner.md)。写入、备份与恢复语义未改，仍由[当前配置契约](2026-10-12-current-client-configuration-contract.md)和 DESIGN.md「写入与恢复」拥有。

同主题审计：上述三篇活跃笔记部分重叠，只在原篇更新「四区块 / 展开状态」的事实句，各自的决定不变；[双客户端当前配置目录](2026-11-04-current-client-settings-expansion.md)拥有目录字段与权限边界，不受影响；已归档的 `2026-10-16-client-configuration-tab-slide.md` 早已被路由控制器笔记吸收，不改。

## Alternatives considered

- 保留内联展开，只加宽、加高并去掉「隐藏表单」。它最强的理由是零新语义：草稿天然保留、没有模态阻断、用户可以一边看全局指令一边对照设置表单。不用的原因是这次要解决的正是形态本身——独立任务和常驻表单争同一个滚动面，「隐藏表单」是为了给展开区腾位置才存在的补丁；加宽加高后仍然是补丁。
- 把四个入口升级为二级页面（二级编辑骨架），而不是弹窗。它最强的理由是与「二级功能页」骨架一致，可以用整页宽度，不需要处理模态在隐藏页里的生命周期。不用：四者都是看一眼或确认一次就回来的短任务，二级页会替换整个内容流，返回成本高于模态；破坏性确认还需要一个与页面表单隔离的焦点圈定，这正是模态的职责。
- 每个弹窗用固定 footer 承载确认动作，即最标准的弹窗解剖。它最强的理由是长差异滚动时确认按钮始终可达。不用：通用配置文件是只读 → 编辑 → 预览的原地三态模块，恢复有生成、失败、额外配置清空三个没有确认动作的阶段，footer 必须逐阶段条件渲染，并把编辑状态整体上提到弹窗层；而写路径 hook 依赖已加载的 source，无法在读取期间就存在的弹窗层调用。内容组件不动、动作紧跟作用对象，已经满足「破坏性动作位于后果之后」。重访触发：恢复差异 routinely 超过一屏、用户反馈要滚到底才能确认时。
- 把 `useCurrentConfigurationEdit` 上提到弹窗层，而不是由组件上报 `{ dirty, working }`。它最强的理由是状态只有弹窗一个所有者，不需要同步 effect。不用：该 hook 在初始化、过期判断和重置 effect 五处直接读取 `source`，要容忍「尚未加载」就得改写真实客户端配置手动写入路径的状态机，而这条路径没有任何测试覆盖。代价是弹窗在编辑状态变化时多一次渲染和一个只在布尔值变化时才有实质效果的 effect。
- 关闭时静默保留草稿，沿用原「收起」语义。它最强的理由是零丢失，也不新增一次确认。不用：模态的关闭是明确的「离开」，页面上没有任何可见的待保存提示，静默保留会让重开弹窗时出现用户以为已经丢弃的文本；应用其余编辑弹窗统一走 `useUnsavedChanges`，这里不该有第二种关闭语义。

## Consequences

- 目录弹窗沿用窗口高度上限，筛选结果增减时搜索框留在原位。通用配置弹窗同时接收手动编辑、语法修复和 Claude 额外配置编辑的忙碌状态，需要放弃未应用的文本时统一确认。

- 收益：独立任务与常驻表单分离，打开目录、通用配置或恢复不再把设置表单藏起来；入口文案稳定；目录条目、配置正文和差异表拿到 `wide` 档弹窗宽度；少四层盒层和一个变量；面板不再自管 Escape 与焦点恢复。
- 代价：`ClientSettingsPanel` 多一个 `active` prop，工作区多两个指令草稿 prop，`CurrentConfigurationEditor` 多一个上报 prop 和一个 effect；弹窗打开期间页面被模态阻断，不能像原先展开全局指令那样同时对照设置表单；带未保存草稿关闭两个编辑弹窗时会多一次确认。
- 行为变化只有一处超出「换个容器」：关闭带未保存草稿的弹窗从「静默保留」变为「确认后放弃」。
- 边界：弹窗的关闭跟随路由可见性而不是页面返回请求；模态打开时侧栏 inert，页面的返回根视图请求无法触发，因此不向 PageRoot 登记关闭成员。
- 无既有测试覆盖这四个入口（检索 `tests/` 与 `scripts/` 确认），本次不新增测试。按项目约束未运行类型检查、构建或自动化测试。

## Verification

- 在完整应用入口使用模拟 IPC 打开四个弹窗。目录筛选前后的搜索框位置一致，键盘关闭后焦点回到入口。Claude 额外配置的关闭确认保留或放弃编辑文本，全局指令弃稿后重开可见原内容，恢复弹窗显示模拟差异预览。
- 实际配置写入和原生 WebView 的焦点行为仍缺实机验证。
