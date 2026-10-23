# Agent Note: 界面文案密度收敛，移除冗余副标题与描述小字

Status: implemented

## Problem

0.3.3 桌面 UI/UX 审计（[desktop-workspace-uiux](../architecture/2026-10-13-desktop-workspace-uiux.md)）之后，界面骨架已收敛（标题独占一行、`WorkspaceHeader`/`ModuleHeader` 行文法固定），但正文里的二级小字没有同步收敛：字段 `.detail` 提示、面板导语、弹窗说明中大量复述标题或相邻按钮的句子，或把同一事实在页头、模块导语、行内提示三处重复。用户要求逐界面（主界面 → 子界面 → 子模块 → 弹窗）移除不必要的副标题与小字描述，必要的描述压成一行短句。

## Decision

文案密度的判定规则只有两条，按界面逐个套用：

1. **复述即删**：句子只重述标题、标签、复选框状态或旁边按钮的文字 → 删除渲染与文案键，不留空壳键。
2. **必要即一行**：句子承载独有事实（安全后果、数据口径、格式契约、交互教学）→ 保留但压成一行短句；同一事实在同一视野出现两次 → 只留信息量最高的一处。

偏好设置的[增强预览卡](../feature/2026-10-12-preference-preview-cards.md)按用户明确选择，每卡常显一条说明，窄卡允许换行以完整表达选项后果；该局部规则不恢复设置行重复副标题，其余界面仍遵守本文规则。

本轮落地的删除（渲染与键同批移除）：

- 设置页 `settings.appearance.scale.detail`、`settings.window.startMinimized.detail`（逐字复述开关标签；缩放效果由预览卡自证）。
- 网关面板导语 `gateway.intro`（复述「本机网关」标题，拓扑图自明）及其孤儿样式 `.asb-gateway-intro`；最近请求空态段 `gateway.noClientRequests`/复用段（标题旁计数行已说「{app} · 暂无记录」）。
- 图表兜底键 `usage.composition.noData`、`usage.trend.noData`（三个调用方全部传必填 `emptyMessage`，兜底不可达）。
- 云备份上传确认 `backup.cloud.uploadConfirm1`（复述面板范围；覆盖警告与密码承诺两条保留）。
- Codex 模型目录行 `imageInputNoteProfileYes/No`（复选框状态自明，「默认启用」并入 `fetchNote` 一行）；工作场景 `newScenarioMeta`（复述选项名）；`plans.restoreNote` 第二句（与模块导语重复）。
- 扩展区：加载失败空态 `loadFailedDetail`（复述相邻重试按钮）；检测结果 `cancelledDetail`（复述徽章，outcomeView 空文本时不再渲染 span）；停用范围弹窗 `disableScope.body`（标题即提问，选项即答案）；Skill 仓库导语 `summaryLead` 的静态半句（保留「共 N 个仓库」计数，事实留给删除确认处的 `removeNote`）；新建 Skill 名字段的硬编码占位符 `"note-helper"`（顺带修复的翻译缺口）。
- 供应商编辑器连接测试状态行 `test.expanded`/`test.ready`（复述开关态）；仅保留有真实前置条件的 `test.needFields`。
- 便携导入路径标签括号里的「不含密钥」承诺（该事实由导出侧 `extensions.export.note` 唯一拥有）。

第二轮收口（同批删除的重复键与死代码）：

- 跨域同文键收敛到单一所有者：`codex.editor.runtimeParamsNote`（两处渲染）→ `providers.editor.parametersNote`；`codex.connection.directNote` → `providers.editor.route.direct`；`codex.connection.modelsUrlNote` → `providers.editor.modelsUrlNote`；`providers.test.connectivityNote` → `providers.probe.note`（文本合并为「仅确认服务地址可达：不携带密钥、不发送模型请求，也不验证密钥有效性。」，覆盖连通性测试与探针两个渲染面）。
- 空态说明行样式 `.asb-empty-state-detail`（本轮后零使用方）连同 panels.css 标记契约注释中的 `<p>` 行一并删除——空态形状收敛为「图符 + 标题 + 至多一个动作」，标题独自承担说明。

压成一行的长文案（键保留、文本收敛）：`usage.radar.statusNote`（去掉复述页签的首个分句）、`usage.query.fieldsNote`/`scriptNote`、`gateway.portRangeNote`/`telemetryNote`/`discardConfirmCopy`/`emptyRoutesCopy`/`latencyOnlyCompleted`、`codex.probe.note`（额度消耗事实已由页头状态行与空态拥有）、`codex.model.fetchNote`、`codex.chatReasoning.note`、`codex.subagent.crossHelp`/`defaultDetail`、`codex.params.pageNote`、`providers.parameters.help`、`providers.editor.responses.minimalNote`/`standardNote`、`clientConfig.modelEffort.help`、`clientConfig.review.scopeHelp`+`scopeCodexHelp`（两段并一段）、`clientConfig.reset.advancedCheckboxHelp`、`clientConfig.extra.actionHelp`、`codex.plans.description`、`importDiscovery.cc/sql.emptyHint`、`importDiscovery.export.note`；诊断修复回执 `doctor.backup`+`backupLocation` 由两行并一行。终扫补收的最后四条：`codex.probe.visual.note`（历史保存句与同视野页头状态重复）、`recovery.body`、`settings.window.startupPage.detail`、`backup.cloud.guide.save.body`（删去「认证请求头自动推导」这一不可操作实现细节）。机械判据：除云端备份配置引导的两条步骤正文（每句一个独立动作或安全约束）外，全部 `.detail/.note/.help/...` 类键的中文不超过一个句号。

保留不动（各有独有职责）：所有破坏性操作确认的后果清单（恢复/删除/会话批量删除/端口恢复记录丢弃）、数据口径与来源声明（Runway feed、官方额度、网关 512 条保留策略）、格式契约（Responses 模式语义、协议 URL 推导、认证头）、交互教学（拓扑点击筛选）。后端中文字符串跨边界显示的缺口由 [discovery-localized-message-contract](../architecture/2026-10-13-discovery-localized-message-contract.md) 接管；CcSwitch/SQL 导入面的 `warnings`/`reason` 裸中文仍在该笔记中登记为已知缺口。

## Alternatives considered

- **只删不压（更激进）**：最强理由是视觉最简。否决：安全后果、数据口径与格式契约是用户无法从界面推断的事实，删掉后确认弹窗只剩标题，破坏性操作失去知情基础。
- **引入折叠/tooltip 收纳长说明**：最强理由是信息不丢且默认干净。否决：为本轮目标新增交互组件与状态，超出「移除与压短」的范围；确有需要的未来场景再单独提案。

## Consequences

界面各视野内二级文字至多一行；同一事实单一拥有者（如「密钥不入便携包」归导出侧、「额度会消耗」归页头状态行）。代价：被删句子里极少数边缘信息随之消失（如 `test.expanded` 的展开回显、上传确认里对加密范围的复述），均属可由界面状态自证的冗余。验证：四个并行盘点覆盖全部工作区/弹窗后逐项决策；全库 `grep` 确认 12 组删除键零残留、消息文件无重复键；`npm run verify-notes` 通过。按项目规则未运行 `typecheck`/`test`，编译一致性靠键名引用对查（`t()` 与 messages 双向核对）。
