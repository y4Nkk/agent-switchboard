import { useCallback, useEffect, useState } from "react";
import {
  getCloudBackupSettings,
  setCloudBackupSettings,
  restoreCloudBackup,
  testCloudBackupConnection,
  uploadCloudBackup,
  type CloudBackupSettings,
  type CommandError,
} from "../api/client";
import { toast } from "../components/use-toast";

interface CloudBackupDeps {
  busy: boolean;
  setBusy: (busy: boolean) => void;
  clearError: () => void;
  onError: (error: CommandError) => void;
  invalidateCandidates: () => void;
  refresh: () => Promise<void>;
}

/** Owns cloud-backup settings and serialises upload/restore into the shared
 * operation frame so a profile-store replacement cannot race local edits. */
export function useCloudBackup({
  busy,
  setBusy,
  clearError,
  onError,
  invalidateCandidates,
  refresh,
}: CloudBackupDeps) {
  const [settings, setSettings] = useState<CloudBackupSettings | null>(null);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let current = true;
    void getCloudBackupSettings()
      .then((result) => {
        if (current) setSettings(result);
      })
      .catch((caught) => {
        if (current) onError(caught as CommandError);
      })
      .finally(() => {
        if (current) setLoaded(true);
      });
    return () => {
      current = false;
    };
  }, [onError]);

  const saveSettings = useCallback(
    async (next: CloudBackupSettings) => {
      if (busy) return false;
      setBusy(true);
      clearError();
      try {
        const saved = await setCloudBackupSettings(next);
        setSettings(saved);
        toast({ kind: "success", title: "已保存云端备份连接" });
        return true;
      } catch (caught) {
        onError(caught as CommandError);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [busy, clearError, onError, setBusy],
  );

  const upload = useCallback(
    async (accountPassword: string, backupPassword: string) => {
      if (busy) return false;
      setBusy(true);
      clearError();
      try {
        const result = await uploadCloudBackup(accountPassword, backupPassword, true);
        toast({
          kind: "success",
          title: "已上传加密云端备份",
          description: `包含 ${result.profileCount} 个供应商档案`,
        });
        return true;
      } catch (caught) {
        onError(caught as CommandError);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [busy, clearError, onError, setBusy],
  );

  const testConnection = useCallback(
    async (next: CloudBackupSettings, accountPassword: string) => {
      if (busy) return false;
      setBusy(true);
      clearError();
      try {
        await testCloudBackupConnection(next, accountPassword);
        toast({
          kind: "success",
          title: "Supabase 连接可用",
          description: "已验证登录和云端备份表读取权限",
        });
        return true;
      } catch (caught) {
        onError(caught as CommandError);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [busy, clearError, onError, setBusy],
  );

  const restore = useCallback(
    async (accountPassword: string, backupPassword: string) => {
      if (busy) return false;
      setBusy(true);
      clearError();
      try {
        const result = await restoreCloudBackup(accountPassword, backupPassword, true);
        invalidateCandidates();
        await refresh();
        toast({
          kind: "success",
          title: "已恢复加密云端备份",
          description: `已恢复 ${result.profileCount} 个供应商档案`,
        });
        return true;
      } catch (caught) {
        onError(caught as CommandError);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [busy, clearError, invalidateCandidates, onError, refresh, setBusy],
  );

  return { settings, loaded, saveSettings, testConnection, upload, restore };
}
