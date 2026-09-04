# Agent Switchboard — 仓库指引

本文件适用于整个仓库。更深层目录中的 `AGENTS.md` 仅在其自身子树内覆盖本文件。

## 1. 产品边界

`Agent Switchboard` 是一个仅服务于 `Codex` 和 `Claude Code` 的本地配置控制台（`Windows` / `macOS` / `Linux`）。
它的职责是让提供商配置可观察、可安全切换。

- 除非用户明确要求，否则不得增加其他编程客户端支持。
- 默认不得构建云同步、遥测、账户系统、`LLM` 代理或提供商自动故障转移。
- `CC Switch` 是提供商配置档、通用配置叠加、客户端专属适配器和可逆激活的后端行为参考。必须通过本项目自身的契约与测试重新实现这些理念；不得复制其模块、界面、品牌、资源或文字。
- 除非用户指定其他分支，`Codex++` 指上游 `BigPizzaV3/CodexPlusPlus` 项目。它是 `AGPL-3.0-only` 的后端安全参考，涵盖管理器、核心与数据分层、可观察锁、备份和诊断。未经用户明确决定将本项目采用 `AGPL` 许可证，不得复制或改编其源码、界面资源或安装脚本。
- 界面必须专为 `Agent Switchboard` 设计和实现。不得复用任一参考项目的布局、组件、图标、视觉令牌、交互模式或微文案。
- 不得引入 `Chromium DevTools Protocol` 注入、`app.asar` 补丁、安装程序补丁、渲染器注入脚本，或自动改写 `Codex` 会话 / `SQLite`。这些均超出本产品安全配置管理器的边界。
- 面向用户的回复和文档默认使用中文。标识符、命令、路径和 API 名称保持不变。

## 2. 阅读顺序与文档职责

进行非简单改动前，按以下顺序阅读相关文档：

1. `AGENTS.md` — 开发与安全规则。
2. `README.md` — 产品范围和计划架构。
3. `DESIGN.md` — 界面与交互契约。
4. `progress.md` — 当前实施状态。

文档职责不得重叠：

| 文档 | 职责 |
| --- | --- |
| `README.md` | 产品入口、范围、架构及后续搭建说明 |
| `DESIGN.md` | 视觉意图、界面令牌、布局与交互规则 |
| `progress.md` | 阶段目标、状态、验收与退出条件的唯一来源 |
| `AGENTS.md` | 人类与 AI 贡献者的工作规则 |

## 3. 核心工作规则

- 安全 > 正确性 > 透明度 > 简洁性 > 一致性。
- 只实现明确提出的请求。不得增加推测性功能或无关清理。
- 改动前检查当前实现。先识别契约所有者和受影响文件。
- 使用 `rg` 或 `rg --files` 搜索仓库；在 Windows 上使用 PowerShell。
- 保持改动小且能追溯到请求。
- 在脏工作树中保留无关的用户改动。
- 除非用户明确要求，否则不得提交、推送、创建拉取请求或发布版本。
- 完成声明必须有实际验证证据支撑。

## 4. 配置安全契约

应用最终将读取和写入用户拥有的 `Codex` 与 `Claude Code` 配置。因此，安全是产品要求，而非可选功能。

- 切换执行器是唯一允许写入真实配置文件的层。界面代码必须请求带类型的预览或切换操作；绝不能直接构造文件文本。
- 规划不得产生副作用：客户端适配器负责解析、合并和渲染；仅切换执行器协调锁、备份、校验、提交、恢复和结构化诊断。
- 将未识别的键视为宿主所有。在目标格式允许时，必须逐字节保留它们。
- 提供商叠加层仅拥有其声明的字段。不得为变更一个提供商而替换整个 `TOML` 或 `JSON` 文档。
- 开发或测试期间，未经用户直接确认，绝不能修改真实用户配置、凭据缓存或环境变量。
- 所有涉及文件写入的测试必须使用隔离的临时目录。
- 每条写入路径必须依次执行：加锁、解析、冲突检查、预览、备份、临时写入、校验、原子替换与写后验证。
- 锁是可观察状态。必须区分空闲锁、活动持有者、陈旧持有者和不确定锁；不得静默删除可能属于活动进程的锁。
- 写入失败时必须恢复紧邻的前一份备份，并清晰报告错误。

## 5. 密钥与数据

- 不得将 API 密钥、令牌、凭据或私有 URL 写入源码、测试数据、快照、日志、`README.md` 或截图。
- 应用数据目录的 `state/configuration/providers/{codex,claude}/{id}.json` 保存供应商 API 密钥、名称、模型和服务地址；活动 Codex 自定义档案另以官方 API-key 缓存形态写入用户的 `auth.json`。密钥只允许经编辑器输入或本机 Codex / Claude / CC Switch 配置导入。
- Codex 自定义切换固定写入 `model_provider = "openai"` 和顶层 `openai_base_url`，并在同一执行器事务内将档案 `apiKey` 写入 `auth.json` 的 `auth_mode = "apikey"` / `OPENAI_API_KEY`；不得写入顶层或 `model_providers.*` 的 bearer token。官方 Codex 档案移除 `openai_base_url`，恢复 `auth_mode = "chatgpt"` 并保留已有 OAuth `tokens`。Claude 切换将密钥写入受管 `env.ANTHROPIC_AUTH_TOKEN`。预览、差异、日志、错误、诊断和截图只可出现稳定脱敏标记。
- 官方 Codex / Claude 登录流程只在用户明确发起登录或重新登录时写 OAuth 凭据；Codex 自定义切换仅更新 `auth.json` 的 API-key 登录字段，不复制、展示或记录 OAuth 令牌。登录令牌不进入供应商档案、渲染层、日志或错误信息。
- 渲染差异、错误、审计记录或诊断前必须脱敏密钥。

## 6. 契约、代码与测试

- 每个数据契约只有一个带类型的所有者。界面、持久化和运行时应消费它，而非各自重建字段列表。
- 同一改动中更新契约所有者、校验器、运行时、直接消费者、测试数据和测试。
- 除非经用户批准的迁移要求，否则不得保留旧配置字段、兼容分支或回退解析。
- 尽早拒绝无效配置，并说明修复方法。
- 优先使用不可变值、显式枚举、窄函数以及注入的文件系统依赖。
- 不得通过自动更换提供商或重试循环隐藏错误。
- 实现存在时，运行最小相关检查：用于合并 / 写入逻辑的 Rust 单元测试、用于界面契约的 TypeScript 类型检查，以及用于切换的隔离临时文件集成测试。

### 规模与拆分

- 行数是发现职责混杂的审查线，不是机械拆分目标。先按一个明确职责和一个契约所有者组织文件；不得只为满足阈值拆出名称空泛、仅调用一次的辅助函数。
- 运行时源文件以 `500` 个非空、非注释行为审查线。超过 `800` 行后，不得继续加入独立职责；若本轮改动已触及可独立命名的职责边界，必须同轮拆分，并更新导入、测试与运行入口。
- 运行时函数以 `80` 个非空、非注释行为审查线。超过 `120` 行时，除连续事务、完整解析过程或不可拆的显式状态机外，必须按有业务名称的步骤拆分；“函数很复杂”不是例外理由。
- 测试文件不按上述阈值机械拆分，但每个测试文件或测试组必须只验证一个契约；测试夹具由被测契约拥有，不得在多个用例间复制维护。
- 已存在的超线文件不授权无关重构，也不能成为继续混入第二职责的理由。修改这类文件时，保持改动范围与现有所有者一致；需要收敛时，在同一职责边界内完成，不保留转发层或双实现。

## 7. 前端规则

- 创建或重设界面样式前先阅读 `DESIGN.md`。
- `DESIGN.md` 拥有视觉意图；未来的令牌样式表拥有运行时 CSS 值。不得在组件中散布原始十六进制值、模糊半径或动画时长。
- 视觉语言为 `Frosted Relay`：克制的 Apple 式层级、审慎的磨砂玻璃和一个双客户端路由控件。
- 装饰性背景必须使用真实 DOM 层。不得用伪元素伪造可见控件或产品内容。
- 保留可见的键盘焦点、至少 40px 的触控目标以及减少动态效果的行为。
- 不得在模糊玻璃内再放模糊玻璃。配置文字与差异必须保持高对比度且接近实心。
- 不得用眉题、标签堆、通用副标题或解释性微文案装饰界面。每一处文字都必须说明真实的设置、操作、状态、警告或决策；若移除后界面并未更难理解，就应删除。

## 8. 文档与交付

- 不得将会话记忆、代理持久化记忆或本机工具状态提交到仓库。
- `progress.md` 是项目唯一的目标文档。阶段启动、完成、范围变化或受阻时更新它。
- 修改文档时，验证所有链接、提及的路径、命令和唯一事实来源声明。
- 最终报告应说明：改动、验证和剩余风险。

<!-- boardui:rules:start -->
# BoardUI design rules

This project uses BoardUI (React + Tailwind CSS v4, source-owned components under `components/`). These rules always apply when writing UI code. MCP tools are on demand; these rules are not optional context.

## Components first

- Before hand-building any UI element, check for an installed BoardUI component under `components/base/` and `components/application/`, and prefer it.
- Missing a component? Install it (BoardUI MCP `install_components`, or `npx boardui@latest add <name>`) instead of writing a lookalike.
- Import through the `@/` alias, e.g. `import { Button } from "@/components/base/buttons/button"`.

## Color: semantic tokens only

- Never use raw palette classes (`text-gray-500`, `bg-white`, `border-neutral-200`) or hex/oklch literals. Every color rides a BoardUI semantic token, which also makes dark mode automatic.
- Text: `text-text-primary`, `text-text-secondary`, `text-text-tertiary`, `text-text-placeholder`, errors `text-text-error-primary`.
- Surfaces: `bg-background-primary-default`, `bg-background-secondary-default` / `-hover`, `bg-background-tertiary-default`, page ground `bg-background-full`.
- Borders: `border-border-button-default` / `-hover`, hairlines `border-separator-border`, tables `border-border-table`, errors `border-border-error-default`.
- Icons: `text-foreground-icon-primary` through `text-foreground-icon-quaternary`.
- Charts: the `chart-1` … `chart-5` tokens (plus `-active` variants). CTAs and selection states: the `accent-50` … `accent-950` ramp.
- Dark mode flips tokens via the `.dark` class on `<html>`. Do not write `dark:` overrides with raw colors; if a token pair looks wrong in dark mode, pick a different token, not a literal.

## Typography: composite utilities only

- Use BoardUI's composite type utilities: `text-title-1-medium`, `text-title-2-medium`, `text-title-3-semibold`, `text-headline-medium`, `text-body-medium`, `text-body-regular`, `text-body-2-*`, `text-caption-1-semibold`, and friends. Each sets size, weight, line-height, and letter-spacing together.
- Never rebuild type by stacking `text-sm font-medium leading-5`; if a style seems missing, look in `styles/typography.css` before inventing one.

## Spacing and shape

- Stay on Tailwind's spacing scale (`gap-2`, `p-4`, `mt-6`); prefer flex/grid `gap` over per-child margins. Arbitrary values (`p-[13px]`) only when matching an existing BoardUI component exactly.
- Cards and panels: `rounded-3xl` with `border-border-button-default`. Inputs and menu rows: `rounded-md` to `rounded-xl`. Pills: `rounded-full`.

## Mechanics

- Merge classes with `cx()` from `@/utils/cx` (tailwind-merge aware of BoardUI's composite text styles). No string concatenation, no plain `clsx`.
- Icons come from `@remixicon/react`, passed as component references (`leadingIcon={RiAddLine}`), not rendered elements.
- Form components build on `react-aria-components`; extend the installed BoardUI form components rather than raw `<input>`/`<select>`.
- Focus states: `outline-none focus-visible:ring-2 focus-visible:ring-border-focus-ring`.

When unsure about a token, a component's API, or working example code, ask the BoardUI MCP server: `get_theme`, `get_component`, `get_usage_examples`.
<!-- boardui:rules:end -->
