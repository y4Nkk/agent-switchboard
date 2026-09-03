import { useCallback, useEffect, useState } from "react";
import {
  listRuntimeLogs,
  openRuntimeLogDir,
  type CommandError,
  type RuntimeLogAction,
  type RuntimeLogEntry,
  type RuntimeLogLevel,
  type RuntimeLogSeverity,
} from "../api/client";
import { Button } from "../components/Button";
import { Select } from "../components/Select";
import { Table, type TableColumn } from "../components/Table";
import { Time } from "../components/Time";

type LevelFilter = "all" | RuntimeLogSeverity;

const LEVEL_FILTERS: ReadonlyArray<{ value: LevelFilter; label: string }> = [
  { value: "all", label: "全部" },
  { value: "debug", label: "调试" },
  { value: "info", label: "信息" },
  { value: "warn", label: "警告" },
  { value: "error", label: "错误" },
];

const LOG_LEVEL_OPTIONS: ReadonlyArray<{ value: RuntimeLogLevel; label: string }> = [
  { value: "debug", label: "调试" },
  { value: "info", label: "信息" },
  { value: "warn", label: "警告" },
  { value: "error", label: "错误" },
  { value: "silent", label: "静默" },
];

const ACTION_LABEL: Record<RuntimeLogAction, string> = {
  appStarted: "应用已启动",
  appSettingsSaved: "已保存应用设置",
  appSettingsRepaired: "已修复应用设置",
  profileStoreReset: "已重置供应商数据",
  profileCreated: "已创建供应商档案",
  profileUpdated: "已更新供应商档案",
  profileDeleted: "已删除供应商档案",
  profilesReordered: "已调整供应商顺序",
  profileImported: "已导入本机供应商档案",
  globalPromptDocumentSaved: "已保存全局提示词文档",
  configurationSwitched: "已切换配置",
  backupRestored: "已恢复备份",
  switchUndone: "已撤回上一次切换",
  staleLockRecovered: "已恢复遗留锁",
  cloudBackupSettingsSaved: "已保存云端备份设置",
  cloudBackupUploaded: "已上传云端备份",
  cloudBackupRestored: "已恢复云端备份",
  sessionResumed: "已恢复会话",
  ccSwitchProfilesImported: "已导入 CC Switch 档案",
  officialLoginCompleted: "已完成官方登录",
};

function levelLabel(level: RuntimeLogSeverity): string {
  switch (level) {
    case "debug":
      return "调试";
    case "info":
      return "信息";
    case "warn":
      return "警告";
    case "error":
      return "错误";
  }
}

const LOG_COLUMNS: Array<TableColumn<RuntimeLogEntry>> = [
  {
    key: "at",
    header: "时间",
    cellClassName: "asb-code",
    render: (entry) => <Time iso={entry.at} />,
  },
  {
    key: "level",
    header: "级别",
    render: (entry) => (
      <span className={`asb-runtime-log-level is-${entry.level}`}>{levelLabel(entry.level)}</span>
    ),
  },
  {
    key: "action",
    header: "事件",
    cellClassName: "asb-runtime-log-action",
    render: (entry) => ACTION_LABEL[entry.action],
  },
  {
    key: "errorCode",
    header: "错误代码",
    cellClassName: "asb-code",
    render: (entry) => entry.errorCode ?? "—",
  },
];

interface LogsPageProps {
  logLevel: RuntimeLogLevel | null;
  busy: boolean;
  onLogLevelChange: (level: RuntimeLogLevel) => void;
}

/** Read-only view of the application's own bounded diagnostic event files. */
export function LogsPage({ logLevel, busy, onLogLevelChange }: LogsPageProps) {
  const [entries, setEntries] = useState<RuntimeLogEntry[]>([]);
  const [filter, setFilter] = useState<LevelFilter>("all");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<CommandError | null>(null);
  const [openingFolder, setOpeningFolder] = useState(false);
  const [folderError, setFolderError] = useState<CommandError | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setEntries(await listRuntimeLogs());
    } catch (caught) {
      setError(caught as CommandError);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const openLogDirectory = useCallback(async () => {
    setOpeningFolder(true);
    setFolderError(null);
    try {
      await openRuntimeLogDir();
    } catch (caught) {
      setFolderError(caught as CommandError);
    } finally {
      setOpeningFolder(false);
    }
  }, []);

  const visibleEntries = filter === "all" ? entries : entries.filter((entry) => entry.level === filter);

  return (
    <section className="asb-panel asb-runtime-logs" aria-label="日志">
      <div className="asb-panel-heading">
        <div>
          <h2 className="asb-panel-title">日志</h2>
          <p className="asb-scope-note">仅显示本应用已脱敏的运行事件。</p>
        </div>
        <div className="asb-runtime-log-controls">
          <div className="asb-runtime-log-level-control">
            <span className="asb-runtime-log-level-label">记录级别</span>
            <Select
              value={logLevel}
              options={LOG_LEVEL_OPTIONS}
              ariaLabel="记录级别"
              placeholder="加载中"
              disabled={busy || logLevel === null}
              onChange={(level) => onLogLevelChange(level as RuntimeLogLevel)}
            />
          </div>
          <div className="asb-tabs" role="group" aria-label="日志级别筛选">
            {LEVEL_FILTERS.map((item) => (
              <button
                key={item.value}
                type="button"
                className={`asb-tab${filter === item.value ? " is-on" : ""}`}
                aria-pressed={filter === item.value}
                onClick={() => setFilter(item.value)}
              >
                {item.label}
              </button>
            ))}
          </div>
          <Button
            variant="secondary"
            disabled={loading}
            onClick={() => void refresh()}
          >
            {loading ? "刷新中" : "刷新"}
          </Button>
          <Button
            variant="secondary"
            disabled={openingFolder}
            onClick={() => void openLogDirectory()}
          >
            {openingFolder ? "打开中" : "打开日志文件夹"}
          </Button>
        </div>
      </div>
      {error && (
        <p className="asb-runtime-log-notice" role="alert">
          无法读取应用日志：{error.message}
        </p>
      )}
      {folderError && (
        <p className="asb-runtime-log-notice" role="alert">
          无法打开日志文件夹：{folderError.message}
        </p>
      )}
      {loading && entries.length === 0 ? (
        <p className="asb-empty asb-runtime-log-empty" role="status">
          正在读取应用日志…
        </p>
      ) : visibleEntries.length === 0 ? (
        <p className="asb-empty asb-runtime-log-empty">
          {filter === "all" ? "暂无应用运行日志" : `暂无${levelLabel(filter)}级别的应用日志`}
        </p>
      ) : (
        <div className="asb-runtime-log-table-wrap">
          <Table
            columns={LOG_COLUMNS}
            rows={visibleEntries}
            rowKey={(entry, index) => `${entry.at}-${entry.action}-${entry.errorCode ?? ""}-${index}`}
            ariaLabel="应用运行日志"
            className="asb-runtime-log-table"
          />
        </div>
      )}
    </section>
  );
}
