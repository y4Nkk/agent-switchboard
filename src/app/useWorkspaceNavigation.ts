import { useCallback, useEffect, useRef, useState } from "react";
import { getStartupPage, onExitRecovery, rememberWorkspacePage, type CommandError } from "../api/client";
import type { DiagnosticSection, ExtensionSection, Page, ProviderView, SettingsSection, UsageSection } from "./navigation";

export function useWorkspaceNavigation(settingsReady: boolean, settingsError: string | null, onError: (error: CommandError) => void) {
  const [page, changePage] = useState<Page>("providers");
  const [navigationReady, setNavigationReady] = useState(false);
  const initialized = useRef(false);
  const userNavigated = useRef(false);
  const writes = useRef<Promise<void>>(Promise.resolve());
  useEffect(() => {
    if (!settingsReady || initialized.current) return;
    if (settingsError) {
      initialized.current = true;
      setNavigationReady(true);
      return;
    }
    let disposed = false;
    void getStartupPage().then((saved) => {
      if (!disposed && !userNavigated.current) changePage(saved);
    }).catch((error: CommandError) => {
      if (!disposed) onError(error);
    }).finally(() => {
      if (!disposed) { initialized.current = true; setNavigationReady(true); }
    });
    return () => { disposed = true; };
  }, [settingsReady, settingsError, onError]);
  useEffect(() => {
    if (!navigationReady || !userNavigated.current) return;
    // Serialize explicit visits so a slower earlier save cannot replace a newer page.
    writes.current = writes.current.then(() => rememberWorkspacePage(page)).catch(onError);
  }, [navigationReady, page, onError]);
  const [providerView, setProviderView] = useState<ProviderView>({ kind: "list" });
  const [settingsSection, setSettingsSection] = useState<SettingsSection>("application");
  const [diagnosticSection, setDiagnosticSection] = useState<DiagnosticSection>("configuration");
  const [extensionSection, setExtensionSection] = useState<ExtensionSection>("skill");
  const [usageSection, setUsageSection] = useState<UsageSection>("consumption");
  const [settingsReturnToProviders, setSettingsReturnToProviders] = useState(false);
  const [backupRecoveryRequest, setBackupRecoveryRequest] = useState(0);
  const setPage = useCallback((next: Page) => {
    userNavigated.current = true;
    setSettingsReturnToProviders(false);
    changePage(next);
  }, []);
  const openSettings = useCallback((section: SettingsSection, diagnostic?: DiagnosticSection) => {
    userNavigated.current = true;
    setSettingsReturnToProviders(page === "providers");
    setSettingsSection(section);
    if (diagnostic) setDiagnosticSection(diagnostic);
    changePage("settings");
  }, [page]);
  const returnToProviders = useCallback(() => setPage("providers"), [setPage]);
  const openQuota = useCallback(() => {
    setUsageSection("quota");
    setPage("usage");
  }, [setPage]);
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void onExitRecovery((destination) => {
      if (destination === "radar") { setUsageSection("radar"); setPage("usage"); }
      else if (destination === "clientManagement") { setSettingsSection("client-management"); setPage("settings"); }
      else if (destination === "sessions") setPage("sessions");
      else { setBackupRecoveryRequest((request) => request + 1); setSettingsSection("backups"); setPage("settings"); }
    }).then((unlisten) => { if (disposed) unlisten(); else stop = unlisten; })
      .catch((error: CommandError) => onError(error));
    return () => { disposed = true; stop?.(); };
  }, [setPage, onError]);
  return { page, setPage, navigationReady, providerView, setProviderView, settingsSection, setSettingsSection, diagnosticSection, setDiagnosticSection,
    extensionSection, setExtensionSection, usageSection, setUsageSection, settingsReturnToProviders,
    openSettings, returnToProviders, openQuota, backupRecoveryRequest };
}
