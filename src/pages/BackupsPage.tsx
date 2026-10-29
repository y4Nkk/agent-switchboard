import { useEffect, useState } from "react";
import type { BackupRecord, ConfigWriteRecord } from "../api/client";
import type { useCloudBackup } from "../app/useCloudBackup";
import { BackupCleanup } from "../components/BackupCleanup";
import { BackupHistory } from "../components/BackupHistory";
import { Button } from "../components/Button";
import { CloudBackupPanel } from "../components/CloudBackupPanel";
import { ModuleHeader } from "../components/WorkspaceHeader";
import { Tabs } from "../components/Tabs";
import { Time } from "../components/Time";
import { useI18n } from "../i18n";
import { clientName } from "../lib/client-name";

interface BackupsPageProps {
  records: BackupRecord[];
  busy: boolean;
  active: boolean;
  recoveryRequest: number;
  lastSwitch: ConfigWriteRecord | null;
  cloudBackup: ReturnType<typeof useCloudBackup>;
  onRestore: (backupId: string) => void | Promise<void>;
  onUndo: (lastSwitch: ConfigWriteRecord) => void;
  onOpenDir: () => void;
  /** Refreshes the record list after a bulk cleanup reclaimed backups. */
  onCleaned: () => void;
}

type BackupTab = "local" | "cloud" | "cleanup";

/** Backup history with restore, undo of the last switch, and the handoff to
 * the system file manager for cleanup. */
export function BackupsPage({
  records,
  busy,
  active,
  recoveryRequest,
  lastSwitch,
  cloudBackup,
  onRestore,
  onUndo,
  onOpenDir,
  onCleaned,
}: BackupsPageProps) {
  const { t } = useI18n();
  const [activeTab, setActiveTab] = useState<BackupTab>("local");
  useEffect(() => { setActiveTab("local"); }, [recoveryRequest]);

  return (
    <section className="asb-panel" aria-label={t("backup.aria")}>
      {/* 设置内容区的子页：模块级 h3 标题独占第一行，备份类型页签在第二行。 */}
      <ModuleHeader
        title={t("backup.title")}
        primary={
          <Tabs value={activeTab} onChange={setActiveTab} scope="backup" label={t("backup.tabsAria")}
            tabs={[{ value: "local", label: t("backup.tab.local"), controls: "backup-local-panel" },
              { value: "cloud", label: t("backup.tab.cloud"), controls: "backup-cloud-panel" },
              { value: "cleanup", label: t("backup.tab.cleanup"), controls: "backup-cleanup-panel" }]} />
        }
      />
      {/* Keep both panels mounted to preserve each tab's session state. */}
      <div
        id="backup-local-panel"
        className="asb-backup-local"
        role="tabpanel"
        aria-labelledby="backup-local-tab"
        hidden={activeTab !== "local"}
      >
          <>
            {lastSwitch && (
              <p className="asb-scope-note">
                {t("backup.lastSwitch.prefix", { app: clientName(lastSwitch.app) })}
                {lastSwitch.operation === "projection" && lastSwitch.profileName
                  ? t("backup.lastSwitch.projectionNamed", { name: lastSwitch.profileName })
                  : lastSwitch.operation === "projection"
                    ? t("backup.lastSwitch.projectionPlain")
                    : lastSwitch.operation === "restore"
                      ? t("backup.lastSwitch.restored")
                      : t("backup.lastSwitch.gatewayPort")}
                {t("backup.lastSwitch.timeBefore")}
                <Time iso={lastSwitch.at} />
                {t("backup.lastSwitch.timeAfter")}
              </p>
            )}
            <div className="asb-backup-toolbar">
              <div className="asb-panel-actions">
                <Button variant="secondary" onClick={onOpenDir}>
                  {t("backup.openFolder")}
                </Button>
              </div>
            </div>
            {lastSwitch && lastSwitch.operation !== "gatewayPortChange" && (
              <div className="asb-backup-danger-row">
                <Button
                  variant="danger"
                  disabled={busy}
                  onClick={() => onUndo(lastSwitch)}
                >
                  {lastSwitch.profileName ? t("backup.undoSwitch") : t("backup.undoWrite")}
                </Button>
              </div>
            )}
            <BackupHistory records={records} busy={busy} onRestore={onRestore} />
          </>
      </div>
      <div
        id="backup-cloud-panel"
        role="tabpanel"
        aria-labelledby="backup-cloud-tab"
        hidden={activeTab !== "cloud"}
      >
          <CloudBackupPanel
            active={active && activeTab === "cloud"}
            settings={cloudBackup.settings}
            loaded={cloudBackup.loaded}
            busy={busy}
            onSave={cloudBackup.saveSettings}
            onTestConnection={cloudBackup.testConnection}
            onUpload={cloudBackup.upload}
            onRestore={cloudBackup.restore}
          />
      </div>
      <div
        id="backup-cleanup-panel"
        role="tabpanel"
        aria-labelledby="backup-cleanup-tab"
        hidden={activeTab !== "cleanup"}
      >
          <BackupCleanup busy={busy} onCleaned={onCleaned} />
      </div>
    </section>
  );
}
