import type { MessageEntry } from "../types";

export const messages = {
  "storage.archive.configChanged": ["{app} 配置历史在导出后变化，变化后的记录未清理。", "{app} configuration history changed after export; the changed records were retained."],
  "storage.archive.configFailed": ["{app} 配置历史已完整归档，部分回收未完成：{detail}", "{app} configuration history was fully archived; reclamation is incomplete: {detail}"],
  "storage.title": ["存储与缓存", "Storage and cache"],
  "storage.description": ["查看应用数据占用，清理可重建缓存，或按日期归档旧配置历史。", "Inspect application storage, clear rebuildable cache, or archive old configuration history by date."],
  "storage.data": ["应用数据", "Application data"],
  "storage.backups": ["其中：配置恢复备份", "Configuration recovery backups, included above"],
  "storage.history": ["其中：可归档历史", "Archivable history, included above"],
  "storage.cache": ["浏览器磁盘缓存", "Browser disk cache"],
  "storage.refresh": ["刷新容量", "Refresh storage"],
  "storage.clearCache": ["清理浏览器缓存", "Clear browser cache"],
  "storage.cachePolicy": ["启动时及每 5 分钟检查缓存，超过 {budget} 时自动回收。仅清理网页与代码缓存，保留登录状态和偏好。", "Cache is checked at startup and every 5 minutes, and cleared above {budget}. Only cached resources are cleared; sign-in state and preferences are preserved."],
  "storage.cutoff": ["归档此日期之前的历史（UTC）", "Archive history before this date (UTC)"],
  "storage.archivePolicy": ["归档先完整导出，再回收匹配的旧记录与空闲数据库空间。当前计费月份、运行中的检测、最新配置恢复记录和本地文件备份保留。", "A complete archive is exported before matching older records and unused database space are reclaimed. Current-month billing, running probes, the latest configuration recovery record, and local file backups are retained."],
  "storage.previewArchive": ["预览历史归档", "Preview history archive"],
  "storage.archiveTitle": ["归档并回收旧历史", "Archive and reclaim older history"],
  "storage.archiveConfirm": ["选择归档位置并执行", "Choose archive location and continue"],
  "storage.archiveReview.head": ["将归档 {count} 条 ", "Archive {count} history records from before "],
  "storage.archiveReview.tail": [" 之前的历史记录。所选位置保存完整压缩归档；导出成功后，这些记录将移出应用内历史。", ". A complete compressed archive is saved to your selected location; after successful export, these records are removed from in-app history."],
  "storage.archiveProtected": ["恢复备份及当前计费周期保持可用。历史变化时会拒绝过期计划；回收失败的部分会单独报告。", "Recovery backups and the current billing period remain available. Changes to history invalidate the preview; incomplete reclamation is reported separately."],
  "storage.archived": ["已回收 {count} 条历史记录。完整归档：{path}", "Reclaimed {count} history records. Complete archive: {path}"],
} as const satisfies Record<string, MessageEntry>;
