# Agent Note: 品牌图标的液态玻璃材质

Status: implemented

## Problem

用户看过 spiralcoder 项目的液态玻璃 logo 后，要求 Agent Switchboard 的品牌图标参考同一设计并实现一样的液态玻璃效果，且应用内图标、favicon、exe/窗口/托盘栅格图标全部统一替换（明确推翻此前"图标保持扁平"的偏好）。旧材质是平铺线性渐变，没有透光、边缘厚度和内部流动，直接给母版加滤镜又必须同时满足浏览器与 resvg 两套渲染器。

## Decision

`src/assets/app-icon.svg` 继续[由同一矢量母版生成](../process/2026-10-14-brand-icon-single-source.md)：几何不变——两条旋转对称折返路径组成 S、中部斜向留白表达切换，四段品牌渐变色值 `#3279F6 / #8055ED / #ED68AB / #FFB36B` 及方向逐字保留。本篇拥有的材质决定：母版整体改为液态玻璃光学管线，层次自下而上为——

1. 形状拟合的原色渐变底（opacity .25），色站透明度 .62–.82，流动矩形使用同一透光渐变，不再覆盖不透明色块；
2. 两块超出画布的大矩形分别承载原色渐变与流彩光谱（`#4C8DFF → #8B7CFF → #ED68AB → #FFB36B` 循环），共享一条 7s ease-in-out 对角平移动画，光谱副本叠加 9s 透明度交换并以负延迟错相；
3. 斜向白色光泽层，动画相位再偏移半周期；
4. 沿路径内侧的低强度原色深边（stroke 18、opacity .38）与白色内缘光环（stroke 12，左上强右下弱）表达厚度；
5. 贴合曲面的反射带、细折痕、顶条高光与模糊底缘回光表达玻璃受光，避免粗硬的同心描边；
6. 3px 外缘描边（白重紫轻渐变）与双层低强度 `#071725` 落影。

动画以 SVG 内嵌 `<style>` 拥有。桌面端唯一组件 `AppLogo` 从母版 raw 导入，使用 React useId 隔离每个实例的 defs 与引用；不复制几何，不保留外部 img 的桌面路径。内联后同一 CSS 同时响应系统 `prefers-reduced-motion` 与应用 `data-motion="reduce"`，即时停止流动与色彩交换。`forced-colors` 隐藏玻璃层，显示 `CanvasText` 实色形体并移除阴影。favicon 与 README 直接消费 SVG，原生图标由 resvg 静态渲染；光谱矩形的 `opacity="0"` 保证静态构图完整。不引入 `feSpecularLighting`/`feDiffuseLighting`（resvg 0.45 在 blur→lighting 链上 panic）、不改四个原色值、不加背景块。

## Related decision audit

- [品牌图标由同一矢量母版生成](../process/2026-10-14-brand-icon-single-source.md)：部分取代。单一母版、`icons:sync` 生成链、安装器嵌入高分辨率 PNG 的决定全部保留；该篇"不引入滤镜"的材质事实由本篇的液态玻璃管线取代，已在旧篇标注。
- 检索 proposed/（空）与 implemented/ 中 icon、logo、图标关键词：[侧栏图标状态](../feature/2026-10-12-sidebar-icon-states.md)与[偏好预览卡](../feature/2026-10-12-preference-preview-cards.md)只拥有导航语义与卡片解剖，无关。无 rejected 记录。
- spiralcoder 的参照实现为其 `web/public/logo.svg`（单一 blob + 7s 流彩/9s 色彩交换 + 镜面高光 + 玻璃边），本篇保留其时间轴与层次次序，形体换成 S 折返路线，配色换成现有品牌渐变。

## Alternatives considered

- 桌面继续以 img 引用 SVG：浏览器隔离天然避免 ID 冲突；但父文档的动效偏好无法控制图片内部动画。统一内联同一受版本控制的母版，以实例命名空间隔离引用，不维护第二份静态母版。
- 保留 `feSpecularLighting` 曲面浮雕（spiralcoder 同款）：浏览器渲染效果最接近参照物；但 `npm run icons:sync` 的 resvg 0.45 在 feGaussianBlur→feSpecularLighting 链上触发 `lighting.rs` 尺寸断言 panic，显式子区域也无效，栅格是 exe/安装器的硬输入，不能两套实现。
- 光谱流动层也用表现属性常显：省掉 CSS 依赖，但静态栅格会永久叠加光谱色，偏离原色识别；`opacity="0"` + 浏览器动画覆盖是唯一同时满足两渲染器的组合。
- 把 glass 形体换成 spiralcoder 式单 blob 容器再内嵌 S：玻璃感更"完整"，但放弃现有 S 形识别等于换品牌，且两条折返路径的斜向留白正是切换语义的承载。
- 银灰/冰蓝无色玻璃调：液态玻璃的经典表达，但用户明确要求保留蓝紫粉橙渐变，参照项目也是以原色层 + 白色光照表达厚度。

## Consequences

应用内标题栏、加载层、设置关于页、托盘面板、偏好预览、favicon、README 头图与全部原生栅格从同一母版获得玻璃材质。原生图标使用未执行 CSS 动画的静态构图；浏览器错相动画的初始画面不宣称与栅格逐像素一致。代价是桌面内联实例需要隔离 SVG ID；透光颜色会受背景影响。docs/ 与官网界面截图仍为已标明日期的历史画面，不回写历史图片。真实任务栏、托盘与安装器运行显示需要打包后验收。

## Testing

- 本轮 `npm run icons:sync` 成功生成六种原生 PNG/ICO/ICNS；母版、原生 SVG、官网 favicon 的 SHA-256 均为 `2fc9ddefc003dbc479a49e8f12e4bda7d61faaefa6960fda23242e714c701563`。512px PNG 四个主体采样 Alpha 为 217/202/202/203，替代旧版的全 255。
- 人工检查生成 PNG 在浅色与深蓝背景上的 256/128/64/32/24/16px 合成预览；S 形、斜向留白与蓝紫粉橙方向保留，小尺寸没有额外背景块。此证据是栅格目检，不是 React 或浏览器动画运行验收。
- `npm run verify-notes` 通过 104 篇笔记及 10 篇归档校验；`git diff --check` 通过。源码检索确认桌面五处消费者统一使用 AppLogo，raw 母版只有组件一处导入；系统媒体查询和应用 data-motion 的样式优先级经静态审查。
- 未运行 typecheck/自动化测试/编译；本轮涉及 React 消费路径，静态审查不能替代桌面运行验收。真实桌面托盘、任务栏与安装器显示未验收。
