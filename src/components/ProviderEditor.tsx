import { useEffect, useRef, useState } from "react";
import { fetchProviderModels } from "../api/client";
import type {
  AppKind,
  CodexModelSettings,
  ClaudeModelSettings,
  ModelOptions,
  ProviderDraft,
  ProviderModel,
  ProviderProfile,
} from "../api/client";
import { clientName } from "../lib/client-name";
import { Checkbox } from "./Checkbox";
import { ClientLogo } from "./ClientLogo";
import { EyeOffIcon, PreviewIcon } from "./icons";
import { Input } from "./Input";
import { ModelPicker } from "./ModelPicker";
import { OfficialLoginPanel } from "./OfficialLoginPanel";
import { ProbePanel } from "./ProbePanel";
import { RadioOption } from "./RadioOption";
import { Button } from "./Button";
import { Select } from "./Select";
import { Textarea } from "./Textarea";
import { normalizeUsageQuery } from "../lib/usage-query";

interface Props {
  profile: ProviderProfile | null;
  initialApp: AppKind;
  busy: boolean;
  /** Clients that already own their single official profile. */
  officialTakenApps: AppKind[];
  /** Model currently live in the client's real configuration, shown for
   * context while editing; null means nothing to display. */
  activeModel: string | null;
  onSave: (draft: ProviderDraft) => void;
  onCancel: () => void;
}

// Reasoning effort, summary, and verbosity live on the general-settings
// page, not on profiles.

const CONTEXT_WINDOW_1M = 1_000_000;

function draftFrom(profile: ProviderProfile | null, initialApp: AppKind): ProviderDraft {
  if (profile) {
    return {
      app: profile.app,
      routeMode: profile.routeMode,
      name: profile.name,
      model: profile.model,
      baseUrl: profile.baseUrl,
      apiKey: profile.apiKey,
      modelOptions: profile.modelOptions,
      notes: profile.notes ?? null,
      websiteUrl: profile.websiteUrl,
      usageQuery: profile.usageQuery ?? null,
    };
  }
  return {
    app: initialApp,
    routeMode: "custom",
    name: "",
    model: null,
    baseUrl: null,
    apiKey: "",
    modelOptions: null,
    notes: null,
    websiteUrl: null,
    usageQuery: null,
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
      : {
          primaryOneM: false,
          haikuModel: null,
          sonnetModel: null,
          sonnetOneM: false,
          opusModel: null,
          opusOneM: false,
          availableModels: null,
        };
  return { kind: "claude", ...base, ...patch };
}

function codexOptionsAreEmpty(options: ModelOptions | null): boolean {
  return !options || (options.kind === "codex" && options.contextWindow === null);
}

/** The local profile editor; it never edits client configuration directly. */
export function ProviderEditor({
  profile,
  initialApp,
  busy,
  officialTakenApps,
  activeModel,
  onSave,
  onCancel,
}: Props) {
  const [draft, setDraft] = useState<ProviderDraft>(() => draftFrom(profile, initialApp));
  const [models, setModels] = useState<ProviderModel[] | null>(null);
  const [modelsBusy, setModelsBusy] = useState(false);
  const [modelsError, setModelsError] = useState<string | null>(null);
  const [apiKeyVisible, setApiKeyVisible] = useState(false);
  /** True once the official login panel reported a completed login. */
  const [loginDone, setLoginDone] = useState(false);
  const modelsVersion = useRef(0);
  const baseUrl = draft.baseUrl?.trim() ?? "";

  useEffect(() => {
    setDraft(draftFrom(profile, initialApp));
    setApiKeyVisible(false);
    setLoginDone(false);
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
      const fetched = await fetchProviderModels(baseUrl, draft.apiKey);
      if (modelsVersion.current === version) setModels(fetched);
    } catch (caught) {
      if (modelsVersion.current === version) {
        setModelsError((caught as { message?: string }).message ?? "无法获取模型列表");
      }
    } finally {
      if (modelsVersion.current === version) setModelsBusy(false);
    }
  };

  const codex = draft.app === "codex";
  const official = draft.routeMode === "official";
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
          usageQuery: normalizeUsageQuery(draft.usageQuery),
        });
      }}
    >
      <label className="asb-field">
        <span>客户端</span>
        <div className="asb-client-control">
          <ClientLogo app={draft.app} className="asb-edit-logo" />
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
              setLoginDone(false);
            }}
          />
        </div>
      </label>
      {!profile && (
        <div className="asb-field">
          <span>接入方式</span>
          <div className="asb-segments" role="radiogroup" aria-label="接入方式">
            <RadioOption
              name="access-mode"
              checked={!official}
              disabled={busy}
              label="自定义 API 中继"
              onChange={() => {
                setDraft((current) => ({ ...current, routeMode: "custom" }));
                setLoginDone(false);
              }}
            />
            <RadioOption
              name="access-mode"
              checked={official}
              disabled={busy || officialTakenApps.includes(draft.app)}
              label="官方登录"
              onChange={() => {
                // The official contract is credential-free, so the custom
                // fields are dropped the moment the mode is chosen instead of
                // being carried hidden and stripped at submit time.
                setDraft((current) => ({
                  ...current,
                  routeMode: "official",
                  name: current.name.trim() || `${clientName(current.app)} 官方登录`,
                  model: null,
                  baseUrl: null,
                  apiKey: "",
                  modelOptions: null,
                  usageQuery: null,
                }));
                setLoginDone(false);
              }}
            />
          </div>
        </div>
      )}
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
      {!official && (
      <div className="asb-field">
        <span>服务地址</span>
        <Input
          aria-label="服务地址"
          type="url"
          required
          value={draft.baseUrl ?? ""}
          disabled={busy}
          onChange={(event) => setDraft((current) => ({ ...current, baseUrl: event.target.value }))}
        />
      </div>
      )}
      {!official && (
      <div className="asb-field">
        <span>API 密钥</span>
        <div className="asb-secret-control">
          <Input
            aria-label="API 密钥"
            type={apiKeyVisible ? "text" : "password"}
            required
            value={draft.apiKey}
            disabled={busy}
            onChange={(event) =>
              setDraft((current) => ({ ...current, apiKey: event.target.value }))
            }
          />
          <Button
            variant="secondary"
            aria-pressed={apiKeyVisible}
            disabled={busy}
            onClick={() => setApiKeyVisible((current) => !current)}
          >
            {apiKeyVisible ? <EyeOffIcon size={16} /> : <PreviewIcon size={16} />}
            {apiKeyVisible ? "隐藏密钥" : "查看密钥"}
          </Button>
        </div>
      </div>
      )}
      {!official && (
      <div className="asb-field">
        <span>主模型</span>
        <div className="asb-model-control">
          <Input
            aria-label="主模型"
            value={draft.model ?? ""}
            disabled={busy}
            placeholder="（可选）"
            onChange={(event) =>
              setDraft((current) => {
                const model = event.target.value;
                if (
                  !codex &&
                  !model.trim() &&
                  current.modelOptions?.kind === "claude"
                ) {
                  return {
                    ...current,
                    model,
                    modelOptions: { ...current.modelOptions, primaryOneM: false },
                  };
                }
                return { ...current, model };
              })
            }
          />
          {models && (
            <ModelPicker
              models={models}
              current={draft.model}
              ariaLabel="选择模型"
              disabled={busy}
              onSelect={(model) => setDraft((current) => ({ ...current, model }))}
            />
          )}
          {codex ? (
            <Checkbox
              label="1M"
              ariaLabel="启用 1M 上下文窗口"
              checked={codexSettings?.contextWindow === CONTEXT_WINDOW_1M}
              disabled={busy}
              onChange={(checked) =>
                setDraft((current) => ({
                  ...current,
                  modelOptions: codexOptions(current.modelOptions, {
                    contextWindow: checked ? CONTEXT_WINDOW_1M : null,
                  }),
                }))
              }
            />
          ) : (
            <Checkbox
              label="1M"
              ariaLabel="主模型启用 1M 上下文"
              checked={claudeSettings?.primaryOneM ?? false}
              disabled={busy || !draft.model?.trim()}
              onChange={(enabled) =>
                setDraft((current) => ({
                  ...current,
                  modelOptions: claudeOptions(current.modelOptions, { primaryOneM: enabled }),
                }))
              }
            />
          )}
          <div className="asb-model-actions">
            <ProbePanel url={draft.baseUrl?.trim() || null} />
            <Button
              variant="secondary"
              disabled={busy || modelsBusy || !baseUrl}
              onClick={() => void fetchModels()}
            >
              {modelsBusy ? "获取中…" : "获取模型"}
            </Button>
          </div>
        </div>
        {activeModel && <p className="asb-scope-note">当前启用模型：{activeModel}</p>}
        {modelsError && <span className="asb-warn-text">{modelsError}</span>}
      </div>
      )}

      {official && (
        <fieldset className="asb-fieldset">
          <legend>官方登录</legend>
          <OfficialLoginPanel app={draft.app} onFinished={setLoginDone} />
        </fieldset>
      )}

      {!official && !codex && (
        <fieldset className="asb-fieldset">
          <legend>模型映射</legend>
          <div className="asb-field">
            <span>Haiku 档</span>
            <div className="asb-input-with-picker">
              <Input
                code
                aria-label="Haiku 档"
                value={claudeSettings?.haikuModel ?? ""}
                disabled={busy}
                onChange={(event) =>
                  setDraft((current) => ({
                    ...current,
                    modelOptions: claudeOptions(current.modelOptions, {
                      haikuModel: optional(event.target.value),
                    }),
                  }))
                }
              />
              {models && (
                <ModelPicker
                  models={models}
                  current={claudeSettings?.haikuModel ?? null}
                  ariaLabel="选择 Haiku 档模型"
                  disabled={busy}
                  onSelect={(haikuModel) =>
                    setDraft((current) => ({
                      ...current,
                      modelOptions: claudeOptions(current.modelOptions, { haikuModel }),
                    }))
                  }
                />
              )}
            </div>
          </div>
          <div className="asb-field">
            <span>Sonnet 档</span>
            <div className="asb-input-with-picker">
              <Input
                code
                aria-label="Sonnet 档"
                value={claudeSettings?.sonnetModel ?? ""}
                disabled={busy}
                onChange={(event) =>
                  setDraft((current) => {
                    const sonnetModel = optional(event.target.value);
                    return {
                      ...current,
                      modelOptions: claudeOptions(current.modelOptions, {
                        sonnetModel,
                        ...(sonnetModel ? {} : { sonnetOneM: false }),
                      }),
                    };
                  })
                }
              />
              {models && (
                <ModelPicker
                  models={models}
                  current={claudeSettings?.sonnetModel ?? null}
                  ariaLabel="选择 Sonnet 档模型"
                  disabled={busy}
                  onSelect={(sonnetModel) =>
                    setDraft((current) => ({
                      ...current,
                      modelOptions: claudeOptions(current.modelOptions, { sonnetModel }),
                    }))
                  }
                />
              )}
              <Checkbox
                label="1M"
                ariaLabel="Sonnet 档启用 1M 上下文"
                checked={claudeSettings?.sonnetOneM ?? false}
                disabled={busy || !claudeSettings?.sonnetModel?.trim()}
                onChange={(enabled) =>
                  setDraft((current) => ({
                    ...current,
                    modelOptions: claudeOptions(current.modelOptions, { sonnetOneM: enabled }),
                  }))
                }
              />
            </div>
          </div>
          <div className="asb-field">
            <span>Opus 档</span>
            <div className="asb-input-with-picker">
              <Input
                code
                aria-label="Opus 档"
                value={claudeSettings?.opusModel ?? ""}
                disabled={busy}
                onChange={(event) =>
                  setDraft((current) => {
                    const opusModel = optional(event.target.value);
                    return {
                      ...current,
                      modelOptions: claudeOptions(current.modelOptions, {
                        opusModel,
                        ...(opusModel ? {} : { opusOneM: false }),
                      }),
                    };
                  })
                }
              />
              {models && (
                <ModelPicker
                  models={models}
                  current={claudeSettings?.opusModel ?? null}
                  ariaLabel="选择 Opus 档模型"
                  disabled={busy}
                  onSelect={(opusModel) =>
                    setDraft((current) => ({
                      ...current,
                      modelOptions: claudeOptions(current.modelOptions, { opusModel }),
                    }))
                  }
                />
              )}
              <Checkbox
                label="1M"
                ariaLabel="Opus 档启用 1M 上下文"
                checked={claudeSettings?.opusOneM ?? false}
                disabled={busy || !claudeSettings?.opusModel?.trim()}
                onChange={(enabled) =>
                  setDraft((current) => ({
                    ...current,
                    modelOptions: claudeOptions(current.modelOptions, { opusOneM: enabled }),
                  }))
                }
              />
            </div>
          </div>
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

      <div className="asb-form-actions">
        <Button variant="secondary" disabled={busy} onClick={onCancel}>
          取消
        </Button>
        {/* A new official profile needs a completed login so it never saves
            credential-free; editing one saves right away — the credentials
            live in the client cache and a profile write never touches them. */}
        <Button
          type="submit"
          variant="primary"
          disabled={busy || (official && !profile && !loginDone)}
        >
          保存供应商
        </Button>
      </div>
    </form>
  );
}
