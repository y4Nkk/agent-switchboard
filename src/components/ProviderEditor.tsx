import { useEffect, useRef, useState } from "react";
import { fetchProviderModels } from "../api/client";
import type {
  AppKind,
  CodexModelSettings,
  ClaudeModelSettings,
  ModelOptions,
  ProviderDraft,
  ProviderProfile,
} from "../api/client";
import { Input } from "./Input";
import { ProbePanel } from "./ProbePanel";
import { Select } from "./Select";
import { Textarea } from "./Textarea";

interface Props {
  profile: ProviderProfile | null;
  initialApp: AppKind;
  busy: boolean;
  onSave: (draft: ProviderDraft) => void;
  onCancel: () => void;
}

// Reasoning effort, summary, and verbosity live on the general-settings
// page, not on profiles.

// Radix rejects empty-string option values, so the picker's "no model"
// choice uses this sentinel; it maps back to null on change.
const MODEL_NONE = "__none__";
const MODEL_PLACEHOLDER_LABEL = "（从获取列表选择）";

function draftFrom(profile: ProviderProfile | null, initialApp: AppKind): ProviderDraft {
  // The editor only produces custom-endpoint profiles (user decision
  // 2026-08-28); official login is not a profile kind at all now. A legacy
  // official profile converts to a custom one on save.
  if (profile) {
    return {
      app: profile.app,
      name: profile.name,
      model: profile.model,
      baseUrl: profile.baseUrl,
      apiKey: profile.apiKey,
      modelOptions: profile.modelOptions,
      notes: profile.notes ?? null,
      websiteUrl: profile.websiteUrl ?? null,
    };
  }
  return {
    app: initialApp,
    name: "",
    model: null,
    baseUrl: null,
    apiKey: "",
    modelOptions: null,
    notes: null,
    websiteUrl: null,
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
    current?.kind === "codex" ? current : { contextWindow: null };
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
  return !options || (options.kind === "codex" && options.contextWindow === null);
}

/** The local profile editor; it never edits client configuration directly. */
export function ProviderEditor({ profile, initialApp, busy, onSave, onCancel }: Props) {
  const [draft, setDraft] = useState<ProviderDraft>(() => draftFrom(profile, initialApp));
  const [models, setModels] = useState<string[] | null>(null);
  const [modelsBusy, setModelsBusy] = useState(false);
  const [modelsError, setModelsError] = useState<string | null>(null);
  const modelsVersion = useRef(0);
  const baseUrl = draft.baseUrl?.trim() ?? "";

  useEffect(() => {
    setDraft(draftFrom(profile, initialApp));
  }, [profile?.id, initialApp]);

  useEffect(() => {
    modelsVersion.current += 1;
    setModels(null);
    setModelsError(null);
    setModelsBusy(false);
  }, [baseUrl]);

  const fetchModels = async () => {
    if (modelsBusy || !baseUrl) return;
    const version = modelsVersion.current;
    setModelsBusy(true);
    setModelsError(null);
    try {
      const ids = await fetchProviderModels(baseUrl, draft.apiKey);
      if (modelsVersion.current === version) setModels(ids);
    } catch (caught) {
      if (modelsVersion.current === version) {
        setModelsError((caught as { message?: string }).message ?? "无法获取模型列表");
      }
    } finally {
      if (modelsVersion.current === version) setModelsBusy(false);
    }
  };

  const codex = draft.app === "codex";
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
          baseUrl: optional(draft.baseUrl ?? ""),
          apiKey: draft.apiKey.trim(),
          notes: optional(draft.notes ?? ""),
          websiteUrl: optional(draft.websiteUrl ?? ""),
          modelOptions,
        });
      }}
    >
      <label className="asb-field">
        <span>客户端</span>
        <Select
          ariaLabel="客户端"
          value={draft.app}
          options={[
            { value: "codex", label: "Codex" },
            { value: "claude", label: "Claude" },
          ]}
          disabled={Boolean(profile) || busy}
          onChange={(app) => {
            setDraft((current) => ({
              ...current,
              app: app as AppKind,
              modelOptions: null,
            }));
          }}
        />
      </label>
      <label className="asb-field">
        <span>名称</span>
        <Input
          value={draft.name}
          required
          disabled={busy}
          onChange={(event) => setDraft((current) => ({ ...current, name: event.target.value }))}
        />
      </label>
      <label className="asb-field">
        <span>官网地址</span>
        <Input
          type="url"
          value={draft.websiteUrl ?? ""}
          disabled={busy}
          placeholder="（可选）"
          onChange={(event) =>
            setDraft((current) => ({ ...current, websiteUrl: event.target.value }))
          }
        />
      </label>
      <label className="asb-field">
        <span>备注</span>
        <Textarea
          rows={2}
          value={draft.notes ?? ""}
          disabled={busy}
          placeholder="（可选，仅保存在本应用）"
          onChange={(event) => setDraft((current) => ({ ...current, notes: event.target.value }))}
        />
      </label>
      <label className="asb-field">
        <span>服务地址</span>
        <Input
          type="url"
          required
          value={draft.baseUrl ?? ""}
          disabled={busy}
          onChange={(event) => setDraft((current) => ({ ...current, baseUrl: event.target.value }))}
        />
      </label>
      <div className="asb-field">
        <span>主模型</span>
        <Input
          aria-label="主模型"
          value={draft.model ?? ""}
          disabled={busy}
          placeholder="（可选）"
          onChange={(event) => setDraft((current) => ({ ...current, model: event.target.value }))}
        />
        <div className="asb-field-actions">
          <button
            type="button"
            className="asb-btn-secondary"
            disabled={busy || modelsBusy || !baseUrl}
            onClick={() => void fetchModels()}
          >
            {modelsBusy ? "获取中…" : "获取模型"}
          </button>
          {modelsError && <span className="asb-warn-text">{modelsError}</span>}
          {models && (
            <Select
              ariaLabel="选择模型"
              value={draft.model}
              placeholder={MODEL_PLACEHOLDER_LABEL}
              options={[
                { value: MODEL_NONE, label: MODEL_PLACEHOLDER_LABEL },
                ...models.map((id) => ({ value: id, label: id })),
              ]}
              disabled={busy}
              onChange={(model) =>
                setDraft((current) => ({
                  ...current,
                  model: model === MODEL_NONE ? null : model,
                }))
              }
            />
          )}
        </div>
      </div>
      <label className="asb-field">
        <span>API 密钥</span>
        <Input
          type="password"
          required
          value={draft.apiKey}
          disabled={busy}
          onChange={(event) =>
            setDraft((current) => ({ ...current, apiKey: event.target.value }))
          }
        />
      </label>

      {codex && (
        <fieldset className="asb-fieldset">
          <legend>模型运行参数</legend>
          <label className="asb-field">
            <span className="asb-kv-label">上下文窗口</span>
            <Input
              code
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
            <Input
              code
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
            <Input
              code
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
            <Input
              code
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
            <Textarea
              code
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

      <ProbePanel url={draft.baseUrl?.trim() || null} />

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
