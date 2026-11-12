# Agent Note: 应用内更新日志的 Markdown 渲染与当前版本段落

Status: implemented

## Problem

更新模块只在发现新版本时显示新版本的更新说明，用户看不到当前运行版本包含哪些变更；且说明中的行内 Markdown（代码、链接等）不曾渲染。仓库根的 CHANGELOG.md 已自声明为"GitHub Release 与应用内更新说明的唯一来源"，但应用内没有读取它的通路。

## Decision

Markdown 正文继续作为格式契约，应用不拆解正文的类别、列表和行内元素。CHANGELOG.md 经 `tauri.conf.json` 资源映射随应用打包；[关于页历史日志](2026-11-12-about-changelog-history.md)接管版本边界与元数据，取代原单版本读取、当前版本披露及失败时静默隐藏的决定。正文渲染仍归前端共享查看器 `MarkdownContent`（react-markdown + remark-gfm，会话记录与更新日志唯一共用的渲染管线）：原生 HTML 不渲染，http(s) 链接经 opener 打开系统浏览器，非 http 文本不渲染为链接。历史日志与新版本 release body 共用 `ReleaseNotes`，原文进入同一个查看器；元素间距由共享 `.asb-markdown` 拥有，`.asb-update-notes` 与 `.asb-session-message-body` 只保留各自布局。设置行（`.asb-app-setting-row`）统一顶端对齐：动作按钮钉在文案首行旁，正文增长不会把按钮拖进中间，不设按行特例。

## Related decisions

- [标题栏信息](../feature/2026-10-12-titlebar-information.md)拥有全局反馈位置，与本篇更新模块内容无冲突。
- [关于页历史日志](2026-11-12-about-changelog-history.md)部分取代当前版本读取；本篇继续拥有唯一随包文档、Markdown 查看器与链接语义。
- 壳层笔记（`2026-10-14-unified-native-glass-shell.md`）拥有侧边栏更新入口的位置与状态，本篇只拥有设置页更新模块的日志展示。

## Alternatives considered

- 自定义结构契约（初版方案：后端解析 `### `/`- ` 返回 `{title, items}`，前端纯渲染结构）：前端无法渲染任意 Markdown，行内语法只能按契约逐个加点；为了"完整 Markdown"再养一套自定义格式，等于重复 markdown 库的职责。
- 后端 pulldown-cmark 渲染成 HTML 下发：多一条 HTML 消毒链路与注入面，Rust 侧重复前端已有的渲染能力。
- 前端直接请求 GitHub releases API 按标签取正文：每次打开设置页都要联网，离线不可用，且与 updater manifest 形成重复通路。
- 两个消费面各写一份 `Markdown remarkPlugins` 接线和链接拦截（中间态）：链接打开行为成为两处契约；已提取为 `MarkdownContent` 单一所有者，会话记录同步迁移。
- 把 CHANGELOG.md 编译进二进制常量：多一条构建生成链路；资源文件随包分发已足够。

## Consequences

- 收益：Markdown 元素格式归 remark 库，应用只拥有版本边界与元数据；会话记录与更新日志共用同一查看器、同一链接行为、同一元素样式，当前、历史与新版本正文共享渲染路径。资源读取不依赖联网。
- 代价：react-markdown 进入主包；`## [version]` 边界仍由 changelog.rs 拥有，改标题格式必须同步；链接打开沿用共享查看器语义。缺失资源现在明确报错并可重试，不能继续当作没有日志。
- 历史披露默认收起并使用共享 Collapsible，打开后才加载，关闭后释放；不增加更新模块的默认正文高度。

## Verification

既有资源映射、lib.rs 模块声明和共享 Markdown 渲染链继续成立；单版本解析用例随旧命令删除，新历史边界和验证范围见[关于页历史日志](2026-11-12-about-changelog-history.md)。正文旧结构契约与样式迁移当时仅按源码核对，不作为新历史模块的运行验收；类型检查、编译与自动化测试仍遵守项目授权规则。
