import type {
  AppKind,
  CcSwitchImportOutcome,
  CcSwitchScan,
  CcSwitchScanItem,
  DiscoveredState,
  DiscoveryReport,
} from "../api/client";
import { Button } from "../components/Button";
import { Checkbox } from "../components/Checkbox";
import { Table, type TableColumn } from "../components/Table";
import { clientName } from "../lib/client-name";

interface DiscoveryPageProps {
  discovery: DiscoveryReport | null;
  busy: boolean;
  onScan: () => void;
  onImport: (app: AppKind) => void;
}

function discoveryPill(state: DiscoveredState): { ok: boolean; text: string } {
  switch (state.kind) {
    case "ok":
      return { ok: true, text: "配置正常" };
    case "missing":
      return { ok: false, text: "未找到配置文件" };
    case "readError":
      return { ok: false, text: "读取失败" };
    case "parseError":
      return { ok: false, text: "语法错误" };
  }
}

interface DiscoveryCardProps {
  file: DiscoveryReport["codex"];
  proposal: DiscoveryReport["importProposals"][number] | undefined;
  busy: boolean;
  onImport: (app: AppKind) => void;
}

function DiscoveryCard({ file, proposal, busy, onImport }: DiscoveryCardProps) {
  const pill = discoveryPill(file.state);
  const ok = file.state.kind === "ok" ? file.state : null;
  const nonImportableHint =
    ok !== null && !ok.importable && !ok.managed
      ? "当前配置包含无法安全导入的设置。"
      : null;
  return (
    <article
      className="asb-status-card"
      aria-label={`${clientName(file.app)} 扫描结果`}
    >
      <header className="asb-status-head">
        <h3 className="asb-status-name">{clientName(file.app)}</h3>
        <span className={`asb-status-pill${pill.ok ? " is-ok" : ""}`}>
          <span className="asb-status-pill-dot" aria-hidden="true" />
          {pill.text}
        </span>
      </header>
      <dl className="asb-status-rows">
        <div className="asb-status-row">
          <dt>配置文件</dt>
          <dd className="asb-code">{file.path}</dd>
        </div>
        {file.state.kind === "readError" && (
          <div className="asb-status-row">
            <dt>读取错误</dt>
            <dd className="asb-warn-text">{file.state.message}</dd>
          </div>
        )}
        {file.state.kind === "parseError" && (
          <div className="asb-status-row">
            <dt>语法错误</dt>
            <dd className="asb-warn-text">
              {file.state.line !== null ? `第 ${file.state.line} 行 · ` : ""}
              {file.state.message}
            </dd>
          </div>
        )}
        {ok && (
          <>
            <div className="asb-status-row">
              <dt>当前服务</dt>
              <dd>
                {ok.route.routeMode === "official" ? "官方登录" : "自定义服务"}
                {" · "}
                {ok.route.model ?? "默认模型"}
              </dd>
            </div>
            {ok.route.providerName && (
              <div className="asb-status-row">
                <dt>供应商</dt>
                <dd>{ok.route.providerName}</dd>
              </div>
            )}
            {ok.route.baseUrl && (
              <div className="asb-status-row">
                <dt>服务地址</dt>
                <dd className="asb-code">{ok.route.baseUrl}</dd>
              </div>
            )}
            {ok.route.apiKey && (
              <div className="asb-status-row">
                <dt>凭据变量</dt>
                <dd className="asb-code">{ok.route.apiKey}</dd>
              </div>
            )}
            <div className="asb-status-row">
              <dt>管理状态</dt>
              <dd>{ok.managed ? "已由本应用管理" : "未由本应用管理"}</dd>
            </div>
            {(ok.warnings.length > 0 || nonImportableHint) && (
              <div className="asb-status-row">
                <dt>警告</dt>
                <dd>
                  {ok.warnings.map((warning) => (
                    <span key={warning} className="asb-warn-text asb-status-warn">
                      {warning}
                    </span>
                  ))}
                  {nonImportableHint && (
                    <span className="asb-warn-text asb-status-warn">{nonImportableHint}</span>
                  )}
                </dd>
              </div>
            )}
          </>
        )}
      </dl>
      {proposal && (
        <div className="asb-discovery-import">
          <p className="asb-discovery-basis">{proposal.basis}</p>
          <Button
            variant="secondary"
            disabled={busy}
            onClick={() => onImport(proposal.app)}
          >
            导入供应商
          </Button>
        </div>
      )}
    </article>
  );
}

/** Local configuration discovery: read-only per-client cards with in-card
 * import of the currently routed provider. */
export function DiscoveryPage({ discovery, busy, onScan, onImport }: DiscoveryPageProps) {
  return (
    <section className="asb-panel" aria-label="本机配置发现">
      <div className="asb-panel-heading">
        <h2 className="asb-panel-title">本机配置</h2>
        <Button variant="secondary" disabled={busy} onClick={onScan}>
          {discovery ? "刷新配置" : "扫描配置"}
        </Button>
      </div>
      {discovery && (
        <div className="asb-status-grid">
          {[discovery.codex, discovery.claude].map((file) => (
            <DiscoveryCard
              key={file.app}
              file={file}
              proposal={discovery.importProposals.find((item) => item.app === file.app)}
              busy={busy}
              onImport={onImport}
            />
          ))}
        </div>
      )}
    </section>
  );
}

/** One scan row: an importable provider or a skipped entry. */
interface CcImportRow {
  key: string;
  /** Present only when the row carries an import selection checkbox. */
  item: CcSwitchScanItem | null;
  name: string;
  detail: string | null;
  status: string | null;
  warnings: string[];
}

function providerDetail(item: CcSwitchScanItem): string {
  return [
    clientName(item.app),
    item.routeMode === "official" ? "官方登录" : null,
    item.model,
    item.baseUrl,
    item.usageScriptUpdatesExisting
      ? "将补充用量查询脚本"
      : item.usageScriptImportable
        ? "将导入用量查询脚本"
        : null,
  ]
    .filter(Boolean)
    .join(" · ");
}

interface CcImportSectionProps {
  scan: CcSwitchScan | null;
  selected: Record<string, boolean>;
  result: CcSwitchImportOutcome | null;
  busy: boolean;
  onSelect: (key: string, checked: boolean) => void;
  onScan: () => void;
  onImport: () => void;
}

/** Read-only CC Switch scan with a checkbox selection for import. API keys
 * and non-routing settings never cross this boundary. */
export function CcImportSection({
  scan,
  selected,
  result,
  busy,
  onSelect,
  onScan,
  onImport,
}: CcImportSectionProps) {
  const selectedCount = scan?.providers.filter((item) => selected[item.key]).length ?? 0;

  const rows: CcImportRow[] = scan
    ? [
        ...scan.providers.map((item) => ({
          key: item.key,
          item,
          name: item.name,
          detail: providerDetail(item),
          status: item.existing ? "已存在相同档案，导入将跳过" : null,
          warnings: item.warnings,
        })),
        ...scan.skipped.map((skip) => ({
          key: skip.key,
          item: null,
          name: skip.name,
          detail: null,
          status: `无法导入：${skip.reason}`,
          warnings: [],
        })),
      ]
    : [];

  const columns: Array<TableColumn<CcImportRow>> = [
    {
      key: "provider",
      header: "供应商",
      render: (row) => {
        const item = row.item;
        if (item === null) return row.name;
        return (
          <Checkbox
            label={row.name}
            checked={Boolean(selected[item.key]) && !item.existing}
            disabled={busy || item.existing}
            onChange={(checked) => onSelect(item.key, checked)}
          />
        );
      },
    },
    { key: "detail", header: "详情", render: (row) => row.detail },
    {
      key: "status",
      header: "状态",
      render: (row) => (
        <>
          {row.status}
          {row.warnings.map((warning) => (
            <div key={warning} className="asb-warn-text">
              {warning}
            </div>
          ))}
        </>
      ),
    },
  ];

  return (
    <section className="asb-panel" aria-label="从 CC Switch 导入">
      <div className="asb-panel-heading">
        <h2 className="asb-panel-title">从 CC Switch 导入</h2>
        <Button variant="secondary" disabled={busy} onClick={onScan}>
          扫描 CC Switch（只读）
        </Button>
      </div>
      {scan && (
        <div className="asb-ccscan">
          {rows.length === 0 ? (
            <p className="asb-empty">CC Switch 中没有供应商。</p>
          ) : (
            <Table
              columns={columns}
              rows={rows}
              rowKey={(row) => row.key}
              ariaLabel="CC Switch 扫描结果"
            />
          )}
          <div className="asb-form-actions">
            <Button
              variant="primary"
              disabled={busy || selectedCount === 0}
              onClick={onImport}
            >
              导入所选 {selectedCount} 项
            </Button>
          </div>
        </div>
      )}
      {result && (
        <div className="asb-banner asb-banner-ok" role="status" aria-label="导入结果">
          <span>
            已导入 {result.importedCount} 项
            {result.usageScriptImportedCount > 0 &&
              ` · 已导入用量脚本 ${result.usageScriptImportedCount} 项`}
            {result.skippedExisting.length > 0 &&
              ` · 跳过已存在 ${result.skippedExisting.length} 项`}
            {result.notImported.length > 0 && ` · 未导入 ${result.notImported.length} 项`}
          </span>
        </div>
      )}
      {result && result.notImported.length > 0 && (
        <div className="asb-ccscan">
          {result.notImported.map((skip) => (
            <div className="asb-kv" key={skip.key}>
              <span className="asb-kv-label">{skip.name}</span>
              <span className="asb-kv-value asb-warn-text">{skip.reason}</span>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
