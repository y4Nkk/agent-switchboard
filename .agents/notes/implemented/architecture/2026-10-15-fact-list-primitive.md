# Agent Note: 全局键值事实行原语 FactList 统一

Status: implemented

## Problem

「标签在左、数值在右、逐行堆叠」的键值事实行有八个消费点、三套并行实现：`.asb-fact-row`（`panels.css`，flex + `--asb-fact-label-width: 6em` 定宽标签列，值用 label 字阶）、`.asb-session-facts`（`sessions.css`，`auto 1fr` 两列网格，值用 body 字阶）、`.asb-provider-resolved-endpoint`（`provider-editor.css`，同型网格 + 值等宽字阶 + 窄屏收拢单列）。三套 CSS 各自声明同一视觉契约——`panels.css` 的注释自称拥有该模式，另两套并未归一；`--asb-fact-label-width` 只服务其中一套。组件层同样三分：`ExtensionResourceFacts` 的本地 `Facts`、`ConfigStatusPanel` 的本地 `StatusField`，其余手写 `<dl>`。跨行对齐行为因此不一致：定宽列与自适应列给出的值起点不同。

路径、版本、状态和测量值采用相同的文本外观时，需要逐字阅读才能分辨事实类型。市场扫描先在调用处增加阶段图标，各页面继续自行制作会产生不同的尺寸和状态色。

## Decision

`src/components/FactList.tsx` 是软件内全部键值事实行的唯一原语，`styles/base/panels.css` 的 `.asb-fact-row` 是其语法的唯一拥有者：

- 保留 `<dl class="asb-fact-row"><div><dt/><dd/></div>…</dl>`、`rows` 与 `inline`。当业务数据有明确含义时，行可声明 `icon`、图标的 `tone` 以及值的 `format`。图标装饰隐藏于辅助技术，标签和值提供完整文本。
- 两列布局继续统一基线和值列起点。标签列以最长标签为基准，占用最多四成宽度，长标签和长值在各自的列内换行。同行对齐图标与文本，共用一套尺寸、底色和活动、警示色。等宽值用带淡底的语义 `code`，读数使用表格数字。
- `FactStatus` 复用现有状态胶囊。调用处根据实际状态声明色调，展开内容继续使用原有的时间、路径和操作组件。市场的阶段图标、导入的管理状态、配置写锁与用量读数使用同一套样式。
- `inline` 变体（网关摘要横轨）：dl 退回可换行横轨、包装 div 退回 flex 行，标签取自然宽度——共享标签列只在值逐行向下时才有收益。
- 行内值的专属排布（`.asb-code` 等宽、`.asb-num` 数字、`.asb-warn-text` 警示色、`.asb-fact-path` 路径控件、会话恢复命令的 `.asb-session-resume-fact` 行内 flex）由行值自身携带，原语不拥有值内部布局。
- 供应商编辑器的解析端点字段保留模块级间距规则 `.asb-provider-endpoint .asb-fact-row { margin-top }`——全局原语之上的业务布局，归 `provider-editor.css`。

最初统一八处事实行时，删除 `.asb-session-facts`、`.asb-provider-resolved-endpoint`、`--asb-fact-label-width` 和本地 `Facts`、`StatusField` 辅助组件。`DESIGN.md` 记录共享组件的外观契约。

客户端配置来源复用两列事实行，完整保留配置键、值与来源。当前连接卡、网关延迟分位数和最近全局重置使用行内变体。当前连接卡的列宽、截断与完整值提示由卡片布局继续负责。版本和普通版本来源用共享代码样式，长错误内容沿用警示正文。图表、差异和长请求诊断保留各自的数据阅读布局。

同主题笔记审计：全库检索无任何笔记持有事实行契约（panels.css 注释是唯一的口头声明），无可归档或互链的旧篇；[DatePicker](2026-10-15-global-date-picker-primitive.md) 与折叠原语笔记不触及键值行布局。

## Alternatives considered

- 根据标签或值的字符串自动选择图标可以省去逐处声明。文字会随语言变化，同样的数字也可能代表版本、端口或数量，图标应由知道业务含义的调用处声明。
- 每项都做成有边框的卡片能立即形成视觉分组，配置和请求诊断的长列表会被重复外框拉长。图标、字形和状态胶囊可直接表达差异。
- 保留 flex + 定宽标签列：最强理由是列宽与最长标签解耦——某行标签特别长时不会把整列值推远，且各行的标签-值间距恒定。否决：本产品的标签全是短 i18n 短语（现存最长「其中：配置恢复备份」），6em 定宽要么留白要么截断；auto 列与用户已见、已认可的存储/会话两处完全一致，且单一网格天然给出跨行对齐，不需要 token 参与。
- children API（`<FactList><FactRow label value/></FactList>`）：最强理由是条件行与带 key 行表达更自然，不必用数组 spread。否决：八个消费点里多数从数据数组出发，rows 数组最短；条件行用条件 spread 表达同样直接；数组 API 的暴露面更小。
- 把「标签在上、数值在下」的请求诊断 dl（`ProviderRequestPanel`）与 Codex 重置记录卡也并入：最强理由是进一步消灭手写 `<dl>`。否决：那是刻意不同的模式——URL、Request ID 等长等宽值竖排更可读，与横排事实行不是同一契约，强行归一会推翻既有的可读性决策；`DESIGN.md` 的禁止项针对的是平行组件，不是形态不同的独立布局。
- 只统一组件层、保留三套 CSS：最强理由是零视觉变化、改动面最小。否决：三套并行实现本身就是技术债，组件归一而样式三头所有只是把债藏进样式层，违背本次目标。

## Consequences

- 为路径、识别码、时间和状态提供稳定视觉线索。图标占用部分标签列，狭窄容器会更早换行。版本轨道和场景槽卡的专属图形继续由对应布局拥有。
- 收益：八处获得同一标签列对齐与同一字阶；新增事实行只有一个入口；`--asb-fact-label-width` 与两套平行类消失；诊断、导入、扩展面板的值列对齐到标签列右缘、值升为 body 字阶，长路径与数值更可读。
- 代价：`display: contents` 意味着包装 div 不参与渲染盒——若未来要做行级装饰（底色、分隔线），必须挂在 dt/dd 或重建行容器，不能挂包装；诊断、导入、扩展三处有可见的视觉变化（对齐与字阶），属于统一的预期结果而非回归。
- 后续任何键值事实行必须经 `FactList`；不得再手写 `<dl>` 事实结构或新增平行键值行布局类。修改行语法只改 `panels.css` 的 `.asb-fact-row` 块。

## Testing

- `npm run verify-notes` 通过（笔记结构门禁）。
- 按工作区规则未运行 typecheck/test（用户未要求）；人工核对：全库 `asb-session-facts`、`asb-provider-resolved-endpoint`、`asb-fact-label-width`、组件内 `<dl className="asb-fact-row">` 与旧辅助组件名零残留；八处消费点逐条对照原渲染——条件行、警示色 span、等宽值、`asb-num` 数字、`FactPath`、恢复命令的行内 flex（类移至值内 span）全部保留，仅承载结构变更。
