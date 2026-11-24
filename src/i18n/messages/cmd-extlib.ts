import type { MessageEntry } from "../types";

/** Backend command errors translated on the renderer side. Keys are emitted
 * by desktop commands via `CommandError::keyed/localized` (see the matching
 * src-tauri area); `message` stays the scrubbed raw diagnostic fallback. */
export const messages = {
  // Extension identity / lookup
  "errors.extlib.extensionNotFound": ["扩展不存在或已被删除", "The extension does not exist or was deleted"],
  "errors.extlib.extensionDefinitionMissing": ["扩展定义不存在", "The extension definition does not exist"],
  "errors.extlib.bindingNotFound": ["绑定不存在或已被移除", "The binding does not exist or was removed"],
  "errors.extlib.notASkill": ["该扩展不是 Skill", "This extension is not a Skill"],
  "errors.extlib.notASkillDefinition": ["该扩展不是 Skill 定义", "This extension is not a Skill definition"],
  "errors.extlib.notAnMcpDefinition": ["该扩展不是 MCP 定义", "This extension is not an MCP definition"],

  // Skill update / fork / local authoring
  "errors.extlib.updateCheckOnlySkill": ["更新检查只适用于 Skill", "Update checks apply only to Skills"],
  "errors.extlib.updateCheckNoSelection": ["更新检查没有选择任何 Skill", "No Skill was selected for the update check"],
  "errors.extlib.updateOnlySkill": ["更新只适用于 Skill", "Updates apply only to Skills"],
  "errors.extlib.updateCandidateExpired": ["更新候选已过期，请重新检查更新", "The update candidate expired. Check for updates again."],
  "errors.extlib.candidateExpiredRescan": ["候选内容已过期；请重新扫描来源", "The candidate content expired; scan the source again"],
  "errors.extlib.candidateSourceConflict": ["候选内容来自不同来源；未更新当前 Skill 的来源绑定", "The candidate content comes from a different source; the current Skill's source binding was not updated"],
  "errors.extlib.notSkillCannotFork": ["该扩展不是 Skill，无法创建本地副本", "This extension is not a Skill and cannot be forked locally"],
  "errors.extlib.forkMissingCommonFields": ["该内容缺少通用字段，无法创建本地副本：{detail}", "This content is missing required common fields and cannot be forked locally: {detail}"],
  "errors.extlib.alreadyLocalEditable": ["该 Skill 已是本地内容，可直接编辑", "This Skill is already local content and can be edited directly"],
  "errors.extlib.localSkillRequiresDescription": ["通用 Skill 必须提供 description", "A generic Skill must provide a description"],
  "errors.extlib.templateSkillMdUnparsable": ["模板生成的 SKILL.md 无法解析，请重试", "The generated SKILL.md could not be parsed; please retry"],

  // Skill file editor
  "errors.extlib.skillConflictRefreshEditor": ["该 Skill 已被其他窗口修改；请刷新编辑器后重试", "This Skill was modified in another window; refresh the editor and retry"],
  "errors.extlib.skillConflictRefreshRetry": ["该 Skill 已被其他窗口修改；请刷新后重试", "This Skill was modified in another window; refresh and retry"],
  "errors.extlib.scopedSkillNotEditable": ["带来源或宿主限定的 Skill 不能直接编辑；请先创建本地副本", "A Skill with a source or host scope cannot be edited directly; create a local copy first"],
  "errors.extlib.contentVersionChangedReload": ["内容版本已变化；请重新加载编辑器", "The content version changed; reload the editor"],
  "errors.extlib.filePathEmpty": ["文件路径不能为空", "The file path must not be empty"],
  "errors.extlib.duplicateFilePath": ["保存请求包含重复的文件路径", "The save request contains duplicate file paths"],
  "errors.extlib.savedContentMissingSkillMd": ["保存后的内容缺少 SKILL.md", "The saved content is missing SKILL.md"],
  "errors.extlib.skillMdFrontmatterUnparsable": ["SKILL.md 的 frontmatter 无法解析；需要 name 字段和闭合的 --- 块", "The SKILL.md frontmatter could not be parsed; a name field and a closed --- block are required"],
  "errors.extlib.skillContentInvalid": ["Skill 内容无效：{detail}", "The Skill content is invalid: {detail}"],

  // Skill dependencies
  "errors.extlib.dependencyMissingResourceId": ["依赖 {name} 缺少资源标识", "Dependency {name} is missing a resource id"],
  "errors.extlib.dependencyNotFound": ["依赖 {name} 不存在", "Dependency {name} does not exist"],
  "errors.extlib.dependencyMustBeMcp": ["依赖 {name} 必须指向 MCP 定义", "Dependency {name} must reference an MCP definition"],

  // Source candidates (scan / import)
  "errors.extlib.candidateMissingSkillMd": ["候选内容缺少 UTF-8 格式的 SKILL.md", "The candidate content is missing a UTF-8 SKILL.md"],
  "errors.extlib.candidateSkillMdNoFrontmatter": ["候选内容缺少可解析的 SKILL.md frontmatter", "The candidate content lacks a parsable SKILL.md frontmatter"],
  "errors.extlib.sourceUnreachable": ["来源不可达；请检查网络、地址和访问权限", "The source is unreachable; check the network, address, and access permissions"],
  "errors.extlib.sourceRejected": ["来源内容未通过完整性或安全校验", "The source content failed the integrity or security checks"],

  // Discovery observations
  "errors.extlib.observationExpired": ["发现结果已过期；请重新扫描本机扩展", "The discovery result expired; scan this machine's extensions again"],
  "errors.extlib.observationNotSkillImport": ["该发现结果不是 Skill，不能按 Skill 导入", "This discovery result is not a Skill and cannot be imported as one"],
  "errors.extlib.observationNotSkillTakeover": ["该发现结果不是 Skill，不能接管", "This discovery result is not a Skill and cannot be taken over"],
  "errors.extlib.observationNotMcpImport": ["该发现结果不是 MCP 服务，不能按 MCP 导入", "This discovery result is not an MCP server and cannot be imported as one"],
  "errors.extlib.observationNotMcpTakeover": ["该发现结果不是 MCP 服务，不能接管", "This discovery result is not an MCP server and cannot be taken over"],
  "errors.extlib.skillNoDigestImport": ["该 Skill 没有可验证的内容摘要，不能安全导入", "This Skill has no verifiable content digest and cannot be imported safely"],
  "errors.extlib.skillNoDigestTakeover": ["该 Skill 没有可验证的内容摘要，不能安全接管", "This Skill has no verifiable content digest and cannot be taken over safely"],
  "errors.extlib.discoveredSkillPathInvalid": ["发现到的 Skill 路径无效", "The discovered Skill path is invalid"],
  "errors.extlib.skillChangedSinceScan": ["该 Skill 内容已在扫描后变化；请重新扫描并确认", "The Skill content changed after the scan; rescan and confirm"],

  // MCP document staleness / takeover
  "errors.extlib.mcpDocumentUnreadable": ["发现到的 MCP 配置已无法读取；请重新扫描并确认", "The discovered MCP configuration can no longer be read; rescan and confirm"],
  "errors.extlib.mcpChangedSinceScan": ["该 MCP 配置已在扫描后变化；请重新扫描并确认", "The MCP configuration changed after the scan; rescan and confirm"],
  "errors.extlib.mcpNotTextImport": ["MCP 配置不是有效文本，不能安全导入", "The MCP configuration is not valid text and cannot be imported safely"],
  "errors.extlib.mcpNotTextTakeover": ["MCP 配置不是有效文本，不能安全接管", "The MCP configuration is not valid text and cannot be taken over safely"],
  "errors.extlib.mcpAlreadyManagedImport": ["该 MCP 服务已由扩展库管理，无需再次导入", "This MCP server is already managed by the extension library; no import is needed"],
  "errors.extlib.mcpAlreadyManagedTakeover": ["该 MCP 服务已由扩展库管理，无需接管", "This MCP server is already managed by the extension library; no takeover is needed"],
  "errors.extlib.skillAlreadyManagedTakeover": ["该 Skill 已由扩展库管理，无需接管", "This Skill is already managed by the extension library; no takeover is needed"],
  "errors.extlib.nativeMcpEntryGone": ["原生 MCP 条目已不存在或为空；请重新扫描并确认", "The native MCP entry no longer exists or is empty; rescan and confirm"],
  "errors.extlib.alreadyBoundNoTakeover": ["该定义已绑定到所选目标，不能重复接管", "This definition is already bound to the selected target; it cannot be taken over again"],
  "errors.extlib.targetManagedNoTakeover": ["该目标目录已被另一绑定管理，不能重复接管", "The target directory is already managed by another binding; it cannot be taken over again"],
  "errors.extlib.takeoverDefinitionNotPersisted": ["接管定义未持久化", "The taken-over definition was not persisted"],
  "errors.extlib.projectNotRegisteredTakeover": ["该项目目录尚未注册；请先在扩展页注册项目后接管", "This project directory is not registered; register the project on the extensions page before taking over"],
  "errors.extlib.readOnlyOriginNoTakeover": ["该来源（{detail}）只读，不能接管", "This origin ({detail}) is read-only and cannot be taken over"],

  // Definition lifecycle / project registration
  "errors.extlib.extensionConflictRefresh": ["扩展已被其他窗口修改；请刷新后重试", "The extension was modified in another window; refresh and retry"],
  "errors.extlib.projectRootUnresolvable": ["项目目录无法解析：{detail}", "The project directory could not be resolved: {detail}"],
  "errors.extlib.projectPathNotDirectory": ["项目路径不是目录", "The project path is not a directory"],

  // Binding version pinning
  "errors.extlib.bindingLockOnlySkill": ["版本固定只适用于 Skill 绑定", "Version pinning applies only to Skill bindings"],
  "errors.extlib.bindingLockRequiresSync": ["只有内容与当前版本一致的目标才能固定；请先更新部署或恢复同步", "Only targets whose content matches the current version can be pinned; update the deployment or restore sync first"],

  // Portable packages
  "errors.extlib.exportTargetIsDirectory": ["导出目标是一个目录，请提供文件路径", "The export target is a directory; provide a file path"],
  "errors.extlib.exportParentMissing": ["导出目标的父目录不存在", "The export target's parent directory does not exist"],
  "errors.extlib.portableSerializeFailed": ["便携包序列化失败：{detail}", "Serializing the portable package failed: {detail}"],
  "errors.extlib.portableWriteFailed": ["无法写入便携包：{detail}", "Writing the portable package failed: {detail}"],
  "errors.extlib.portableUnreadable": ["便携包文件无法读取", "The portable package file could not be read"],
  "errors.extlib.portableInvalidFormat": ["便携包不是有效格式：{detail}", "The portable package is not a valid format: {detail}"],

  // Secrets / credentials
  "errors.extlib.secretValueEmpty": ["凭据值不能为空", "The credential value must not be empty"],
  "errors.extlib.secretUnavailableRepreview": ["扩展所需凭据当前不可用；请在系统凭据存储中补充后重新预览", "A credential required by the extension is currently unavailable; add it in the system credential store and preview again"],
  "errors.extlib.planRolledBack": ["扩展变更已回滚，客户端文件保持原状", "The extension changes were rolled back; the client file is untouched"],
} as const satisfies Record<string, MessageEntry>;
