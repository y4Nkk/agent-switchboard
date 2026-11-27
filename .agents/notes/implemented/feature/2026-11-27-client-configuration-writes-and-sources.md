# Agent Note: 客户端配置的按项写入、来源检查与恢复

Status: implemented

## Problem

配置表单提交整份已保存意图，未编辑的自动项也会移除用户文件中的内容。供应商切换会重放这些移除，原文编辑也会连带提交表单。读取不到设置时缺省值会被显示为自动。高级恢复一次移除全部符合条件的界面外字段。Codex 原生权限档案只有状态提示，Claude Code 工作目录外的文件工具边界缺少控件。

## Decision

表单以最近读取的用户文件为基准，提交变更项。省略字段表示沿用，显式移除删除用户级值及 ASB 共用意图，「沿用文件值并解除管理」只移除已保存的共用意图。供应商切换只投影明确保存的共用值，未声明字段与自动项均由原生文件提供。原文编辑独立提交脱敏复原后的差异。确认同时绑定用户文件与 ASB 已保存值的差异，使用既有备份和恢复事务。生成预览及写入期间全部表单控件共享禁用状态，确认失败直接在弹窗中显示，保留待确认的差异。

标准恢复保留原子删除范围，可从符合归属边界的候选中逐项添加界面外字段。选择标识由实际键段编码，同一选择的预览及确认都从当前文件核验，保护配置、凭据和扩展由各自模块拥有。

Codex 来源检查复用已发现的本机 CLI，通过 App Server 读取合并配置、来源、组织要求和 Windows 沙箱就绪状态。用户级档案选择引用可识别的全局档案，在预览和确认时核验策略、运行版本和旧沙箱冲突。已由 Codex 通用片段声明的档案保持单一归属。Claude 读取所选工作目录与本机可访问的设置文件，实际会话由 `/status` 核对加载来源。两项工作目录外的文件工具限制来自同一目录和版本校验。

## Related decisions

[配置契约](2026-10-12-current-client-configuration-contract.md)和[结构化权限](2026-11-04-current-client-settings-expansion.md)持有类型目录和模型参数边界。本篇接管配置表单的投影、恢复范围和用户级权限选择。[配置入口弹窗](2026-11-23-client-configuration-entry-dialogs.md)持有入口与关闭交互。[软件版本](2026-11-04-client-software-version-management.md)持有安装发现与更新策略。[实际配置状态](../bug-fix/2026-10-16-observed-configuration-status.md)提供用户文件的观察状态。

## Alternatives considered

- 保留全量投影，在最终差异上给连带删除增加提示。它能保留整份意图的重放能力，代价是每次小改仍需审核无关项，也不能保护切换入口中的未接管值。
- 由 ASB 解释两端的所有生效规则。它可以提供统一字段视图，但客户端的项目可信状态、组织策略、启动参数和会话覆盖无法从静态文件推导。Codex 采用官方合并结果，Claude 可读文件只提供来源证据。
- 继续在原文里切换 Codex 档案。它能编辑全部规则，但不能在控件中使用官方名单或及早发现冲突。选择器只声明要写入的档案名。

## Consequences

单项提交和档案切换避免重复移除用户级设置。稀疏共用意图允许备份使用创建时已有的字段，恢复不为后来添加的配置推断意图。高级恢复多一次逐项选择，换取精确的破坏范围。Codex 核验依赖本机 CLI 的 App Server 版本和响应预算。Claude 的组织配置、进程启动参数及已运行会话需要原生确认。

## Verification

回归用例覆盖普通改动保留拒绝规则、沿用文件值保留原始字节、切换保留未接管值、权限档案和旧沙箱冲突、组织名单过滤、选择性恢复、路径歧义及脱敏。执行笔记结构校验与源码差异检查。按项目验证约定，编译、自动化回归及本机客户端生效留待授权执行。

## Sources

- [Codex App Server](https://learn.chatgpt.com/docs/app-server) 与 [Codex 权限档案](https://learn.chatgpt.com/docs/permissions)。
- [Claude Code 配置作用域](https://code.claude.com/docs/en/settings) 与 [Claude Code 设置](https://code.claude.com/docs/en/settings-reference)。
