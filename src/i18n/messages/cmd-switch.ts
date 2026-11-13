import type { MessageEntry } from "../types";

/** Backend command errors translated on the renderer side. Keys are emitted
 * by desktop commands via `CommandError::keyed/localized` (see the matching
 * src-tauri area); `message` stays the scrubbed raw diagnostic fallback. */
export const messages = {
	// switching/mod.rs
	"errors.sw.preparationStateUninitialized": [
		"供应商保存准备状态尚未初始化",
		"The provider save preparation state has not been initialized.",
	],
	"errors.sw.codexPreparationStateUninitialized": [
		"Codex 供应商保存准备状态尚未初始化",
		"The Codex provider save preparation state has not been initialized.",
	],
	"errors.sw.writeGateUninitialized": [
		"写入闸门尚未初始化",
		"The write gate has not been initialized.",
	],
	"errors.sw.noUndoRecord": [
		"该客户端没有可撤回的切换记录",
		"This client has no switch record that can be undone.",
	],
	"errors.sw.undoGatewayPortUnsupported": [
		"网关端口修改涉及全部客户端与监听器，请在网关页修改端口，不能撤回单个客户端配置",
		"A gateway port change involves all clients and listeners; change the port on the gateway page instead. A single client configuration cannot be undone.",
	],
	"errors.sw.currentConfigUnreadableDiffUnavailable": [
		"无法读取当前配置文件，无法生成差异",
		"The current configuration file cannot be read, so no diff can be generated.",
	],
	"errors.sw.backupUnreadableDiffUnavailable": [
		"备份文件不可读，无法生成差异",
		"The backup file cannot be read, so no diff can be generated.",
	],
	"errors.sw.backupHashMismatchDiffRefused": [
		"备份内容与记录哈希不符，拒绝生成差异",
		"The backup content does not match its recorded hash; diff generation is refused.",
	],
	"errors.sw.backupDirCreateFailed": [
		"无法创建备份目录",
		"The backup directory cannot be created.",
	],
	"errors.sw.backupDirOpenFailed": [
		"无法打开备份文件夹",
		"The backup folder cannot be opened.",
	],

	// switching/plan.rs
	"errors.sw.codexPreviewStale": [
		"Codex 配置预览已失效，请重新查看差异",
		"The Codex configuration preview is stale; please review the diff again.",
	],
	"errors.sw.codexConfigUnreadableForCatalogCleanup": [
		"无法读取 Codex 配置以清理模型目录",
		"The Codex configuration cannot be read to clean up the model catalog.",
	],
	"errors.sw.codexConfigUnparseable": [
		"Codex 配置无法解析",
		"The Codex configuration cannot be parsed.",
	],
	"errors.sw.codexConfigDirInvalid": [
		"Codex 配置目录无效",
		"The Codex configuration directory is invalid.",
	],
	"errors.sw.ownedCodexCatalogUnreadable": [
		"无法读取 ASB 管理的 Codex 模型目录",
		"The ASB-managed Codex model catalog cannot be read.",
	],
	"errors.sw.codexCatalogFileUnreadable": [
		"无法读取 Codex 模型目录文件",
		"The Codex model catalog file cannot be read.",
	],
	"errors.sw.codexCatalogRevisionConflict": [
		"同一 Codex 路由修订的模型目录内容不一致，拒绝覆盖",
		"The model catalog content is inconsistent within the same Codex route revision; overwriting is refused.",
	],

	// switching/profile_save.rs
	"errors.sw.preparationStateUnavailable": [
		"供应商保存准备状态不可用",
		"The provider save preparation state is unavailable.",
	],
	"errors.sw.savePreviewStale": [
		"保存预览已失效，请重新保存并查看最新差异",
		"The save preview is stale; please save again and review the latest diff.",
	],
	"errors.sw.savePreviewExpired": [
		"保存预览已过期，请重新保存并查看最新差异",
		"The save preview has expired; please save again and review the latest diff.",
	],
	"errors.sw.saveCommitLockUnavailable": [
		"供应商保存事务锁不可用",
		"The provider save transaction lock is unavailable.",
	],
	"errors.sw.profileRolledBackClearFailed": [
		"{detail}；供应商档案已回滚，但{error}",
		"{detail}; the provider profile was rolled back, but {error}",
	],
	"errors.sw.profileRollbackFailedRecoveryKept": [
		"{detail}；供应商档案未能回滚：{error}；已保留恢复记录以继续完成已确认的保存",
		"{detail}; the provider profile could not be rolled back: {error}. Recovery records were kept to finish the confirmed save.",
	],
	"errors.sw.saveContextChanged": [
		"供应商、客户端设置或当前客户端配置已变更，请重新保存并查看最新差异",
		"The provider, client settings, or current client configuration has changed; please save again and review the latest diff.",
	],
	"errors.sw.savePreviewMissing": [
		"保存预览缺失，请重新保存",
		"The save preview is missing; please save again.",
	],

	// switching/codex_profile_save.rs
	"errors.sw.codexCreateNoActiveSave": [
		"Codex 新建供应商不使用活跃保存事务",
		"New Codex providers do not use the active save transaction.",
	],
	"errors.sw.codexProfileNotFound": [
		"Codex 供应商不存在",
		"The Codex provider does not exist.",
	],
	"errors.sw.codexProfileFileModified": [
		"Codex 供应商文件已被外部修改，请重新读取后再保存",
		"The Codex provider file was modified externally; please re-read it before saving.",
	],
	"errors.sw.codexProfileRolledBackClearFailed": [
		"{detail}；Codex 供应商档案已回滚，但{error}",
		"{detail}; the Codex provider profile was rolled back, but {error}",
	],
	"errors.sw.codexProfileRollbackFailedRecoveryKept": [
		"{detail}；Codex 供应商未能回滚：{error}；已保留恢复记录以继续完成已确认的保存",
		"{detail}; the Codex provider could not be rolled back: {error}. Recovery records were kept to finish the confirmed save.",
	],
	"errors.sw.codexPreparationStateUnavailable": [
		"Codex 供应商保存准备状态不可用",
		"The Codex provider save preparation state is unavailable.",
	],
	"errors.sw.codexSavePreviewStale": [
		"Codex 保存预览已失效，请重新保存并查看最新差异",
		"The Codex save preview is stale; please save again and review the latest diff.",
	],

	// switching/backups.rs
	"errors.sw.resetPrecheckFailed": [
		"当前配置无法通过备份恢复校验，尚未重置：{message}",
		"The current configuration failed the backup restore validation and was not reset: {message}",
	],
	"errors.sw.backupNotFound": [
		"找不到指定备份",
		"The specified backup cannot be found.",
	],
	"errors.sw.restoreGatewayPortUnsupported": [
		"网关端口修改涉及全部客户端与监听器，请在网关页修改端口，不能恢复单个客户端配置",
		"A gateway port change involves all clients and listeners; change the port on the gateway page instead. A single client configuration cannot be restored.",
	],
	"errors.sw.backupTargetInvalid": [
		"备份不属于当前本机配置路径，已拒绝恢复",
		"The backup does not belong to the current local configuration path; the restore was refused.",
	],
	"errors.sw.legacyAuthBackupRetired": [
		"旧版认证联动备份只能查看或导出，当前切换器不会恢复登录缓存",
		"Legacy auth-linked backups can only be viewed or exported; the current switcher never restores login caches.",
	],
	"errors.sw.backupUnreadable": ["无法读取备份", "The backup cannot be read."],
	"errors.sw.backupHashMismatch": [
		"备份内容与记录不一致",
		"The backup content does not match its record.",
	],

	// switching/claude_gateway.rs
	"errors.sw.claudeConfigUnreadable": [
		"无法读取 Claude 配置：{error}",
		"The Claude configuration cannot be read: {error}",
	],
	"errors.sw.claudeNotOnTakeoverRoute": [
		"当前 Claude 配置已不属于活动接管路由；未修改任何文件，请先重新应用供应商或在切换历史中恢复",
		"The current Claude configuration no longer belongs to the active takeover route; no file was modified. Re-apply the provider or restore from the switch history first.",
	],
	"errors.sw.preTakeoverBackupUnreadable": [
		"接管前备份不可读：{error}",
		"The pre-takeover backup cannot be read: {error}",
	],
	"errors.sw.preTakeoverBackupHashMismatch": [
		"接管前备份校验失败，已拒绝恢复",
		"The pre-takeover backup failed verification; the restore was refused.",
	],
	"errors.sw.preTakeoverBackupMissing": [
		"找不到接管前的 Claude 直连/官方配置备份；请在供应商页明确切换到官方登录或直连供应商",
		"The pre-takeover Claude direct/official configuration backup cannot be found; explicitly switch to official login or a direct provider on the providers page.",
	],
	"errors.sw.claudePreviewChanged": [
		"Claude 配置或备份在预览后改变，请重新预览",
		"The Claude configuration or backup changed after the preview; please preview again.",
	],

	// switching/codex_restore_auth.rs
	"errors.sw.codexBackupMultipleAuthSources": [
		"Codex 配置备份关联多个认证来源，已拒绝恢复",
		"The Codex configuration backup is linked to multiple auth sources; the restore was refused.",
	],
	"errors.sw.codexAuthBackupTargetMismatch": [
		"Codex 认证备份目标或版本不匹配",
		"The Codex auth backup target or revision does not match.",
	],
	"errors.sw.codexAuthBackupUnreadable": [
		"Codex 认证备份不可读",
		"The Codex auth backup cannot be read.",
	],
	"errors.sw.codexAuthBackupHashMismatch": [
		"Codex 认证备份哈希不匹配",
		"The Codex auth backup hash does not match.",
	],
	"errors.sw.codexCurrentAuthUnreadable": [
		"Codex 当前认证文件不可读",
		"The current Codex auth file cannot be read.",
	],
	"errors.sw.codexBackupConfigInvalid": [
		"Codex 备份配置格式无效",
		"The Codex backup configuration format is invalid.",
	],
	"errors.sw.directBackupMissingApiKey": [
		"此直连备份没有配套的 API-key 认证，恢复可能误用官方登录；请重新应用目标 Codex 供应商",
		"This direct-connection backup has no accompanying API-key auth; the restore could mistakenly use official login. Please re-apply the target Codex provider.",
	],

	// switching/codex_backfill.rs
	"errors.sw.codexLiveReadFailed": [
		"无法读取当前 Codex 配置，已取消切换以保护供应商档案",
		"The current Codex configuration cannot be read; the switch was cancelled to protect the provider store.",
	],
	"errors.sw.codexLiveChanged": [
		"当前 Codex 配置已变化，请重新预览后再切换",
		"The current Codex configuration has changed; please preview again before switching.",
	],
	"errors.sw.codexLiveProfileMissing": [
		"当前 Codex 网关路由找不到对应供应商档案",
		"No provider profile matches the current Codex gateway route.",
	],
	"errors.sw.codexBackfillFailed": [
		"无法回填 Codex 供应商：{error}",
		"The Codex provider cannot be backfilled: {error}",
	],
	"errors.sw.codexBackfillRevisionFailed": [
		"无法计算 Codex 供应商回填版本：{error}",
		"The Codex provider backfill revision cannot be computed: {error}",
	],
	"errors.sw.codexBackfillRevisionMismatch": [
		"Codex 供应商回填版本与 durable 事务记录不一致",
		"The Codex provider backfill revision does not match the durable transaction record.",
	],

	// switching/projection_transaction.rs
	"errors.sw.codexAuthPreviewIncomplete": [
		"Codex 认证预览字段不完整，请重新查看差异",
		"The Codex auth preview fields are incomplete; please review the diff again.",
	],
	"errors.sw.codexBackfillRecoveryFailed": [
		"{detail}；Codex 回填未能恢复：{recovery}",
		"{detail}; the Codex backfill failed to recover: {recovery}",
	],

	// switching/codex_policy/preparations.rs
	"errors.sw.codexPolicyPreviewStale": [
		"Codex 网关预览已失效，请重新预览",
		"The Codex gateway preview is stale; please preview again.",
	],
	"errors.sw.codexPolicyPreviewUnavailable": [
		"Codex 网关预览暂不可用",
		"The Codex gateway preview is temporarily unavailable.",
	],
	"errors.sw.failoverRequiresQueueHead": [
		"开启故障转移必须先通过事务激活队列首项",
		"Enabling failover requires activating the first queue item through a transaction first.",
	],
	"errors.sw.policyContextChanged": [
		"供应商、网关策略或配置来源已变化，请重新预览",
		"The provider, gateway policy, or configuration source has changed; please preview again.",
	],

	// switching/codex_policy/mod.rs
	"errors.sw.clientRecoveryFailed": [
		"{detail}；客户端事务恢复失败：{recovery}",
		"{detail}; the client transaction recovery failed: {recovery}",
	],
	"errors.sw.policyRecoveryFailed": [
		"{detail}；策略恢复失败：{recovery}",
		"{detail}; the policy recovery failed: {recovery}",
	],

	// commands/profiles.rs
	"errors.sw.codexInFailoverQueue": [
		"请先从 Codex 故障转移队列移除该供应商，再删除档案",
		"Remove this provider from the Codex failover queue before deleting the profile.",
	],
	"errors.sw.codexReferencedBySubagentRoute": [
		"该 Codex 供应商正被其他档案的子代理路由引用；请先移除引用后再删除档案",
		"This Codex provider is referenced by another profile's subagent route; remove the reference before deleting the profile.",
	],
	"errors.sw.codexDeleteCheckUnreadable": [
		"无法读取 Codex 配置以确认档案未被引用",
		"The Codex configuration cannot be read to confirm the profile is unreferenced.",
	],
	"errors.sw.codexDeleteCheckUnparseable": [
		"Codex 配置格式无效，无法确认档案是否仍被引用",
		"The Codex configuration format is invalid; it cannot be confirmed whether the profile is still referenced.",
	],
	"errors.sw.codexStillReferenced": [
		"该 Codex 供应商仍被当前客户端配置引用；请先切换后再删除",
		"This Codex provider is still referenced by the current client configuration; switch away before deleting.",
	],
	"errors.sw.codexGatewayActiveDelete": [
		"该 Codex 供应商正在被本机协议网关使用；请先切换到官方登录后再删除",
		"This Codex provider is in use by the local protocol gateway; switch to official login before deleting.",
	],
	"errors.sw.gatewayActiveDelete": [
		"该供应商正在被本机协议网关使用；请先切换到直连或官方登录后再删除",
		"This provider is in use by the local protocol gateway; switch to direct connection or official login before deleting.",
	],
	"errors.sw.gatewayActiveImport": [
		"该客户端正在使用本机协议网关；请先切换到直连或官方登录后再导入供应商",
		"This client is using the local protocol gateway; switch to direct connection or official login before importing providers.",
	],
	"errors.sw.noImportableProvider": [
		"当前配置没有可导入的供应商",
		"The current configuration has no importable providers.",
	],
	"errors.sw.codexGatewayActiveImport": [
		"Codex 当前正在使用本机协议网关；请先切换到官方登录或直连后再导入供应商",
		"Codex is currently using the local protocol gateway; switch to official login or direct connection before importing providers.",
	],

	// switching/transaction.rs
	"errors.sw.configCompensationIncomplete": [
		"{failure}；配置补偿未完成，保留事务和备份",
		"{failure}; configuration compensation did not complete; the transaction and backups are preserved.",
	],
	"errors.sw.transactionTargetClientMismatch": [
		"事务目标不属于当前客户端",
		"The transaction target does not belong to the current client.",
	],
	"errors.sw.saveTransactionMismatch": [
		"供应商保存与配置事务不匹配",
		"The provider save does not match the configuration transaction.",
	],

	// switching/transaction/journal.rs
	"errors.sw.intentEncodeFailed": [
		"配置事务意图无法编码",
		"The configuration transaction intent cannot be encoded.",
	],

	// switching/transaction/intent.rs
	"errors.sw.pendingTransactionExists": [
		"存在未完成配置事务，请先恢复",
		"An unfinished configuration transaction exists; recover it first.",
	],
	"errors.sw.codexAuthChangedAfterPreview": [
		"Codex 认证文件在预览后已发生变化，请重新查看差异",
		"The Codex auth file changed after the preview; please review the diff again.",
	],

	// switching/transaction/catalog.rs
	"errors.sw.ownedCodexCatalogRemoveFailed": [
		"无法清理 ASB 管理的 Codex 模型目录文件",
		"The ASB-managed Codex model catalog file cannot be removed.",
	],
} as const satisfies Record<string, MessageEntry>;
