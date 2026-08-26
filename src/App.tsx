import { useCallback, useEffect, useMemo, useState } from "react";
import {
  createProfile,
  deleteProfile,
  discoverLocal,
  executeSwitch,
  getCommon,
  getConfigStatus,
  getLockStatus,
  importDiscoveredProfile,
  listBackups,
  listProfiles,
  previewSwitch,
  recoverStaleLock,
  restoreBackup,
  setCommon,
  undoLastSwitch,
  updateProfile,
  type AppKind,
  type BackupRecord,
  type CommandError,
  type CommonConfigPatch,
  type ConfigFileStatus,
  type DiscoveryReport,
  type FilePreview,
  type MatchStatus,
  type LockStatus,
  type ProviderDraft,
  type ProviderProfile,
  type RouteState,
  type SwitchOutcome,
  type SwitchLog,
} from "./api/client";
import { BackupHistory } from "./components/BackupHistory";
import { ConfirmSheet } from "./components/ConfirmSheet";
import { DualRelay } from "./components/DualRelay";
import { GeneralSettingsForm } from "./components/GeneralSettingsForm";
import { PreviewInspector } from "./components/PreviewInspector";
import { ProviderEditor } from "./components/ProviderEditor";
import { ProviderList } from "./components/ProviderList";

const PAGES = ["概览", "供应商", "通用设置", "备份", "发现"] as const;
type Page = (typeof PAGES)[number];
type EditorMode = "new" | "edit" | null;
const PAGE_ICONS: Record<Page, string> = {
  概览: "⌂",
  供应商: "◌",
  通用设置: "⌘",
  备份: "↶",
  发现: "⌕",
};

function routesFrom(statuses: ConfigFileStatus[]): {
  codex: RouteState | null;
  claude: RouteState | null;
} {
  const find = (app: string) => statuses.find((status) => status.app === app)?.route ?? null;
  return { codex: find("codex"), claude: find("claude") };
}

function clientName(app: AppKind): string {
  return app === "codex" ? "Codex" : "Claude";
}

function timeLabel(iso: string): string {
  return iso.replace("T", " ").replace(/(?:\.\d+)?(?:Z|\+00:00)$/, " 世界协调时");
}

function matchLabel(status: MatchStatus): string {
  switch (status.kind) {
    case "matchesProfile":
      return `与档案「${status.profileName}」一致`;
    case "profileChanged":
      return `档案「${status.profileName}」或通用设置已变更，尚未应用`;
    case "restoredBackup":
      return `当前为已恢复备份（${timeLabel(status.at)}）`;
    case "externallyModified":
      return `与上次切换 (${timeLabel(status.at)}) 不符，配置可能被外部修改`;
    case "unmanaged":
      return "从未由本应用切换，也不匹配任何档案";
    case "unknown":
      return "无法评估（文件缺失或语法错误）";
  }
}

function lockLabel(status: LockStatus | undefined): string {
  if (!status) return "写入锁状态加载中";
  switch (status.state) {
    case "free":
      return "写入锁空闲";
    case "held": {
      const holder = status.processName ?? (status.pid ? `进程 ${status.pid}` : "其他进程");
      return `写入锁由${holder}持有`;
    }
    case "stale":
      return "发现遗留写入锁，可在确认后清理";
    case "indeterminate":
      return `写入锁状态无法确定：${status.reason}`;
  }
}

/** Latest switch log entry across both clients, for the undo affordance. */
function latestOverall(statuses: ConfigFileStatus[]): SwitchLog | null {
  const entries = statuses
    .map((status) => status.lastSwitch)
    .filter((entry): entry is SwitchLog => entry !== null);
  if (entries.length === 0) return null;
  return entries.reduce((latest, entry) => (entry.at > latest.at ? entry : latest));
}

export default function App() {
  const [page, setPage] = useState<Page>("概览");
  const [statuses, setStatuses] = useState<ConfigFileStatus[] | null>(null);
  const [profiles, setProfiles] = useState<ProviderProfile[]>([]);
  const [backups, setBackups] = useState<BackupRecord[]>([]);
  const [locks, setLocks] = useState<Partial<Record<AppKind, LockStatus>>>({});
  const [appFilter, setAppFilter] = useState<AppKind>("codex");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [preview, setPreview] = useState<FilePreview | null>(null);
  const [error, setError] = useState<CommandError | null>(null);
  const [operationWarnings, setOperationWarnings] = useState<string[]>([]);
  const [outcome, setOutcome] = useState<SwitchOutcome | null>(null);
  const [confirmingSwitch, setConfirmingSwitch] = useState(false);
  const [busy, setBusy] = useState(false);
  const [commonPatches, setCommonPatches] = useState<Record<string, CommonConfigPatch>>({});
  const [discovery, setDiscovery] = useState<DiscoveryReport | null>(null);
  const [editorMode, setEditorMode] = useState<EditorMode>(null);
  const [deletePending, setDeletePending] = useState<ProviderProfile | null>(null);
  const [undoPending, setUndoPending] = useState<SwitchLog | null>(null);
  const [recoverLockPending, setRecoverLockPending] = useState<AppKind | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [nextStatuses, nextProfiles, nextBackups, codexLock, claudeLock] = await Promise.all([
        getConfigStatus(),
        listProfiles(),
        listBackups(),
        getLockStatus("codex"),
        getLockStatus("claude"),
      ]);
      setStatuses(nextStatuses);
      setProfiles(nextProfiles);
      setBackups(nextBackups);
      setLocks({ codex: codexLock, claude: claudeLock });
      setSelectedId((current) =>
        current && !nextProfiles.some((profile) => profile.id === current) ? null : current,
      );
    } catch (caught) {
      setError(caught as CommandError);
    }
  }, []);

  useEffect(() => {
    void refresh();
    for (const app of ["codex", "claude"] as const) {
      void getCommon(app)
        .then((patch) => setCommonPatches((current) => ({ ...current, [app]: patch })))
        .catch((caught) => setError(caught as CommandError));
    }
  }, [refresh]);

  const routes = useMemo(() => routesFrom(statuses ?? []), [statuses]);
  const selectedProfile = profiles.find((profile) => profile.id === selectedId) ?? null;
  const visibleProfiles = profiles.filter((profile) => profile.app === appFilter);
  const activeProfileId = useMemo(() => {
    const status = (statuses ?? []).find((item) => item.app === appFilter);
    if (status?.matchStatus.kind === "matchesProfile") {
      return status.matchStatus.profileId;
    }
    return null;
  }, [statuses, appFilter]);
  const lastSwitchOverall = useMemo(() => latestOverall(statuses ?? []), [statuses]);

  const selectProfile = useCallback(async (profileId: string) => {
    if (busy) return;
    setSelectedId(profileId);
    setPreview(null);
    setOutcome(null);
    setOperationWarnings([]);
    setError(null);
    try {
      setPreview(await previewSwitch(profileId));
    } catch (caught) {
      setError(caught as CommandError);
    }
  }, [busy]);

  const runPreview = useCallback(async () => {
    if (busy || !selectedId) return;
    setError(null);
    setPreview(null);
    setOperationWarnings([]);
    try {
      setPreview(await previewSwitch(selectedId));
    } catch (caught) {
      setError(caught as CommandError);
    }
  }, [busy, selectedId]);

  const saveProfile = useCallback(
    async (draft: ProviderDraft) => {
      setBusy(true);
      setError(null);
      setOperationWarnings([]);
      try {
        const saved =
          editorMode === "edit" && selectedProfile
            ? await updateProfile(selectedProfile.id, draft)
            : await createProfile(draft);
        setAppFilter(saved.app);
        setEditorMode(null);
        await refresh();
        await selectProfile(saved.id);
      } catch (caught) {
        setError(caught as CommandError);
      } finally {
        setBusy(false);
      }
    },
    [editorMode, refresh, selectProfile, selectedProfile],
  );

  const runDelete = useCallback(async () => {
    if (busy || !deletePending) return;
    setDeletePending(null);
    setBusy(true);
    setError(null);
    setOutcome(null);
    setOperationWarnings([]);
    try {
      await deleteProfile(deletePending.id);
      if (selectedId === deletePending.id) {
        setSelectedId(null);
        setPreview(null);
      }
      setEditorMode(null);
      await refresh();
    } catch (caught) {
      setError(caught as CommandError);
    } finally {
      setBusy(false);
    }
  }, [busy, deletePending, refresh, selectedId]);

  const runSwitch = useCallback(async () => {
    if (busy || !selectedId || !preview) return;
    setConfirmingSwitch(false);
    setBusy(true);
    setError(null);
    setOperationWarnings([]);
    try {
      const result = await executeSwitch(
        selectedId,
        preview.contentHash,
        preview.renderedHash,
        true,
      );
      await refresh();
      await selectProfile(selectedId);
      setOutcome(result);
      setOperationWarnings(result.warnings);
      try {
        setDiscovery(await discoverLocal());
      } catch {
        setOperationWarnings((current) => [
          ...current,
          "配置已写入，但无法刷新本机配置发现结果。",
        ]);
      }
    } catch (caught) {
      const commandError = caught as CommandError;
      setError(commandError);
      if (commandError.code === "external-change" || commandError.code === "preview-stale") {
        setPreview(null);
        setOutcome(null);
      }
      await refresh();
    } finally {
      setBusy(false);
    }
  }, [busy, preview, refresh, selectProfile, selectedId]);

  const runRestore = useCallback(
    async (backupId: string) => {
      if (busy) return;
      setBusy(true);
      setError(null);
      setOutcome(null);
      setPreview(null);
      setOperationWarnings([]);
      try {
        const result = await restoreBackup(backupId, true);
        await refresh();
        if (selectedId) await selectProfile(selectedId);
        setOperationWarnings(result.warnings);
        try {
          setDiscovery(await discoverLocal());
        } catch {
          setOperationWarnings((current) => [
            ...current,
            "配置已恢复，但无法刷新本机配置发现结果。",
          ]);
        }
      } catch (caught) {
        setError(caught as CommandError);
      } finally {
        setBusy(false);
      }
    },
    [busy, refresh, selectProfile, selectedId],
  );

  const runUndo = useCallback(async () => {
    if (busy || !undoPending) return;
    setUndoPending(null);
    setBusy(true);
    setError(null);
    setOutcome(null);
    setPreview(null);
    setOperationWarnings([]);
    try {
      const result = await undoLastSwitch(undoPending.app, true);
      await refresh();
      if (selectedId) await selectProfile(selectedId);
      setOperationWarnings(result.warnings);
      try {
        setDiscovery(await discoverLocal());
      } catch {
        setOperationWarnings((current) => [
          ...current,
          "配置已撤回，但无法刷新本机配置发现结果。",
        ]);
      }
    } catch (caught) {
      setError(caught as CommandError);
    } finally {
      setBusy(false);
    }
  }, [busy, refresh, selectProfile, selectedId, undoPending]);

  const runRecoverStaleLock = useCallback(async () => {
    if (busy || !recoverLockPending) return;
    const target = recoverLockPending;
    setRecoverLockPending(null);
    setBusy(true);
    setError(null);
    setOperationWarnings([]);
    try {
      await recoverStaleLock(target);
      await refresh();
    } catch (caught) {
      setError(caught as CommandError);
    } finally {
      setBusy(false);
    }
  }, [busy, recoverLockPending, refresh]);

  const saveCommon = useCallback(async (patch: CommonConfigPatch) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await setCommon(patch.app, patch);
      setCommonPatches((current) => ({ ...current, [patch.app]: patch }));
      setPreview(null);
      setOutcome(null);
      setOperationWarnings([]);
      await refresh();
    } catch (caught) {
      setError(caught as CommandError);
    } finally {
      setBusy(false);
    }
  }, [busy, refresh]);

  const runDiscovery = useCallback(async () => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      setDiscovery(await discoverLocal());
    } catch (caught) {
      setError(caught as CommandError);
    } finally {
      setBusy(false);
    }
  }, [busy]);

  const runImport = useCallback(
    async (app: AppKind) => {
      if (busy) return;
      setBusy(true);
      setError(null);
      setOperationWarnings([]);
      try {
        const profile = await importDiscoveredProfile(app);
        setAppFilter(profile.app);
        setPage("供应商");
        setDiscovery(null);
        setOutcome(null);
        setOperationWarnings([]);
        await refresh();
        await selectProfile(profile.id);
      } catch (caught) {
        setError(caught as CommandError);
      } finally {
        setBusy(false);
      }
    },
    [busy, refresh, selectProfile],
  );

  return (
    <>
      <div className="asb-ambient" aria-hidden="true" />
      <div className="asb-shell">
        <aside className="asb-sidebar asb-glass">
          <h1 className="asb-sidebar-title">Agent Switchboard</h1>
          <nav aria-label="主导航">
            <ul className="asb-nav">
              {PAGES.map((item) => (
                <li key={item}>
                  <button
                    type="button"
                    aria-label={item}
                    aria-current={page === item ? "page" : undefined}
                    onClick={() => setPage(item)}
                  >
                    <span className="asb-nav-icon" aria-hidden="true">
                      {PAGE_ICONS[item]}
                    </span>
                    <span className="asb-nav-label">{item}</span>
                  </button>
                </li>
              ))}
            </ul>
          </nav>
        </aside>
        <main className="asb-main" aria-label={page}>
          {error && (
            <div className="asb-banner asb-banner-error" role="alert" aria-label="操作错误">
              <span>{error.message}</span>
            </div>
          )}
          {operationWarnings.length > 0 && (
            <div className="asb-banner asb-banner-warning" role="status" aria-label="操作警告">
              <ul className="asb-banner-list">
                {operationWarnings.map((warning) => (
                  <li key={warning}>{warning}</li>
                ))}
              </ul>
            </div>
          )}
          {outcome && (
            <div className="asb-banner asb-banner-ok" role="status" aria-label="切换结果">
              <span>
                已完成切换 · 备份 {outcome.backup.backupPath}
                {outcome.warnings.length > 0 && ` · 警告 ${outcome.warnings.length} 条`}
                · 客户端在下次启动时读取新配置
              </span>
            </div>
          )}
          <DualRelay
            routes={routes}
            selectedProfile={selectedProfile}
            canSwitch={preview !== null}
            busy={busy}
            onPreview={runPreview}
            onSwitch={() => setConfirmingSwitch(true)}
          />
          {page === "概览" && (
            <section className="asb-panel" aria-label="配置状态">
              <div className="asb-panel-heading">
                <h2 className="asb-panel-title">配置状态</h2>
                <button
                  type="button"
                  className="asb-btn-secondary"
                  disabled={busy}
                  onClick={() => void refresh()}
                >
                  刷新状态
                </button>
              </div>
              <p className="asb-scope-note">仅管理用户级配置；项目级配置与命令行参数可能覆盖此处设置。</p>
              {(statuses ?? []).map((status) => (
                <div className="asb-kv" key={status.app}>
                  <span className="asb-kv-label">{clientName(status.app)}</span>
                  <span className="asb-kv-value asb-code">{status.path}</span>
                  <span className="asb-kv-value">
                    {status.readError
                      ? status.readError
                      : !status.exists
                        ? "未找到配置文件"
                        : !status.syntaxOk
                          ? "语法错误"
                          : `${status.route?.routeMode === "official" ? "官方登录" : "自定义服务"} · ${status.route?.model ?? "默认模型"}`}
                  </span>
                  {status.syntaxOk && (
                    <span className="asb-kv-value">{matchLabel(status.matchStatus)}</span>
                  )}
                  {status.lastSwitch && (
                    <span className="asb-kv-value">
                      上次切换 {timeLabel(status.lastSwitch.at)}
                      {status.lastSwitch.profileName
                        ? ` · ${status.lastSwitch.profileName}`
                        : " · 已恢复备份"}
                    </span>
                  )}
                  {status.route?.scopeWarnings.map((warning) => (
                    <span key={warning} className="asb-kv-value asb-warn-text">
                      {warning}
                    </span>
                  ))}
                  <span className="asb-kv-value">{lockLabel(locks[status.app])}</span>
                  {locks[status.app]?.state === "stale" && (
                    <div className="asb-kv-actions">
                      <button
                        type="button"
                        className="asb-btn-secondary"
                        disabled={busy}
                        onClick={() => setRecoverLockPending(status.app)}
                      >
                        清理遗留锁
                      </button>
                    </div>
                  )}
                </div>
              ))}
              {statuses === null && <p className="asb-empty">加载中</p>}
            </section>
          )}
          {page === "供应商" && (
            <div className="asb-workspace">
              <section className="asb-panel" aria-label="供应商工作区">
                <div className="asb-panel-heading">
                  <h2 className="asb-panel-title">供应商</h2>
                  {editorMode === null && (
                    <button
                      type="button"
                      className="asb-btn-secondary"
                      disabled={busy}
                      onClick={() => setEditorMode("new")}
                    >
                      新建供应商
                    </button>
                  )}
                </div>
                {editorMode ? (
                  <ProviderEditor
                    profile={editorMode === "edit" ? selectedProfile : null}
                    initialApp={appFilter}
                    busy={busy}
                    onSave={saveProfile}
                    onCancel={() => setEditorMode(null)}
                  />
                ) : (
                  <>
                    <div className="asb-tabs" role="tablist" aria-label="客户端">
                      {(["codex", "claude"] as const).map((app) => (
                        <button
                          key={app}
                          type="button"
                          role="tab"
                          aria-selected={appFilter === app}
                          className={`asb-tab${appFilter === app ? " is-on" : ""}`}
                          onClick={() => {
                            setAppFilter(app);
                            setSelectedId(null);
                            setPreview(null);
                            setOutcome(null);
                            setOperationWarnings([]);
                          }}
                        >
                          {clientName(app)}
                        </button>
                      ))}
                    </div>
                    <ProviderList
                      profiles={visibleProfiles}
                      activeProfileId={activeProfileId}
                      selectedId={selectedId}
                      onSelect={selectProfile}
                    />
                    {selectedProfile && (
                      <div className="asb-form-actions">
                        <button
                          type="button"
                          className="asb-btn-secondary"
                          disabled={busy}
                          onClick={() => setEditorMode("edit")}
                        >
                          编辑供应商
                        </button>
                        <button
                          type="button"
                          className="asb-btn-secondary"
                          disabled={busy}
                          onClick={() => setDeletePending(selectedProfile)}
                        >
                          删除供应商
                        </button>
                      </div>
                    )}
                  </>
                )}
              </section>
              <section className="asb-panel" aria-label="变更预览">
                <h2 className="asb-panel-title">变更预览</h2>
                <PreviewInspector filePreview={preview} />
              </section>
            </div>
          )}
          {page === "通用设置" && (
            <div className="asb-workspace">
              {(["codex", "claude"] as const).map((app) => (
                <section className="asb-panel" key={app} aria-label={`${clientName(app)} 通用设置`}>
                  <h2 className="asb-panel-title">{clientName(app)} 通用设置</h2>
                  {commonPatches[app] ? (
                    <GeneralSettingsForm
                      patch={commonPatches[app]}
                      busy={busy}
                      onChange={saveCommon}
                    />
                  ) : (
                    <p className="asb-empty">加载中</p>
                  )}
                </section>
              ))}
            </div>
          )}
          {page === "备份" && (
            <section className="asb-panel" aria-label="备份历史">
              <div className="asb-panel-heading">
                <h2 className="asb-panel-title">备份历史</h2>
                {lastSwitchOverall && (
                  <button
                    type="button"
                    className="asb-btn-secondary"
                    disabled={busy}
                    onClick={() => setUndoPending(lastSwitchOverall)}
                  >
                    撤回上一次切换
                  </button>
                )}
              </div>
              {lastSwitchOverall && (
                <p className="asb-scope-note">
                  上次操作：{clientName(lastSwitchOverall.app)}
                  {lastSwitchOverall.profileName
                    ? ` 切换到「${lastSwitchOverall.profileName}」`
                    : " 恢复了备份"}
                  ，{timeLabel(lastSwitchOverall.at)}。
                </p>
              )}
              <BackupHistory records={backups} busy={busy} onRestore={runRestore} />
            </section>
          )}
          {page === "发现" && (
            <section className="asb-panel" aria-label="本机配置发现">
              <div className="asb-panel-heading">
                <h2 className="asb-panel-title">本机配置</h2>
                <button type="button" className="asb-btn-secondary" disabled={busy} onClick={runDiscovery}>
                  扫描配置
                </button>
              </div>
              {discovery && (
                <div className="asb-discovery">
                  {[discovery.codex, discovery.claude].map((file) => (
                    <div className="asb-kv" key={file.app}>
                      <span className="asb-kv-label">{clientName(file.app)}</span>
                      <span className="asb-kv-value asb-code">{file.path}</span>
                      <span className="asb-kv-value">
                        {file.state.kind === "missing"
                          ? "未找到配置文件"
                          : file.state.kind === "readError"
                            ? file.state.message
                          : file.state.kind === "parseError"
                            ? `语法错误（第 ${file.state.line ?? "?"} 行）`
                            : file.state.managed
                              ? "已由本应用管理"
                              : "未由本应用管理"}
                      </span>
                      {file.state.kind === "ok" &&
                        file.state.warnings.map((warning) => (
                          <span key={warning} className="asb-kv-value asb-warn-text">
                            {warning}
                          </span>
                        ))}
                      {file.state.kind === "ok" && !file.state.importable && !file.state.managed && (
                        <span className="asb-kv-value asb-warn-text">当前配置包含无法安全导入的设置。</span>
                      )}
                    </div>
                  ))}
                  {discovery.importProposals.map((proposal) => (
                    <div className="asb-kv" key={proposal.app}>
                      <span className="asb-kv-label">{proposal.draft.name}</span>
                      <span className="asb-kv-value">{proposal.basis}</span>
                      <div className="asb-routebar-actions">
                        <button
                          type="button"
                          className="asb-btn-secondary"
                          disabled={busy}
                          onClick={() => runImport(proposal.app)}
                        >
                          导入供应商
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </section>
          )}
        </main>
      </div>
      {confirmingSwitch && preview && (
        <ConfirmSheet
          title="确认切换"
          details={[
            `将写入 ${preview.preview.target}`,
            `变更 ${preview.preview.changes.length} 个键`,
            ...(preview.preview.warnings.length > 0 ? [`警告 ${preview.preview.warnings.length} 条`] : []),
            `备份位置 ${preview.preview.backupDir}`,
          ]}
          confirmLabel="确认切换"
          onConfirm={runSwitch}
          onCancel={() => setConfirmingSwitch(false)}
        />
      )}
      {deletePending && (
        <ConfirmSheet
          title="删除供应商"
          details={[`删除本地记录 ${deletePending.name}`, "不会修改当前客户端配置。"]}
          confirmLabel="确认删除"
          destructive
          onConfirm={runDelete}
          onCancel={() => setDeletePending(null)}
        />
      )}
      {undoPending && (
        <ConfirmSheet
          title="撤回上一次切换"
          details={[
            `${clientName(undoPending.app)} ${
              undoPending.profileName ? `上次切换到「${undoPending.profileName}」` : "上次操作是恢复备份"
            }`,
            `切换时间 ${timeLabel(undoPending.at)}`,
            "将恢复该次切换前的备份；当前内容会先另行备份。",
          ]}
          confirmLabel="确认撤回"
          onConfirm={runUndo}
          onCancel={() => setUndoPending(null)}
        />
      )}
      {recoverLockPending && (
        <ConfirmSheet
          title="清理遗留锁"
          details={[
            `${clientName(recoverLockPending)} 的遗留写入锁将被删除。`,
            "仅在确认该客户端没有正在进行的切换时继续。",
          ]}
          confirmLabel="确认清理"
          destructive
          onConfirm={runRecoverStaleLock}
          onCancel={() => setRecoverLockPending(null)}
        />
      )}
      {busy && (
        <div className="asb-busy" role="status" aria-label="处理中">
          处理中
        </div>
      )}
    </>
  );
}
