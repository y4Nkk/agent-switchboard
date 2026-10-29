import { invoke } from "./client";
import type { AppKind, RouteMode, CodexModelSettings } from "./shared";

/** One backend-produced warning with renderer translation coordinates; `text`
 * is the scrubbed product-language rendering used as detail and fallback. */
export interface LocalizedMessage {
  key: string;
  params?: Record<string, string | number>;
  text: string;
}

export interface RouteState {
  app: AppKind;
  routeMode: RouteMode;
  providerName: string | null;
  model: string | null;
  baseUrl: string | null;
  apiKey: string;
  wireApi: string | null;
  codexModelOptions: CodexModelSettings | null;
  haikuModel: string | null;
  sonnetModel: string | null;
  opusModel: string | null;
  fableModel: string | null;
  availableModels: string[] | null;
  scopeWarnings: LocalizedMessage[];
}

export type WriteOperation = "projection" | "gatewayPortChange" | "restore";

export interface ConfigWriteRecord {
  app: AppKind;
  profileId: string | null;
  profileName: string | null;
  contentHash: string;
  backupId: string;
  at: string;
  operation: WriteOperation;
}

export type LockStatus =
  | { state: "free" }
  | { state: "held"; pid?: number | null; processName?: string | null; acquiredAt?: string | null }
  | { state: "stale"; pid?: number | null; processName?: string | null; acquiredAt?: string | null }
  | { state: "indeterminate"; reason: string };

export interface KeyChange {
  key: string;
  kind: "set" | "remove";
  before: string | null;
  after: string | null;
}

export interface SwitchPreview {
  app: AppKind;
  target: string;
  changes: KeyChange[];
  warnings: LocalizedMessage[];
  backupDir: string;
}

export interface FilePreview {
  preview: SwitchPreview;
  contentHash: string;
  renderedHash: string;
  /** Redacted candidate file text for the pretty-printed file view. */
  content: string;
  authHash?: string;
  authRenderedHash?: string;
  authExisted?: boolean;
  authRenderedExisted?: boolean;
}

export interface BackupRecord {
  id: string;
  app: AppKind;
  targetPath: string;
  backupPath: string;
  createdAt: string;
  contentHash: string;
  targetExisted: boolean;
  linkedBackupId: string | null;
  reason: string;
}

export type RecoveryOutcome =
  | { outcome: "not_needed" }
  | { outcome: "restored"; backup: BackupRecord }
  | { outcome: "restore_failed"; reason: string; backupPath: string };

export interface SwitchOutcome {
  lock: LockStatus;
  acquiredAt: string;
  changed: string[];
  warnings: LocalizedMessage[];
  backup: BackupRecord;
  preview: SwitchPreview;
  recovery: RecoveryOutcome;
  finalHash: string;
}

export interface RestoreOutcome {
  preRestoreBackup: BackupRecord;
  restoredHash: string;
  warnings: LocalizedMessage[];
}

export interface RecoveryEntry {
  lockPath: string;
  removedHolderPid: number | null;
  at: string;
}

export function previewSwitch(profileId: string): Promise<FilePreview> {
  return invoke<FilePreview>("preview_switch", { profileId });
}

export function executeSwitch(
  profileId: string,
  expectedHash: string,
  expectedRenderedHash: string,
  confirmWrite: boolean,
  auth?: Pick<FilePreview, "authHash" | "authRenderedHash" | "authExisted">,
): Promise<SwitchOutcome> {
  const request = {
    profileId,
    expectedHash,
    expectedRenderedHash,
    confirmWrite,
    ...(auth?.authHash && auth.authRenderedHash
      ? {
          authHash: auth.authHash,
          authExisted: auth.authExisted ?? false,
          authRenderedHash: auth.authRenderedHash,
        }
      : {}),
  };
  return invoke<SwitchOutcome>("execute_switch", request);
}

export function listBackups(): Promise<BackupRecord[]> {
  return invoke<BackupRecord[]>("list_backups");
}

export function restoreBackup(backupId: string, confirmWrite: boolean): Promise<RestoreOutcome> {
  return invoke<RestoreOutcome>("restore_backup", { backupId, confirmWrite });
}

export function undoLastSwitch(target: AppKind, confirmWrite: boolean): Promise<RestoreOutcome> {
  return invoke<RestoreOutcome>("undo_last_switch", { target, confirmWrite });
}

export function backupDiff(backupId: string): Promise<KeyChange[]> {
  return invoke<KeyChange[]>("backup_diff", { backupId });
}

export function openBackupDir(): Promise<void> {
  return invoke<void>("open_backup_dir");
}

export function getLockStatus(app: AppKind): Promise<LockStatus> {
  return invoke<LockStatus>("lock_status", { target: app });
}

export function recoverStaleLock(app: AppKind): Promise<RecoveryEntry> {
  return invoke<RecoveryEntry>("recover_stale_lock", { target: app });
}
