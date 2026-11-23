import { useState } from "react";
import { createRoot } from "react-dom/client";
import type { ApplicationPreparation } from "../src/api/application-preparation";
import type { BackupRecord } from "../src/api/switching";
import { useConfigurationModel } from "../src/app/useConfigurationModel";
import { BackupHistory } from "../src/components/BackupHistory";
import { applyLanguagePreference } from "../src/i18n";

const transaction: BackupRecord = {
  id: "pending-backup", app: "codex", targetPath: "fixture/config.toml", backupPath: "fixture/pending.toml",
  createdAt: "2026-11-22T00:00:00Z", contentHash: "a".repeat(64),
  targetExisted: true, linkedBackupId: null, reason: "provider-projection",
};
const history: BackupRecord = { ...transaction, id: "older-backup", backupPath: "fixture/older.toml",
  contentHash: "b".repeat(64) };
const base: ApplicationPreparation = {
  sessionId: "backup-ui-fixture", runId: 1, revision: 1, status: "degraded", stage: "complete",
  elapsedMs: 0, settings: null, configurationReady: false, recoveryBackupId: transaction.id,
  preferencesReset: false, repairedFiles: [], backupPaths: [], issues: [],
};
const calls: Record<string, unknown>[] = [];
let failure: { code: string; message: string } | null = null;
let callbacks = 0;
const unexpected: string[] = [];
const mock = {
  transformCallback: () => ++callbacks,
  invoke: async (command: string, args?: Record<string, unknown>) => {
    switch (command) {
      case "plugin:event|listen": return callbacks;
      case "config_status":
      case "list_profiles":
      case "list_codex_profiles":
      case "backup_diff": return [];
      case "list_backups": return [history, transaction];
      case "lock_status": return { state: "free" };
      case "codex_login_blocker": return null;
      case "discover_local": return {};
      case "restore_backup":
        calls.push(args ?? {});
        if (failure) throw failure;
        return { preRestoreBackup: transaction, restoredHash: transaction.contentHash, warnings: [] };
      default: throw new Error(`Unexpected command ${command}`);
    }
  },
};
Object.assign(window, { __TAURI_INTERNALS__: mock });
const noop = () => {};
const onError = (error: { message: string }) => { unexpected.push(error.message); };

function Fixture() {
  const [busy, setBusy] = useState(false);
  const [preparation, setPreparation] = useState(base);
  const { snapshot, operations } = useConfigurationModel({
    operationContext: { busy, onError, clearError: noop, setBusy },
    appFilter: "codex", setAppFilter: noop, page: "settings", setPage: noop, setProviderView: noop,
    workspaceVersion: () => 0, preparation, dataRevision: preparation.revision,
  });
  Object.assign(window, { backupRecoveryFixture: {
    calls, unexpected, setFailure: (next: typeof failure) => { failure = next; },
    setReady: (ready: boolean) => setPreparation((previous) => ({
      ...base, revision: previous.revision + 1, configurationReady: ready,
      recoveryBackupId: ready ? null : transaction.id, status: ready ? "ready" : "degraded",
    })),
  } });
  return <BackupHistory records={snapshot.backups} busy={busy} onRestore={operations.runRestore}
    configurationReady={preparation.configurationReady} recoveryBackupId={preparation.recoveryBackupId} />;
}

applyLanguagePreference("en-US");
createRoot(document.getElementById("root")!).render(<Fixture />);
