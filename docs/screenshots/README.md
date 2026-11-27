# 界面截图与演示数据

这 13 张图片于 2026 年 11 月 27 日从 v0.3.36 前端源码截取，使用简体中文界面。中英文 README 共用这组图片。官网使用同图的副本。

## 截图来源

- 使用临时浏览器配置目录、独立无头 Chromium 和本机前端服务。截图加载项目的 React 页面、样式、字体与 SVG。
- Tauri 调用由内存中的演示数据响应。页面请求只允许访问本机预览服务。捕获过程未启动原生后端，也未读写个人配置、凭据、会话或扩展库。
- 供应商档案、模型和端点为示例，例如 Atlas、Nova、Orion、Lyra，`atlas-code`、`orion-mini`、`https://atlas.example/v1`。配置和备份使用 `C:/Demo/` 下的示例路径。
- 用量、额度、延迟、请求结果、检测结果及仓库扫描均为模拟读数。鹈鹕画面来自项目内置的参考动画。

## 图片索引

| 图片 | 展示内容 |
| --- | --- |
| `providers.png` | Codex 与 Claude 当前连接、演示档案和余额 |
| `switch-preview.png` | 切换差异、脱敏候选配置及备份位置 |
| `client-configuration.png` | 分组设置、子 agent 和默认权限档案 |
| `usage.png` | 五项累计读数、52 周 Token 活动、按模型日趋势和占比 |
| `request-performance.png` | 采集入口、性能概览、趋势和细分 |
| `radar.png` | 检测设置、鹈鹕参考画面和模拟批次结果 |
| `radar-history.png` | 模拟历史记录及时间、档案、状态筛选 |
| `gateway.png` | 网关监听状态、端口和实际连接拓扑 |
| `gateway-monitoring.png` | 已保存的模型方案、分段环图与最近请求 |
| `model-routing.png` | 供应商编辑器里的主模型、跨档案子 agent 路由及悬浮操作 |
| `extensions.png` | Skills 扩展库、客户端启停、搜索和新建入口 |
| `skill-market.png` | Skills 仓库扫描结果、各请求阶段和一键解决 |
| `extension-deployment.png` | 部署与诊断的目标、状态、版本与来源 |

更新图片时，将同一 PNG 同步到 `website/public/media/screenshots/`，并按实际画面更新中英文 README、官网 `website/src/content/site-content.ts` 的说明与像素尺寸。
