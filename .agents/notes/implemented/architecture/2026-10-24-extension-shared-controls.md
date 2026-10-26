# Agent Note: 扩展工作区的共享控件与唯一外观归属

Status: implemented

## Problem

扩展页虽然引用 Button，仍用 unstyled 自绘客户端启停、MCP 客户端选择、搜索清除、条目入口和批量操作；扫描警告按钮只有文字颜色，原生按钮外观会漏出。部分图标操作在页面重复声明 32px 尺寸与图标大小。搜索和菜单也由模块复制结构与样式，其中扩展、会话和收藏的更多菜单直接绕过全局 Button。

这些局部声明既重复事实，也使 mixed、disabled、busy、键盘焦点和 aria-controls 的正确性依赖每个调用点自行拼装。问题不涉及扩展导入、部署或 MCP 检测业务契约。

## Decision

- 客户端行启停与 MCP 客户端选择直接消费全局 Checkbox，真实原生 input 拥有键盘、禁用、选中和 mixed 状态。Checkbox 增加当前消费者需要的 busy 语义并同步原生 indeterminate 属性；Tooltip 对 disabled 复合控件遵守相同命中契约。
- 批量启停保持命令按钮，继续明确作用于当前分类的全部条目，不把汇总状态伪装成另一份选择状态。条目与警告动作使用 Button.ghost，危险图标操作与 compact 尺寸由 Button 拥有，页面仅保留布局与业务文字。
- `src/components/SearchInput.tsx` 与 `styles/base/search-input.css` 是搜索图标、输入内留白、清除按钮、Escape 清空和焦点归还的唯一组合，内部复用 Input 和 Button；扩展库、来源筛选与目录查询直接使用它。
- `src/components/ActionMenu.tsx` 与 `styles/base/action-menu.css` 是更多菜单的唯一结构和样式，使用已有 react-aria-components 的 MenuTrigger / Pressable / Menu / Popover 行为并组合真实 Button。一次替换扩展、会话和收藏的三个消费者，删除两套旧 CSS；不新增依赖、不手写菜单键盘状态机。
- 菜单项的鼠标悬停与键盘焦点分别消费 react-aria 的 data-hovered / data-focused 状态和全局行背景变量；禁用项不响应悬停高亮。不能用焦点状态推断指针悬停，因为库分别发布这两种输入状态。
- 扫描警告复用 CollapsibleTrigger 的展开语义与按钮，移除局部摘要按钮、失效目标引用和无消费者文案。业务选择与异步写入仍由原有 hooks 和类型化入口拥有。
- 删除没有源码消费者的旧来源浏览器、环境行和 MCP 固定操作栏样式。Table 行点击的指针规则移回 tables-backups.css，扩展样式不再持有全局表格规则；清除搜索的文案收敛到 common.clearSearch，不保留旧扩展键。

## Related decisions

[共享折叠原语](../../implemented/architecture/2026-10-14-unified-collapsible-primitive.md)拥有挂载、动画和折叠触发语义，本篇仅新增扩展消费者。[扩展范围与焦点](../../implemented/bug-fix/2026-09-24-extension-visibility-and-focus.md)继续拥有批量作用范围与键盘可见性。[桌面工作区](../../implemented/architecture/2026-10-13-desktop-workspace-uiux.md)继续拥有页面层级与菜单中的业务动作。[Tooltip 命中契约](../../implemented/bug-fix/2026-10-14-tooltip-dismissal-anchor-contract.md)继续拥有禁用控件的悬浮说明。检索未发现 SearchInput 或 ActionMenu 的现有实现或独立决策，无需保留平行所有者或归档其他篇。

## Alternatives considered

- 为每处 unstyled 补齐 reset：改动少且不改变控件结构；但继续复制尺寸、状态与交互，不能消除同类遗漏，因此改为直接组合共享原语。
- 全局改变 unstyled 的视觉语义：一次覆盖所有遗漏；但它还用于导航、弹层和卡片入口，强加按钮外观会改变无关控件。本次为当前真实用途选择明确变体并删除局部实现，不更改 unstyled 契约。
- 只抽出扩展菜单：影响最小；但会留下扩展与会话两套相同菜单事实，因此同步替换三个直接消费者。菜单的业务回调、权限和确认仍留在所属工作区。

## Verification

- 源码核对确认扩展界面 7 处 unstyled 用法清零，无自造 checkbox 角色、原生字段和模块自绘菜单；已有输入、选择、分页、表格与对话框继续直接消费全局组件。
- 紧凑按钮尺寸、危险状态、搜索清除及警告折叠只有共享实现。退役类名、无消费者文案与样式无残留，没有兼容别名或过渡开关；五个扩展样式文件逐类核对源码消费者。
- 使用本机 rolldown 的纯解析 API 和 PostCSS 检查 24 个改动 TS/TSX 模块与 16 个样式文件，语法、相对导入路径、无引用导入、源文件 500 行与函数 80 行检查通过；不执行转换、构建或用户代码。`git diff --check` 通过；`npm run verify-notes` 的 65 篇结构与链接、8 篇归档封印通过。
- 用户授权后，在独立临时 HOME、USERPROFILE、APPDATA、LOCALAPPDATA、CODEX_HOME、CLAUDE_CONFIG_DIR、XDG 和临时目录内运行验收；固定 Node 22.23.2 的 44 项前端回归、TypeScript 检查及生产构建通过，Windows 原生应用的 dev 编译与链接通过。Rust 全工作区 743 项通过；两个忽略项是所属测试显式启动的子进程入口，其拥有者测试通过。
- 测试用 Chromium 加载真实生产组件和样式，业务数据与回调使用隔离夹具。51 组浏览器检查通过，覆盖深浅主题、减少动态效果、1440×900 与 1080×640 视口、侧栏展开后等效 820×536 内容区、英语清空文案、搜索 Escape 与焦点归还、菜单鼠标与键盘操作、禁用项、mixed 与忙状态、MCP 选择、全分类批量操作、资源分页和警告折叠。渲染无异常、无外部请求，并检查实际截图。
- 悬停回归在修复前直接证明两种主题的菜单项背景仍透明：react-aria 发布 data-hovered，旧样式只监听 data-focused。共享样式补齐悬停行背景后，同一视觉断言与禁用项、键盘焦点检查通过，未新增页面补丁或改变菜单状态所有者。
- 本机已安装的 react-aria-components 1.21.1 MenuTrigger 将触发属性和 ref 交给 PressResponder，Pressable 经 usePress 合并这些属性并向子元素传递合并后的 ref；共享 Button 已 forwardRef，因此沿用现有键盘、ARIA、定位和关闭行为，不另造菜单控制器。
- 审查同时参照 [Web Interface Guidelines](https://raw.githubusercontent.com/vercel-labs/web-interface-guidelines/main/command.md) 的原生控件、标签、可见焦点、复选框完整命中和悬停反馈要求；项目的既有状态、业务操作与动效契约仍优先。

## Consequences

共享菜单、复选框、搜索和图标动作的视觉与语义同时收敛。客户端列为标准复选框预留相应宽度，仍由 Table 拥有列表滚动；MCP 客户端选择不再保留自绘品牌按钮。菜单项的禁用、批量作用范围、删除确认、导入部署和连接检测的业务回调均沿用原入口。来源查询清空只调用原有输入更新入口，不自动提交查询或写入。

代价是三个菜单和三个搜索面共同依赖共享组件；修改这些组件必须同时关注现有消费者。浏览器证据覆盖生产组件与真实样式，配置写入回归只使用隔离临时目录；它不等同于安装器、原生 WebView2 壳层交互或真实供应商联调。没有启动真实用户桌面实例或写入真实客户端配置。
