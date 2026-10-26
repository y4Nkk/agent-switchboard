# 软件截图与演示数据

本目录的截图供中英文 README 和官网共用。除模型路由外，7 张截图于 2026 年 10 月 14 日从本地 0.3.3 生产前端产物截取；`model-routing.png` 于 2026 年 10 月 26 日从当前模型路由 React 组件与全局样式截取，替换旧子 agent 参数表单截图。它们均展示中文界面，不代表已发布安装包的验收结果。

## 数据边界

- 使用隔离临时目录、无头浏览器和空白浏览器上下文，只加载本地前端，不操作桌面窗口；新模型路由图加载当前组件源码，其余图片加载 0.3.3 生产产物。
- 在页面加载前接管 Tauri 调用，只返回演示档案、配置预览、余额、用量、检测历史和网关状态；不启动真实配置后端。
- 浏览器只允许访问截图专用的本机前端地址，阻断外部请求；不调用模型服务，不读取或写入个人配置、凭据、真实会话或业务数据。
- 供应商仅使用演示档案与占位凭据。原截图使用 `.example` 地址和 `C:/Demo/` 路径；模型路由图使用 `fixture.invalid` 地址与 `isolated/` 配置预览路径，不连接模型服务。
- 余额、Token、检测通过数、耗时和网关请求均为模拟值，不是价格、配额、性能或可用性的实测声明。

## 模型名称

本次更新界面与品牌展示，演示模型标识沿用 2026 年 10 月 14 日选定的名称与来源：

- Codex：`gpt-6.1-sol`、`gpt-6-astra`、`gpt-6-luna`。来源：[OpenAI 官方模型文档](https://learn.chatgpt.com/docs/models)。
- Claude：`claude-sonnet-5-5`；演示档案中的其他角色使用 `claude-opus-5-5`、`claude-haiku-5-5` 和 `claude-fable-5-1`。来源：[Claude 官方模型概览](https://platform.claude.com/docs/en/models/overview)。

这些标识仅用于说明界面，不表示演示供应商实际提供对应模型。

新模型路由图使用 `gpt-6-sol` 与明确的模拟标识 `claude-sonnet-fixture`，只用于辨识节点，不声明真实模型的能力或可用性。

## 图片归属

| 图片 | 展示内容 |
| --- | --- |
| `providers.png` | 当前连接、OpenAI 档案与模拟余额 |
| `switch-preview.png` | 模型及运行参数的切换差异、候选文件与备份位置 |
| `client-configuration.png` | 子 agent 运行设置与终端偏好 |
| `usage.png` | 模型消耗构成、七日趋势与明细 |
| `radar.png` | 五次模拟检测的结果与消耗 |
| `radar-history.png` | 模拟检测历史及筛选入口 |
| `gateway.png` | 模拟网关连接拓扑、协议与请求状态 |
| `model-routing.png` | 客户端、用途与供应商模型的图形配置及默认子 agent 目标 |

官网使用相同图片的副本，位于 `website/public/media/screenshots/`；图片说明和尺寸由 `website/src/content/site-content.ts` 拥有。更新截图时同步替换两处图片，并按实际画面更新两份 README 和官网说明。
