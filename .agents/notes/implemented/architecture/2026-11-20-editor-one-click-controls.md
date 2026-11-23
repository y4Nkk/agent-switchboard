# Agent Note: 供应商编辑器一击化——常驻控件取代值编辑回合

Status: implemented

## Problem

供应商编辑器沿用以 `ProviderValue` 为核心的值编辑回合：每个值先显示为文本，点「修改」才挂控件，改完点「完成」收起（供应商能力、Chat 转换、模型限额、运行参数、连接协议、1M 路由共 15 处调用点）。Codex 模型目录再叠一层行级「编辑」按钮展开能力表，能力值本身又是「启用/修改」回合；推理等级用逗号分隔的自由文本输入，8 档封闭枚举被当成开放字符串让用户手敲。调研 cc-switch（farion1231）与 ZCode 官方仓库后结论一致：同类工具全部采用常驻内联控件，枚举全部结构化（cc-switch 的档位多选控件注释明确「后端丢弃未知值，所以 UI 只提供规范档位」），没有一处编辑回合。

## Decision

**删除 `ProviderValue` 与编辑回合。** 编辑器内每个值恰好有一个常驻控件，一次点击直达：

- 枚举 → `Select`（封闭单选）或 `ToggleChip` 多选 chips（封闭多选）；开关语义 → `Switch`。
- 数值 → 常驻 `Input type="number"`，空值即默认。
- 「修改 / 完成 / 待补全」按钮、`editing` 状态与焦点回退逻辑全部删除，无兼容层。

**推理等级是 8 档 chips 多选，不是文本。** `REASONING_LEVELS`（= Rust `CODEX_REASONING_LADDER`）按深度渲染为固定顺序 chips，点击切换选中；默认档是 `Select`，选项 = 已选档位。清空归一到 `["none"]` / 默认 `none`；`reasoning` 能力关闭时整组档位 UI 消失（联动置 none 的既有契约不变）。非法档位在 UI 层不可能产生，`invalidLevels` 校验退化为对历史数据的兜底。

**Codex 模型目录是常驻模型卡片，不是表格加行编辑。** 每模型一张卡片：模型 ID + 来源徽标 + 主模型胶囊/按钮 + 删除（dangerText）；限额行 = 窗口与输出上限的常驻 number Input，各带一排**限额档位 chips**（上下文 128k/200k/256k/400k/1M，输出 8k/16k/32k/64k/128k，点击写入、当前值命中档位时高亮；取值与内置快照真实值精确对齐）；能力行 = 6 个 toggle chips；推理开启时追加档位 chips 行。`selected` 行编辑状态与 `CodexCatalogDetails` 删除。`withCatalog` 能力汇总、`catalogEntry` 预设补全、`modelSource` 来源徽标、保存校验与数据 shape 全部不变。

**添加模型以批量勾选为主路径。** 「获取模型」成功后自动打开 `ModelPickerDialog`：候选 = 拉取结果 ∪ 端点匹配预设的模型（协议一致才并入），带搜索、全选/全不选；勾选即加入（走 `catalogEntry` 自动补全），取消勾选即移除，应用一次生效。手输 ID 保留为兜底（中转站 `/models` 常不可用），降级为次要入口。Claude 目录接入同一弹窗。

**能力声明区与 Chat 转换配置改为 Switch 行 / 常驻 Select。** 供应商级操作（模型列表、Chat Completions 等 5 项）是「标签 + Switch」设置行；Chat 思考转换的启停是 Switch，4 个子参数是常驻 Select/Switch。运行参数每个 spec 恰好一个常驻控件（toggle → 默认/启用/禁用 Select；slider/segment → 默认+选项 Select；integer → number Input 空=默认），显式值的「自定义」caption 保留。

参考结论归属：cc-switch 的常驻内联与档位多选、ZCode 的 chip 编辑与基数退化（0 选项隐藏、不渲染假可用控件）。

## 契约归属

- `ToggleChip`（`src/components/ToggleChip.tsx`）拥有 chip 的全部几何与 `aria-pressed` 语义；样式归 `provider-editor-sections.css` 的 `.asb-chip-*`。
- `useProviderConnection.fetchModels` 以数组表示当前请求成功，以 `null` 表示失败、跳过或结果已过期。Codex 与 Claude 目录仅在收到数组时打开 `ModelPickerDialog`，并传入合并后的候选和当前集合；请求错误显示在目录的获取入口下方，勾选和搜索由弹窗管理。
- 限额档位常量归 `CodexModelSection`（仅此一处消费）。
- 数据契约（`CodexCatalogEntry`、`CodexCapabilities`、参数 shape、Rust 侧）零变更。

## Related decisions

- [供应商配置减负](../feature/2026-11-16-provider-configuration-relief.md)：其「只在主动修改时呈现值控件」的值编辑回合被本篇取代；预设补全、能力单处编辑、汇总与校验所有者不变。
- [供应商高级设置完整显示](../simplification/2026-10-13-provider-advanced-settings-always-visible.md)：常显原则被本篇推到控件层，继续成立。
- [供应商目录动作按钮契约](../feature/2026-11-20-provider-catalog-action-buttons.md)：dangerText 变体与主模型胶囊继续使用；Codex 目录改为卡片后 `providerSetup.actions` 列头只剩 Claude 目录表格消费。

## Alternatives considered

- 保留 `ProviderValue` 值编辑回合（2026-11-16 决定）。它最强的理由是扫读时只见结果值、同屏只挂一个输入控件，编辑器保持紧凑。不用：用户明确否决多级操作；枚举结构化后 chips 的选中态本身就是结果值，「扫读值」与「一击修改」不再互斥，回合只剩成本。
- ZCode 式 chip 原位文本编辑（点击 chip 变输入框改档位名）。它最强的理由是支持任意自定义档位标识。不用：Codex 只认 8 档封闭枚举，自由命名必然产生被后端丢弃的值；自定义在这里没有合法出口。
- 每模型编辑区收进 collapsible 折叠。它最强的理由是 5 模型 × (2 限额 + 6 能力 + 8 档位) 常驻的视觉密度。不用：折叠仍是二级操作；预设自动填正确后 chips 处于目标态，密度即信息本身。
- 打包未裁剪的 models.dev 快照可以完整保留各类模型资料。档位 chips 只需要公开档位等有限字段，全量打包增加体积。生成快照的字段范围由[供应商配置减负](../feature/2026-11-16-provider-configuration-relief.md)持有。

## Consequences

- 收益：编辑器内任何值一次点击可达；推理等级与能力全部点选，手敲只发生在真正的开放值（端点、密钥、上游模型名）；添加模型对主流供应商全程零键入；`ProviderValue`、`CodexCatalogDetails`、编辑/完成按钮与逗号解析路径全部删除，交互面收窄为三类原语（Input、Select/Switch、chips）。
- 代价：模型卡片常驻编辑区增加目录区高度（由编辑器中部滚动承载）；档位 chips 对快照未收录的限额值不高亮，只作快捷键不作验证；批量弹窗多一个临时勾选态，应用前草稿不变。
- 数据契约零变更：旧档案打开即正常渲染；保存路径与校验时机不变。
