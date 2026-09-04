<p align="center">
  <img src="src/assets/app-icon.png" width="72" alt="Agent Switchboard 图标">
</p>

<h1 align="center">Agent Switchboard</h1>

<p align="center">
  为 <strong>Codex</strong> 与 <strong>Claude Code</strong> 准备的本地配置控制台。<br>
  管理供应商档案、查看变更、确认切换；不必手改配置文件。
</p>

<p align="center">
  <a href="#界面">界面</a> · <a href="#核心能力">核心能力</a> · <a href="#开始使用">开始使用</a>
</p>

<p align="center">
  <a href="https://github.com/y4Nkk/agent-switchboard/actions/workflows/package.yml"><img src="https://github.com/y4Nkk/agent-switchboard/actions/workflows/package.yml/badge.svg?branch=master" alt="打包工作流"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-4c8bf5.svg" alt="MIT License"></a>
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-0f7ee8.svg" alt="Windows、macOS 与 Linux">
  <img src="https://img.shields.io/badge/data-local--first-1f8f6a.svg" alt="本地优先">
</p>

当你需要在多个供应商、模型或登录方式之间切换时，真正容易出错的往往不是选择本身，而是散落在客户端配置文件中的改动。Agent Switchboard 把这些改动收束为一条清晰的本地流程：保存档案，查看将要发生的变化，确认后再生效；需要回退时，直接从历史备份恢复。

它只服务 Codex 与 Claude Code，不是模型网关，也不替你转发请求。

## 界面

<p align="center">
  <img src="docs/screenshots/providers.png" width="100%" alt="供应商编辑页：Amazon Bedrock 的服务地址、脱敏密钥与 GPT-6 Astra 主模型">
</p>

<p align="center">
  <img src="docs/screenshots/common-settings.png" width="100%" alt="通用设置页面：以开关、滑块和档位编辑配置">
</p>

<p align="center">
  <img src="docs/screenshots/switch-preview.png" width="100%" alt="切换预览：从 gpt-5-codex 切换到 GPT-6 Astra 的脱敏模型与路由差异">
</p>

截图来自隔离的演示环境，档案与密钥均为虚构；Amazon Bedrock 仅展示其公开服务域名。截图不包含用户私有服务地址、密钥、文件路径、账号、会话或应用数据。

## 核心能力

### 把配置变成可管理的档案

- 为 Codex 与 Claude Code 分别保存供应商名称、模型、服务地址和密钥；支持新建、编辑、排序、删除，以及从本机现有配置发现并导入。
- 自定义供应商与官方登录是两种清晰的连接方式。应用引导完成官方登录，但不会把登录令牌放进供应商档案或界面中。
- 通用设置、全局指令和供应商声明分开管理。切换供应商时，当前客户端只得到这次档案明确声明的设置。

### 在落盘之前看见改变

- 启用前先显示脱敏差异、候选配置和备份位置；没有确认，就不会修改客户端配置。
- 系统会识别外部编辑冲突和占用中的写入锁，让你先处理不确定状态再继续。
- 档案未声明的客户端设置保持原样，不会被一次切换覆盖。

### 让每一次切换可撤回

- 每次确认切换都会创建本地备份；可查看历史、恢复指定备份，或撤回上一次切换。
- 可选择把应用自己的配置备份到你控制的 Supabase 项目；云端内容在离开设备前加密，不包含原始客户端配置文件。

### 了解本机使用状态

- 查看 Codex 官方订阅额度、重置时间及只基于真实读取结果的本地趋势。
- 汇总本机 Codex 与 Claude Code 会话中的模型 token，按时间和模型查看趋势与构成；这与供应商余额明确分开。
- 从系统托盘快速发起切换，或只读浏览本机会话并在新终端中恢复。

## 使用方式

1. 新建一个供应商档案，或从本机已有配置导入。
2. 在档案中填写要使用的模型与连接信息；需要时调整通用设置。
3. 选择目标档案并打开变更预览，确认差异无误后执行切换。
4. 如果结果不符合预期，从备份历史恢复，或撤回上一次切换。

## 本地、透明、可控

- 默认在本机运行：没有账号体系、遥测或请求代理。
- 密钥只在必要时写入客户端要求的位置；预览、差异、日志和错误信息都会使用脱敏标记。
- 真实配置文件只会在你确认切换或恢复后变更；写入前备份，写入后验证。
- 应用不会注入客户端、修改安装包，也不会改写会话或 SQLite 数据。

## 开始使用

### 安装包

macOS、Linux 与 Windows 的直接安装包通过 [GitHub Releases](https://github.com/y4Nkk/agent-switchboard/releases/latest) 提供。Windows 直接下载继续使用 NSIS 安装包和应用内签名更新。

Windows 同时构建 Microsoft Store 专用 MSIX。该包只作为标签构建的 Actions 制品保留，供维护者提交 Partner Center；在 Store 认证并重签名之前，它不会作为 GitHub Release 资产公开分发。Store 安装的版本由 Microsoft Store 自动更新，不会调用 GitHub 更新器。

### 从源码运行

准备好 [Tauri 的平台依赖](https://v2.tauri.app/start/prerequisites/)、Node.js 和 Rust 后，在仓库根目录执行：

```bash
npm ci
npm run dev:desktop
```

若只需开发前端界面，可运行 `npm run dev:frontend`。构建和测试脚本见 [`package.json`](package.json)。

在 Windows 上，`npm run msix:build` 会先生成 NSIS 构建输出，再调用 Windows 10 SDK 的 `MakeAppx.exe` 生成 Store MSIX。MSIX 版本从 Cargo 版本映射为四段数字：`X.Y.Z` 对应 `X+1.Y.Z.0`；第四段保留为 `0`，避免与 Store 的版本规则冲突。

## 参与项目

欢迎通过 [Issues](https://github.com/y4Nkk/agent-switchboard/issues) 提交问题或建议。提交代码前请阅读 [AGENTS.md](AGENTS.md)，其中说明了配置写入、安全边界和验证要求。

## 项目文档

| 文档 | 内容 |
| --- | --- |
| [DESIGN.md](DESIGN.md) | `Frosted Relay` 的视觉与交互契约 |
| [progress.md](progress.md) | 当前阶段目标、验收与退出条件 |
| [AGENTS.md](AGENTS.md) | 贡献规范、产品边界与安全规则 |

## 许可证

本仓库中由 Agent Switchboard 编写的源码与文档以 [MIT License](LICENSE) 发布。第三方依赖继续遵守各自的许可证；本许可证不授予任何第三方商标的使用权。

产品名称 `Codex`、`Claude Code` 及相关商标归其各自权利人所有；Agent Switchboard 与 OpenAI、Anthropic 均无隶属、认可或合作关系。
