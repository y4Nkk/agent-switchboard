import { useEffect, useState } from "react";
import type {
  AppKind,
  CodexModelSettings,
  ClaudeModelSettings,
  ModelOptions,
  ProviderDraft,
  ProviderProfile,
} from "../api/client";
import { ProbePanel } from "./ProbePanel";

interface Props {
  profile: ProviderProfile | null;
  initialApp: AppKind;
  busy: boolean;
  onSave: (draft: ProviderDraft) => void;
  onCancel: () => void;
}

const CODEX_EFFORTS = [
  { value: "minimal", label: "极低" },
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
  { value: "xhigh", label: "极高" },
];
const CODEX_SUMMARIES = [
  { value: "none", label: "不生成" },
  { value: "auto", label: "自动" },
  { value: "concise", label: "简洁" },
  { value: "detailed", label: "详细" },
];
const CODEX_VERBOSITIES = [
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
];

function draftFrom(profile: ProviderProfile | null, initialApp: AppKind): ProviderDraft {
  if (profile) {
    return {
      app: profile.app,
      mode: profile.mode,
      name: profile.name,
      model: profile.model,
      baseUrl: profile.baseUrl,
      envKey: profile.envKey,
      modelOptions: profile.modelOptions,
    };
  }
  return {
    app: initialApp,
    mode: "custom",
    name: "",
    model: null,
    baseUrl: null,
    envKey: null,
    modelOptions: null,
  };
}

function optional(value: string): string | null {
  const normalized = value.trim();
  return normalized || null;
}

function codexOptions(
  current: ModelOptions | null,
  patch: Partial<CodexModelSettings>,
): ModelOptions {
  const base: CodexModelSettings =
    current?.kind === "codex"
      ? current
      : { reasoningEffort: null, reasoningSummary: null, verbosity: null, contextWindow: null };
  return { kind: "codex", ...base, ...patch };
}

function claudeOptions(
  current: ModelOptions | null,
  patch: Partial<ClaudeModelSettings>,
): ModelOptions {
  const base: ClaudeModelSettings =
    current?.kind === "claude"
      ? current
      : { haikuModel: null, sonnetModel: null, opusModel: null, availableModels: null };
  return { kind: "claude", ...base, ...patch };
}

function codexOptionsAreEmpty(options: ModelOptions | null): boolean {
  return (
    !options ||
    (options.kind === "codex" &&
      options.reasoningEffort === null &&
      options.reasoningSummary === null &&
      options.verbosity === null &&
      options.contextWindow === null)
  );
}

/** The local profile editor; it never edits client configuration directly. */
export function ProviderEditor({ profile, initialApp, busy, onSave, onCancel }: Props) {
  const [draft, setDraft] = useState<ProviderDraft>(() => draftFrom(profile, initialApp));

  useEffect(() => {
    setDraft(draftFrom(profile, initialApp));
  }, [profile?.id, initialApp]);

  const codex = draft.app === "codex";
  const custom = draft.mode === "custom";
  const codexSettings = draft.modelOptions?.kind === "codex" ? draft.modelOptions : null;
  const claudeSettings = draft.modelOptions?.kind === "claude" ? draft.modelOptions : null;

  return (
    <form
      className="asb-form"
      aria-label={profile ? "编辑供应商" : "新建供应商"}
      onSubmit={(event) => {
        event.preventDefault();
        const modelOptions = codexOptionsAreEmpty(draft.modelOptions) ? null : draft.modelOptions;
        onSave({
          ...draft,
          name: draft.name.trim(),
          model: optional(draft.model ?? ""),
          baseUrl: custom ? optional(draft.baseUrl ?? "") : null,
          envKey: codex && custom ? optional(draft.envKey ?? "") : null,
          modelOptions,
        });
      }}
    >
      <fieldset className="asb-fieldset">
        <legend>路由模式</legend>
        <div className="asb-radio-row" role="radiogroup" aria-label="路由模式">
          <label className="asb-field-check">
            <input
              type="radio"
              name="asb-mode"
              checked={draft.mode === "custom"}
              disabled={busy}
              onChange={() =>
                setDraft((current) => ({
                  ...current,
                  mode: "custom",
                  envKey: current.app === "codex" ? current.envKey : null,
                }))
              }
            />
            <span>自定义服务</span>
          </label>
          <label className="asb-field-check">
            <input
              type="radio"
              name="asb-mode"
              checked={draft.mode === "official"}
              disabled={busy}
              onChange={() =>
                setDraft((current) => ({ ...current, mode: "official", baseUrl: null, envKey: null }))
              }
            />
            <span>官方登录</span>
          </label>
        </div>
      </fieldset>

      <label className="asb-field">
        <span>客户端</span>
        <select
          className="asb-input"
          value={draft.app}
          disabled={Boolean(profile) || busy}
          onChange={(event) => {
            const app = event.target.value as AppKind;
            setDraft((current) => ({
              ...current,
              app,
              envKey: app === "codex" ? current.envKey : null,
              modelOptions: null,
            }));
          }}
        >
          <option value="codex">Codex</option>
          <option value="claude">Claude</option>
        </select>
      </label>
      <label className="asb-field">
        <span>名称</span>
        <input
          className="asb-input"
          value={draft.name}
          required
          disabled={busy}
          onChange={(event) => setDraft((current) => ({ ...current, name: event.target.value }))}
        />
      </label>
      {custom && (
        <label className="asb-field">
          <span>服务地址</span>
          <input
            className="asb-input"
            type="url"
            required
            value={draft.baseUrl ?? ""}
            disabled={busy}
            onChange={(event) => setDraft((current) => ({ ...current, baseUrl: event.target.value }))}
          />
        </label>
      )}
      <label className="asb-field">
        <span>主模型</span>
        <input
          className="asb-input"
          value={draft.model ?? ""}
          disabled={busy}
          placeholder={custom ? "（可选）" : "（可选）"}
          onChange={(event) => setDraft((current) => ({ ...current, model: event.target.value }))}
        />
      </label>
      {codex && custom && (
        <label className="asb-field">
          <span>环境变量名</span>
          <input
            className="asb-input asb-code"
            value={draft.envKey ?? ""}
            disabled={busy}
            onChange={(event) => setDraft((current) => ({ ...current, envKey: event.target.value }))}
          />
        </label>
      )}

      {codex && (
        <fieldset className="asb-fieldset">
          <legend>模型运行参数</legend>
          <label className="asb-field">
            <span className="asb-kv-label">推理强度</span>
            <select
              className="asb-input"
              value={codexSettings?.reasoningEffort ?? ""}
              disabled={busy}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  modelOptions: codexOptions(current.modelOptions, {
                    reasoningEffort: event.target.value || null,
                  }),
                }))
              }
            >
              <option value="">（不设置）</option>
              {CODEX_EFFORTS.map(({ value, label }) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <label className="asb-field">
            <span className="asb-kv-label">推理摘要</span>
            <select
              className="asb-input"
              value={codexSettings?.reasoningSummary ?? ""}
              disabled={busy}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  modelOptions: codexOptions(current.modelOptions, {
                    reasoningSummary: event.target.value || null,
                  }),
                }))
              }
            >
              <option value="">（不设置）</option>
              {CODEX_SUMMARIES.map(({ value, label }) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <label className="asb-field">
            <span className="asb-kv-label">输出详细程度</span>
            <select
              className="asb-input"
              value={codexSettings?.verbosity ?? ""}
              disabled={busy}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  modelOptions: codexOptions(current.modelOptions, {
                    verbosity: event.target.value || null,
                  }),
                }))
              }
            >
              <option value="">（不设置）</option>
              {CODEX_VERBOSITIES.map(({ value, label }) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <label className="asb-field">
            <span className="asb-kv-label">上下文窗口</span>
            <input
              className="asb-input asb-code"
              type="number"
              min={1}
              value={codexSettings?.contextWindow ?? ""}
              disabled={busy}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  modelOptions: codexOptions(current.modelOptions, {
                    contextWindow: event.target.value ? Number(event.target.value) : null,
                  }),
                }))
              }
            />
          </label>
        </fieldset>
      )}

      {!codex && (
        <fieldset className="asb-fieldset">
          <legend>模型映射</legend>
          <label className="asb-field">
            <span>Haiku 档</span>
            <input
              className="asb-input asb-code"
              value={claudeSettings?.haikuModel ?? ""}
              disabled={busy}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  modelOptions: claudeOptions(current.modelOptions, {
                    haikuModel: event.target.value || null,
                  }),
                }))
              }
            />
          </label>
          <label className="asb-field">
            <span>Sonnet 档</span>
            <input
              className="asb-input asb-code"
              value={claudeSettings?.sonnetModel ?? ""}
              disabled={busy}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  modelOptions: claudeOptions(current.modelOptions, {
                    sonnetModel: event.target.value || null,
                  }),
                }))
              }
            />
          </label>
          <label className="asb-field">
            <span>Opus 档</span>
            <input
              className="asb-input asb-code"
              value={claudeSettings?.opusModel ?? ""}
              disabled={busy}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  modelOptions: claudeOptions(current.modelOptions, {
                    opusModel: event.target.value || null,
                  }),
                }))
              }
            />
          </label>
          <label className="asb-field">
            <span>可选模型列表（每行一个）</span>
            <textarea
              className="asb-input asb-code asb-textarea"
              rows={3}
              value={(claudeSettings?.availableModels ?? []).join("\n")}
              disabled={busy}
              onChange={(event) =>
                setDraft((current) => {
                  const lines = event.target.value
                    .split("\n")
                    .map((line) => line.trim())
                    .filter(Boolean);
                  return {
                    ...current,
                    modelOptions: claudeOptions(current.modelOptions, {
                      availableModels: lines.length > 0 ? lines : null,
                    }),
                  };
                })
              }
            />
          </label>
        </fieldset>
      )}

      {custom && <ProbePanel url={draft.baseUrl?.trim() || null} />}

      <div className="asb-form-actions">
        <button type="button" className="asb-btn-secondary" disabled={busy} onClick={onCancel}>
          取消
        </button>
        <button type="submit" className="asb-btn-primary" disabled={busy}>
          保存供应商
        </button>
      </div>
    </form>
  );
}
