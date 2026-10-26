import type { MessageEntry } from "../types";

/** Domain dictionary for operations (zh-CN / en-US pairs). Keys are prefixed with
 * "operations." and must never collide with another domain file. */
export const messages = {
  // Save-and-apply confirmation sheet.
  "operations.save.title": ["确认保存并应用", "Confirm save and apply"],
  "operations.save.writesTarget": ["将写入 {target}", "Writes to {target}"],
  "operations.save.previewMissing": ["无法生成供应商变更预览", "Cannot generate the provider change preview"],
  "operations.save.codexPreviewMissing": ["Codex 保存预览缺失，请重新保存", "The Codex save preview is missing; save again"],
  "operations.save.patchNoApply": ["此修改只允许保存供应商资料，不能应用客户端配置", "This change may only save provider details; it cannot apply the client configuration"],

  "operations.repairData.done": ["应用数据检查完成，修复 {count} 个文件", "Application data checked; repaired {count} files"],
  "operations.repairData.partial": ["已修复 {count} 个文件，部分项目仍需处理", "Repaired {count} files; some issues still need attention"],
  "operations.repairData.backup": ["原始数据保留在 {path}", "Original data retained at {path}"],

  // Delete sheet.
  "operations.delete.title": ["删除供应商", "Delete provider"],
  "operations.delete.confirm": ["确认删除", "Confirm deletion"],
  "operations.delete.line1": ["删除本地记录 {name}", "Deletes the local record {name}"],
  "operations.delete.line2": ["不会修改当前客户端配置。", "The current client configuration is not modified."],
  "operations.delete.missing": ["供应商已不存在，请重新读取", "The provider no longer exists; reload and try again"],

  // Undo sheet.
  "operations.undo.title": ["撤回上一次切换", "Undo the last switch"],
  "operations.undo.confirm": ["确认撤回", "Confirm undo"],
  "operations.undo.lastSwitch": ["{client} 上次切换到「{name}」", "{client} last switched to \"{name}\""],
  "operations.undo.lastRestore": ["{client} 上次操作是恢复备份", "{client}'s last operation was a backup restore"],
  "operations.undo.switchedAt": ["切换时间", "Switched at"],
  "operations.undo.restoreNote": ["将恢复该次切换前的备份；当前内容会先另行备份。", "Restores the backup from before that switch; the current content is backed up separately first."],
  "operations.undo.diffLoading": ["正在生成撤回后会写入的差异。", "Generating the diff the undo will write."],
  "operations.undo.diffEmpty": ["当前受管配置已与将恢复的备份一致。", "The current managed configuration already matches the backup to be restored."],
  "operations.undo.diffLabel": ["撤回后写入的差异", "Diff written by the undo"],
  "operations.undo.diffError": ["无法生成撤回差异", "Cannot generate the undo diff"],

  // Stale-lock recovery sheet.
  "operations.recoverLock.title": ["清理遗留锁", "Clear stale lock"],
  "operations.recoverLock.confirm": ["确认清理", "Confirm cleanup"],
  "operations.recoverLock.line1": ["{client} 的遗留写入锁将被删除。", "The stale write lock of {client} will be removed."],
  "operations.recoverLock.line2": ["仅在确认该客户端没有正在进行的切换时继续。", "Continue only after confirming the client has no switch in progress."],

  // Outcome notifications.
  "operations.notify.switched": ["已切换到「{name}」", "Switched to \"{name}\""],
  "operations.notify.restored": ["已恢复备份", "Backup restored"],
  "operations.notify.undone": ["已撤回上一次切换", "Last switch undone"],
  "operations.notify.warningsSuffix": ["，有 {count} 条警告", " ({count} warning(s))"],
  "operations.notify.readAtLaunch": ["{client}将在下次启动时读取新配置", "{client} will read the new configuration the next time it starts"],
  "operations.notify.discoveryStale.written": ["配置已写入，但无法刷新本机配置发现结果。", "The configuration was written, but local configuration discovery could not be refreshed."],
  "operations.notify.discoveryStale.restored": ["配置已恢复，但无法刷新本机配置发现结果。", "The configuration was restored, but local configuration discovery could not be refreshed."],
  "operations.notify.discoveryStale.undone": ["配置已撤回，但无法刷新本机配置发现结果。", "The configuration was undone, but local configuration discovery could not be refreshed."],
} as const satisfies Record<string, MessageEntry>;
