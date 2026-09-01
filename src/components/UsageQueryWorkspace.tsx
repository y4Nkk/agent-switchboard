import { useEffect, useRef, useState } from "react";
import {
  testUsageQuery,
  type DeclarativeUsageQuery,
  type UsageQuery,
  type UsageReading,
  type UsageSummary,
} from "../api/client";
import { Input } from "./Input";
import { Time } from "./Time";
import { Textarea } from "./Textarea";
import { UsageIcon } from "./icons";
import { normalizeUsageQuery } from "../lib/usage-query";

interface Props {
  providerName: string;
  value: UsageQuery | null;
  apiKey: string;
  baseUrl: string | null;
  busy: boolean;
  onSave: (next: UsageQuery | null) => Promise<boolean> | boolean;
  onClose: () => void;
}

const SCRIPT_TEMPLATE = `({
  request: ({ baseUrl, apiKey }) => ({
    url: \`${"${baseUrl}"}/user/balance\`,
    method: "GET",
    headers: { Authorization: \`Bearer ${"${apiKey}"}\` },
  }),
  extract: ({ body }) => ({
    remaining: body.balance,
    used: null,
    total: null,
    unit: "USD",
  }),
})`;

function emptyDeclarative(): DeclarativeUsageQuery {
  return {
    kind: "declarative",
    url: "",
    remainingPath: null,
    usedPath: null,
    totalPath: null,
    unit: null,
  };
}

function canRun(query: UsageQuery | null): boolean {
  if (!query) return false;
  return query.kind === "declarative" ? Boolean(query.url.trim()) : Boolean(query.source.trim());
}

function optional(raw: string): string | null {
  const value = raw.trim();
  return value || null;
}

function Metric({ label, value, unit }: { label: string; value: number | null; unit: string | null }) {
  return (
    <div className="asb-usage-metric">
      <span>{label}</span>
      <strong>{value ?? "—"}</strong>
      {unit && <small>{unit}</small>}
    </div>
  );
}

function Reading({ reading }: { reading: UsageReading }) {
  return (
    <section className="asb-usage-reading">
      {reading.planName && <h3 className="asb-usage-reading-title">{reading.planName}</h3>}
      <div className="asb-usage-metrics">
        <Metric label="余额" value={reading.remaining} unit={reading.unit} />
        <Metric label="已用" value={reading.used} unit={reading.unit} />
        <Metric label="总量" value={reading.total} unit={reading.unit} />
      </div>
    </section>
  );
}

/**
 * Dedicated settings workspace for one provider's optional usage query. It
 * owns the draft and transient result; its caller owns the persisted profile.
 */
export function UsageQueryWorkspace({
  providerName,
  value,
  apiKey,
  baseUrl,
  busy,
  onSave,
  onClose,
}: Props) {
  const [draft, setDraft] = useState<UsageQuery | null>(() => value);
  const [querying, setQuerying] = useState(false);
  const [saving, setSaving] = useState(false);
  const [summary, setSummary] = useState<UsageSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const queryVersion = useRef(0);
  const firstRun = useRef(true);

  const clearResult = () => {
    queryVersion.current += 1;
    setSummary(null);
    setError(null);
  };

  const run = async () => {
    if (!draft || querying || saving) return;
    const version = ++queryVersion.current;
    setQuerying(true);
    setError(null);
    try {
      const next = await testUsageQuery(draft, apiKey, baseUrl);
      if (queryVersion.current === version) setSummary(next);
    } catch (caught) {
      if (queryVersion.current === version) {
        setSummary(null);
        setError((caught as { message?: string }).message ?? "查询失败");
      }
    } finally {
      if (queryVersion.current === version) setQuerying(false);
    }
  };

  // Entering a configured workspace reads once. Empty optional
  // configurations only open the editor.
  useEffect(() => {
    if (!firstRun.current) return;
    firstRun.current = false;
    if (canRun(draft)) void run();
    // This subview mounts for each explicit open. Subsequent edits require an
    // explicit query, rather than issuing network requests for every keystroke.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !querying && !saving) onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose, querying, saving]);

  const kind = draft?.kind ?? "declarative";
  const declarative = draft?.kind === "declarative" ? draft : emptyDeclarative();

  const selectKind = (next: "declarative" | "script") => {
    if (next === kind) return;
    clearResult();
    setDraft(next === "declarative" ? emptyDeclarative() : { kind: "script", source: "" });
  };

  const patchDeclarative = (fields: Partial<DeclarativeUsageQuery>) => {
    clearResult();
    setDraft({ ...declarative, ...fields, kind: "declarative" });
  };

  const patchScript = (source: string) => {
    clearResult();
    setDraft({ kind: "script", source });
  };

  const save = async () => {
    if (busy || querying || saving) return;
    setSaving(true);
    try {
      await onSave(normalizeUsageQuery(draft));
    } finally {
      setSaving(false);
    }
  };

  const controlsDisabled = busy || querying || saving;

  return (
    <section className="asb-usage-workspace" id="asb-usage-workspace" aria-label="用量查询">
      <header className="asb-usage-workspace-head">
        <button
          type="button"
          className="asb-btn-back"
          aria-label="返回供应商配置"
          disabled={controlsDisabled}
          onClick={onClose}
        >
          ←
        </button>
        <div>
          <h2 className="asb-panel-title">用量查询</h2>
          <p className="asb-usage-provider">{providerName.trim() || "未命名供应商"}</p>
        </div>
      </header>

      <div className="asb-usage-mode" role="tablist" aria-label="查询方式">
        <button
          type="button"
          role="tab"
          aria-selected={kind === "declarative"}
          className={`asb-usage-mode-tab${kind === "declarative" ? " is-active" : ""}`}
          disabled={controlsDisabled}
          onClick={() => selectKind("declarative")}
        >
          字段提取
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={kind === "script"}
          className={`asb-usage-mode-tab${kind === "script" ? " is-active" : ""}`}
          disabled={controlsDisabled}
          onClick={() => selectKind("script")}
        >
          自编脚本
        </button>
      </div>

      {kind === "declarative" ? (
        <div className="asb-usage-config" role="tabpanel" aria-label="字段提取配置">
          <label className="asb-field">
            <span>查询地址</span>
            <Input
              aria-label="用量查询地址"
              value={declarative.url}
              disabled={controlsDisabled}
              placeholder="{{baseUrl}}/user/balance"
              onChange={(event) => patchDeclarative({ url: event.target.value })}
            />
          </label>
          <div className="asb-usage-paths">
            <label className="asb-field">
              <span>余额路径</span>
              <Input
                code
                aria-label="余额提取路径"
                value={declarative.remainingPath ?? ""}
                disabled={controlsDisabled}
                placeholder="data/balance"
                onChange={(event) => patchDeclarative({ remainingPath: optional(event.target.value) })}
              />
            </label>
            <label className="asb-field">
              <span>已用路径</span>
              <Input
                code
                aria-label="已用提取路径"
                value={declarative.usedPath ?? ""}
                disabled={controlsDisabled}
                placeholder="data/used"
                onChange={(event) => patchDeclarative({ usedPath: optional(event.target.value) })}
              />
            </label>
            <label className="asb-field">
              <span>总量路径</span>
              <Input
                code
                aria-label="总量提取路径"
                value={declarative.totalPath ?? ""}
                disabled={controlsDisabled}
                placeholder="data/total"
                onChange={(event) => patchDeclarative({ totalPath: optional(event.target.value) })}
              />
            </label>
            <label className="asb-field">
              <span>单位</span>
              <Input
                aria-label="用量单位"
                value={declarative.unit ?? ""}
                disabled={controlsDisabled}
                placeholder="USD"
                onChange={(event) => patchDeclarative({ unit: optional(event.target.value) })}
              />
            </label>
          </div>
          <p className="asb-scope-note">
            以一次 GET 请求读取 JSON；地址可使用 {"{{baseUrl}}"} 与 {"{{apiKey}}"}。
          </p>
        </div>
      ) : (
        <div className="asb-usage-config" role="tabpanel" aria-label="自编脚本配置">
          <label className="asb-field">
            <span>用量查询脚本</span>
            <Textarea
              code
              aria-label="用量查询脚本"
              rows={16}
              value={draft?.kind === "script" ? draft.source : ""}
              disabled={controlsDisabled}
              placeholder={SCRIPT_TEMPLATE}
              spellCheck={false}
              onChange={(event) => patchScript(event.target.value)}
            />
          </label>
          <div className="asb-usage-script-contract">
            <span>输入</span>
            <code>{"request({ baseUrl, apiKey })"}</code>
            <span>输出</span>
            <code>{"extract({ body, status })"}</code>
          </div>
          <p className="asb-scope-note">
            脚本只能生成一次 GET / POST 请求并提取 JSON 数值；网络请求由应用执行。
          </p>
        </div>
      )}

      <div className="asb-usage-actions">
        <button
          type="button"
          className="asb-btn-secondary asb-usage-run"
          disabled={controlsDisabled || !canRun(draft)}
          onClick={() => void run()}
        >
          <UsageIcon />
          {querying ? "查询中…" : "查询用量"}
        </button>
        <button
          type="button"
          className="asb-btn-primary"
          disabled={controlsDisabled}
          onClick={() => void save()}
        >
          {saving ? "保存中…" : "保存查询"}
        </button>
      </div>

      {summary && (
        <section className="asb-usage-readout" aria-label="本次用量结果">
          <div className="asb-usage-readout-head">
            <span>本次结果</span>
            <Time iso={summary.at} />
          </div>
          {summary.readings.map((reading, index) => (
            <Reading key={`${reading.planName ?? "默认"}-${index}`} reading={reading} />
          ))}
        </section>
      )}
      {error && <p className="asb-warn-text" role="alert">{error}</p>}
    </section>
  );
}
