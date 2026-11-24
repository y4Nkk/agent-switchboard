# Agent Note: 用途路由以目标卡片直接编辑

Status: implemented

## Problem

表格和单行检查器将去向与改动分开。把插座和线缆放进原表格行仍保留了同一条拥挤的读数轴，窄窗也难以同时辨认用途、状态和模型。

## Decision

Codex、Claude 的编辑器与网关已保存方案共用用途卡片。每张卡片先读用途和状态，再看带原创 SVG 的目标。编辑卡片常驻模型选择和来源，网关卡片以整个卡片直达对应编辑位置。主模型归模型目录所有。Claude 的 Fable 继承复用实际路由解析，当 Opus 的目标不可用时两个去向均显示原因。

## Related decisions

[供应商配置减负](2026-11-16-provider-configuration-relief.md)继续拥有目录和预设，配置用途表格由本篇接管。[图形化模型路由](../architecture/2026-10-26-graphical-model-routing.md)与[供应商统一编辑](../architecture/2026-11-13-provider-routing-editor-integration.md)继续拥有唯一档案草稿和保存边界。[网关空间拓扑](2026-10-12-gateway-spatial-topology.md)继续拥有实时连接图。

## Alternatives considered

React Flow 式自由画布擅长表达任意节点和分支；此处用途由客户端定义，固定卡片能在一次点击内完成选择，并容纳稳定的键盘与窄窗口布局。图标和材料参考 [React Flow](https://reactflow.dev/examples/styling/turbo-flow)、[Excalidraw](https://github.com/excalidraw/excalidraw) 与 [Apple Freeform](https://www.apple.com/newsroom/2022/12/apple-launches-freeform-a-powerful-new-app-designed-for-creative-collaboration/)。

## Consequences

用户可以在同一张卡片比较当前去向与候选，并单击更新草稿。网关显示已保存的去向。图面使卡片比原表格更高，窄内容区单列呈现，长名换行，状态与错误始终有对应文字。

## Verification

在模拟应用中由网关进入 Codex 子 agent 和 Claude Fable，确认定位、直接点选模型与继承转换。780 像素深色窗口中，卡片单列重排，无横向溢出。失效的 Opus 和继承它的 Fable 各显示原因。内置浏览器的截图复核覆盖了共享按钮的深色底色。
