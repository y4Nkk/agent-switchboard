# Agent Switchboard 网站

独立静态介绍站：Vite + React + TypeScript，与桌面应用工程完全分离。
内容契约见 `src/content/site-content.ts`；视觉令牌唯一所有者是 `src/styles/tokens.css`。

## 本地开发

```bash
cd website
npm ci
npm run dev        # http://127.0.0.1:1421 之前请以命令输出为准
npm test           # vitest
npm run build      # tsc --noEmit && vite build，输出 dist/
npm run preview    # 预览生产构建
```

配置积木台的静态证据由桌面端适配器生成；变更核心渲染规则后，在仓库根目录运行：

```bash
npm run verify:assembly
```

该检查保证已提交的官网展示文件仍与桌面端适配器输出一致；Vercel 只读取这一静态产物，不执行 Rust。

## Vercel 部署（Git 集成，无需令牌与脚本）

1. 在 vercel.com 导入 GitHub 仓库 `y4Nkk/agent-switchboard`。
2. 项目设置：
   - **Root Directory**：`website`
   - **Framework Preset**：Vite（或 Other，构建命令相同）
   - **Build Command**：`npm run build`
   - **Output Directory**：`dist`
   - **生产分支**：`master`
3. 保存后，推送 `master` 即自动更新正式站点；其他分支获得 Preview 部署。
4. 下载入口唯一指向 GitHub Releases 最新页，不维护版本号与安装包文件名；
   无需因发版改动站点代码。

链接契约：

- 源码：`https://github.com/y4Nkk/agent-switchboard`
- 下载：`https://github.com/y4Nkk/agent-switchboard/releases/latest`
