# Agent Note: 供应商运行参数沿用客户端设置行

Status: implemented

## Problem

Codex 与 Claude 的运行参数和客户端配置读取同类目录字段，但供应商端将封闭选项收进下拉框，文本参数也误用了数字输入。两处的当前值和编辑位置不对齐。

## Decision

用 `SettingsFields` 和同一设置行渲染两端运行参数。分段按钮负责封闭选项，滑杆负责有序档位，文本与数值由各自的常驻输入控件负责。Claude 的按模型设置用独立的单选组表达继承、原生自动与显式值。

## Related decisions

[供应商编辑器一击化](../architecture/2026-11-20-editor-one-click-controls.md)继续拥有常驻控件，运行参数的控件选择由本篇接管。[供应商高级设置完整显示](../simplification/2026-10-13-provider-advanced-settings-always-visible.md)继续拥有分区完整呈现的规则。

## Alternatives considered

运行参数独立保留下拉框可在窄窗口容纳长选项列表。项目已有会折行的客户端设置行，沿用它能让当前值与可选值处在同一视线，并避免维护两套目录控件映射。

## Consequences

供应商编辑中的当前值来自已保存档案，改动显示在待保存摘要。分段单选使可选值一直可见，长标签在窄窗口按共享规则换行，自定义文本输入在经过预设值时也保持编辑。参数、按模型字段和保存继续使用原有档案契约。

## Verification

在完整应用的两端编辑器观察运行参数区。Codex 的服务档位可输入 `flex`，Claude 可输入以预设名称开头的自定义输出风格，两处均显示待保存摘要。运行参数面板的可访问树中均无下拉选择控件。
