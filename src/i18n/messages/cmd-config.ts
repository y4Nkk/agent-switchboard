import type { MessageEntry } from "../types";

/** Backend command errors translated on the renderer side. Keys are emitted
 * by desktop commands via `CommandError::keyed/localized` (see the matching
 * src-tauri area); `message` stays the scrubbed raw diagnostic fallback. */
export const messages = {
  // commands/client_configuration_apply.rs (also manual/repair/gateway)
  "errors.cfg.clearExtraClaudeOnly": ["仅 Claude 支持清空 ASB 管理的额外通用配置", "Only Claude supports clearing the ASB-managed extra general configuration"],
  "errors.cfg.clientConfigUnreadable": ["无法读取真实客户端配置文件", "Could not read the real client configuration file"],
  "errors.cfg.previewStaleTargetChanged": ["真实配置或候选已变化，请重新预览", "The real configuration or the candidate changed; preview again"],
  "errors.cfg.previewStaleSettingsChanged": ["通用配置意图已变化，请重新预览", "The general configuration intent changed; preview again"],
  "errors.cfg.writeGateNotInitialized": ["写入闸门尚未初始化", "The write gate has not been initialized"],

  // commands/client_configuration_manual.rs
  "errors.cfg.manualPreviewStale": ["真实配置已变化，请重新读取后再编辑", "The real configuration changed; re-read it before editing"],
  "errors.cfg.manualNoChange": ["手动配置没有变更", "The manual configuration has no changes"],
  "errors.cfg.manualOwnedFieldRejected": ["手动配置只能修改界面未拥有的字段；请在对应界面修改：{paths}", "Manual edits may only change fields not owned by a UI page; edit them in the corresponding page: {paths}"],

  // commands/client_configuration_repair.rs
  "errors.cfg.repairPreviewStale": ["真实配置已变化，请重新读取后再修复", "The real configuration changed; re-read it before repairing"],
  "errors.cfg.repairCandidateStale": ["真实配置或修复候选已变化，请重新预览", "The real configuration or the repair candidate changed; preview again"],

  // commands/codex_project_plans.rs
  "errors.cfg.planLibraryStale": ["Codex 项目方案库已变化，请刷新后重试", "The Codex project plan library changed; refresh and try again"],
  "errors.cfg.planMissing": ["项目方案 {planId} 不存在", "Project plan {planId} does not exist"],
  "errors.cfg.planNameEmpty": ["项目方案名称不能为空", "The project plan name cannot be empty"],

  // commands/prompt_management.rs
  "errors.cfg.promptDocumentUnreadable": ["无法读取全局提示词文档", "Could not read the global prompt document"],
  "errors.cfg.promptDocumentLocked": ["全局提示词文档正被其他写入操作占用", "The global prompt document is currently held by another write operation"],
  "errors.cfg.promptDocumentChanged": ["全局提示词文档已在读取后被外部修改，请重新读取后再保存", "The global prompt document was modified externally after it was read; re-read it before saving"],
  "errors.cfg.promptSaveNotReplaced": ["保存全局提示词文档失败，原文未被替换", "Saving the global prompt document failed; the original content was not replaced"],
  "errors.cfg.promptSaveRestored": ["保存全局提示词文档失败，已恢复保存前的内容", "Saving the global prompt document failed; the content from before the save was restored"],
  "errors.cfg.promptSaveRestoreFailed": ["保存全局提示词文档失败，且无法自动恢复；请从应用备份恢复", "Saving the global prompt document failed and could not be restored automatically; recover from the application backup"],

  // commands/gateway.rs
  "errors.cfg.gatewayPreparationsUninitialized": ["端口修改准备状态尚未初始化", "The port-change preparation state has not been initialized"],

  // commands/query.rs
  "errors.cfg.geminiNativeCodexUnsupported": ["Gemini Native 不能用于 Codex 模型列表", "Gemini Native cannot be used for the Codex model list"],
  "errors.cfg.claudeNativeNoModelList": ["原生云 SDK 不提供此通用 HTTP 模型接口，请填写云服务已开通的模型 ID", "The native cloud SDK does not provide this generic HTTP model endpoint; enter a model ID already enabled by the cloud service"],

  // commands/status/mod.rs
  "errors.cfg.runtimeDataDirUnavailable": ["无法定位应用数据目录", "Could not locate the application data directory"],
  "errors.cfg.openContainingFolderFailed": ["无法打开所在文件夹", "Could not open the containing folder"],
  "errors.cfg.revealFileFailed": ["无法在文件管理器中定位该文件", "Could not locate the file in the file manager"],
  "errors.cfg.fileAndFolderMissing": ["文件与所在文件夹均不存在", "Neither the file nor its containing folder exists"],
  "errors.cfg.lockNotStale": ["当前锁不是可恢复的遗留状态", "The current lock is not a recoverable leftover state"],

  // commands/status/codex.rs
  "errors.cfg.codexConfigInvalid": ["Codex 配置格式无效", "The Codex configuration format is invalid"],
  "errors.cfg.codexDirectAuthUnreadable": ["无法读取 Codex 直连认证状态", "Could not read the Codex direct-connection authentication state"],
  "errors.cfg.gatewayRouteActiveForRestore": ["本机协议网关正在使用供应商；请先切换到直连或官方登录后再恢复云端备份", "The local protocol gateway is using a provider; switch to direct or official sign-in before restoring the cloud backup"],
  "errors.cfg.subagentSettingsUnreadable": ["无法读取用户级 Codex 配置", "Could not read the user-level Codex configuration"],
  "errors.cfg.planNeverCaptured": ["该项目方案尚未拍过 Codex 快照；已标记为当前项目且未改动任何配置，切走时会自动补拍。", "This project plan has never captured a Codex snapshot; it is marked as current with no configuration changed, and a snapshot will be captured automatically on switch-away."],
  "errors.cfg.planProviderGone": ["供应商 {target} 已不存在，已跳过供应商切换", "Provider {target} no longer exists; the provider switch was skipped"],
  "errors.cfg.planMcpGone": ["MCP {id} 已不存在，已跳过", "MCP {id} no longer exists; skipped"],
  "errors.cfg.planSkillGone": ["Skill {id} 已不存在，已跳过", "Skill {id} no longer exists; skipped"],
  "errors.cfg.planPromptGone": ["指令预设 {id} 已不存在，已跳过", "Prompt preset {id} no longer exists; skipped"],
  "errors.cfg.applyProviderSwitchFailed": ["供应商切换失败（{providerId}）：{detail}", "Provider switch failed ({providerId}): {detail}"],
  "errors.cfg.applyBindingEnableFailed": ["{kind} {label} 启用失败：{detail}", "{kind} {label} failed to enable: {detail}"],
  "errors.cfg.applyBindingDisableFailed": ["{kind} {label} 停用失败：{detail}", "{kind} {label} failed to disable: {detail}"],
  "errors.cfg.applyPromptActivateFailed": ["指令预设 {presetId} 激活失败：{detail}", "Prompt preset {presetId} failed to activate: {detail}"],
} as const satisfies Record<string, MessageEntry>;
