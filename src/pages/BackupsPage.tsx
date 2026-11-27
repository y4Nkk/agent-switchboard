import { useEffect, useState } from "react";
import type { BackupRecord, ConfigWriteRecord } from "../api/client";
import type { useCloudBackup } from "../app/useCloudBackup";
import { BackupArchiveArt } from "../components/BackupArchiveArt";
import { BackupCleanup } from "../components/BackupCleanup";
import { BackupHistory } from "../components/BackupHistory";
import { Button } from "../components/Button";
import { CloudBackupPanel } from "../components/CloudBackupPanel";
import { SettingsGroup, SettingsLayout } from "../components/SettingsLayout";
import { Tabs } from "../components/Tabs";
import { Time } from "../components/Time";
import { ModuleHeader } from "../components/WorkspaceHeader";
import { useWorkspaceVisible } from "../hooks/use-workspace-visible";
import { useI18n } from "../i18n";
import { clientName } from "../lib/client-name";

interface BackupsPageProps {
  records: BackupRecord[];
  busy: boolean;
  active: boolean;
  recoveryRequest: number;
  configurationReady: boolean;
  recoveryBackupId: string | null;
  lastSwitch: ConfigWriteRecord | null;
  cloudBackup: ReturnType<typeof useCloudBackup>;
  onRestore: (backupId: string, transactionOnly: boolean) => Promise<void>;
  onUndo: (lastSwitch: ConfigWriteRecord) => void;
  onOpenDir: () => void;
  /** Refreshes the record list after a bulk cleanup reclaimed backups. */
  onCleaned: () => void;
}

type BackupTab = "local" | "cloud" | "cleanup";

function LastWrite({ lastSwitch, busy, configurationReady, onUndo }: {
  lastSwitch: ConfigWriteRecord;
  busy: boolean;
  configurationReady: boolean;
  onUndo: (lastSwitch: ConfigWriteRecord) => void;
}) {
  const { t } = useI18n();
  return (
    <>
      <p className="asb-backup-last-write">
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
      {lastSwitch.operation !== "gatewayPortChange" && (
        <Button variant="danger" disabled={busy || !configurationReady} onClick={() => onUndo(lastSwitch)}>
          {lastSwitch.profileName ? t("backup.undoSwitch") : t("backup.undoWrite")}
        </Button>
      )}
    </>
  );
}

function LocalArchive({ records, lastSwitch, busy, configurationReady, active, onUndo }: Pick<BackupsPageProps,
  "records" | "lastSwitch" | "busy" | "configurationReady" | "active" | "onUndo">) {
  const { t } = useI18n();
  return <div className="asb-backup-hero">
    <div className="asb-backup-hero-art"><BackupArchiveArt kind="local" active={active} /></div>
    <div className="asb-backup-hero-copy">
      <span className="asb-backup-history-count asb-num">
        {records.length ? t("backup.history.count", { count: records.length }) : t("backup.history.empty")}
      </span>
      {lastSwitch && <LastWrite lastSwitch={lastSwitch} busy={busy} configurationReady={configurationReady} onUndo={onUndo} />}
    </div>
  </div>;
}

/** Backup history with restore, undo of the last switch, and the handoff to
 * the system file manager for cleanup. */
export function BackupsPage({
  records,
  busy,
  active,
  recoveryRequest,
  configurationReady,
  recoveryBackupId,
  lastSwitch,
  cloudBackup,
  onRestore,
  onUndo,
  onOpenDir,
  onCleaned,
}: BackupsPageProps) {
  const { t } = useI18n();
  const visible = useWorkspaceVisible(active);
  const [activeTab, setActiveTab] = useState<BackupTab>("local");
  useEffect(() => { setActiveTab("local"); }, [recoveryRequest]);

  return (
    <section className="asb-panel" aria-label={t("backup.aria")}>
      {/* 设置内容区的子页：模块级 h3 标题独占第一行，备份类型页签在第二行，
          本地备份文件夹入口作为页面动作挂在页签右侧。 */}
      <ModuleHeader
        title={t("backup.title")}
        primary={
          <Tabs value={activeTab} onChange={setActiveTab} scope="backup" label={t("backup.tabsAria")}
            tabs={[{ value: "local", label: t("backup.tab.local"), controls: "backup-local-panel" },
              { value: "cloud", label: t("backup.tab.cloud"), controls: "backup-cloud-panel" },
              { value: "cleanup", label: t("backup.tab.cleanup"), controls: "backup-cleanup-panel" }]} />
        }
        primaryActions={activeTab === "local" && (
          <Button variant="secondary" onClick={onOpenDir}>{t("backup.openFolder")}</Button>
        )}
      />
      {/* Keep the backup tabs mounted so switching retains their state. */}
      <SettingsLayout
        id="backup-local-panel"
        role="tabpanel"
        aria-labelledby="backup-local-tab"
        hidden={activeTab !== "local"}
      >
        <SettingsGroup title={t("backup.history.title")} surface="plain">
          <LocalArchive records={records} lastSwitch={lastSwitch} busy={busy} configurationReady={configurationReady}
            active={visible && activeTab === "local"} onUndo={onUndo} />
          <BackupHistory records={records} busy={busy} onRestore={onRestore}
            configurationReady={configurationReady} recoveryBackupId={recoveryBackupId} />
        </SettingsGroup>
      </SettingsLayout>
      <SettingsLayout
        id="backup-cloud-panel"
        role="tabpanel"
        aria-labelledby="backup-cloud-tab"
        hidden={activeTab !== "cloud"}
      >
        <CloudBackupPanel
          active={active && activeTab === "cloud"}
          visible={visible && activeTab === "cloud"}
          settings={cloudBackup.settings}
          loaded={cloudBackup.loaded}
          busy={busy}
          onSave={cloudBackup.saveSettings}
          onTestConnection={cloudBackup.testConnection}
          onUpload={cloudBackup.upload}
          onRestore={cloudBackup.restore}
        />
      </SettingsLayout>
      <SettingsLayout
        id="backup-cleanup-panel"
        role="tabpanel"
        aria-labelledby="backup-cleanup-tab"
        hidden={activeTab !== "cleanup"}
      >
        <BackupCleanup active={active && activeTab === "cleanup"} busy={busy} onCleaned={onCleaned} />
      </SettingsLayout>
    </section>
  );
}
