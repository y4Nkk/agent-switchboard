import { useMessageState } from "../../i18n/use-message-state";
import { uiMessage } from "../../i18n/errors";
import { useState } from "react";
import type { CodexUpstream } from "../../api/client";
import type { ProviderConnectionOptions } from "../../api/providers";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { requiresGateway } from "../../lib/protocol";
import { Input } from "../Input";
import { Select } from "../Select";
import { Textarea } from "../Textarea";
import { ProviderCredentialField } from "../provider-editor/ProviderCredentialField";
import { ProviderEndpointField } from "../provider-editor/ProviderEndpointField";
import { reconcileCodexUpstream } from "./draft";
import type { CodexEditorState } from "./useCodexProviderEditor";

interface Props { editor: CodexEditorState; busy: boolean }

function ProtocolField({ editor, busy }: Props) {
  const { t } = useI18n();
  const { draft, setDraft } = editor;
  return (
    <label className="asb-field">
      <span>{t("codex.connection.apiFormat")}</span>
      <Select ariaLabel={t("codex.connection.apiFormat")} value={draft.upstream} disabled={busy}
        options={[
          { value: "responses", label: "Responses (/responses)" },
          { value: "chatCompletions", label: "Chat Completions (/chat/completions)" },
          { value: "anthropicMessages", label: "Anthropic Messages (/v1/messages)" },
        ]}
        onChange={(value) => setDraft((current) =>
          reconcileCodexUpstream(current, value as CodexUpstream))} />
    </label>
  );
}

function OverrideFields({ editor, busy }: Props) {
  const { t } = useI18n();
  const { draft, setDraft } = editor;
  const overrides = draft.connection.localProxyRequestOverrides ?? null;
  const [headers, setHeaders] = useState(() =>
    JSON.stringify(overrides?.headers ?? {}, null, 2));
  const [body, setBody] = useState(() =>
    JSON.stringify(overrides?.body ?? {}, null, 2));
  const [problem, setProblem] = useMessageState();
  const update = (patch: Partial<ProviderConnectionOptions>) =>
    setDraft((current) => ({ ...current, connection: { ...current.connection, ...patch } }));
  const applyOverrides = (parsedHeaders: Record<string, string>, parsedBody: Record<string, unknown>) => {
    const emptyHeaders = Object.keys(parsedHeaders).length === 0;
    const emptyBody = Object.keys(parsedBody).length === 0;
    update({ localProxyRequestOverrides: emptyHeaders && emptyBody ? null : {
      headers: parsedHeaders,
      body: emptyBody ? null : parsedBody,
    } });
  };
  return (
    <div className="asb-provider-route-note">
      <label className="asb-field"><span>{t("codex.connection.customUserAgent")}</span>
        <Input value={draft.connection.customUserAgent ?? ""} disabled={busy}
          onChange={(e) => update({ customUserAgent: e.target.value || null })} /></label>
      <label className="asb-field"><span>{t("codex.connection.headersOverride")}</span>
        <Textarea code value={headers} disabled={busy}
          onChange={(e) => {
            setHeaders(e.target.value);
            try {
              const parsed = JSON.parse(e.target.value);
              if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)
                || Object.values(parsed).some((value) => typeof value !== "string")) {
                throw uiMessage("codex.connection.headersMustBeObject");
              }
              setProblem(null);
              applyOverrides(parsed, parseBody(body));
            } catch (error) {
              setProblem(error);
            }
          }} /></label>
      <label className="asb-field"><span>{t("codex.connection.bodyOverride")}</span>
        <Textarea code value={body} disabled={busy}
          onChange={(e) => {
            setBody(e.target.value);
            try {
              const parsed = parseBody(e.target.value);
              setProblem(null);
              applyOverrides(parseHeaders(headers), parsed);
            } catch (error) {
              setProblem(error);
            }
          }} /></label>
      {problem && <p className="asb-scope-note asb-warn-text">{problem}</p>}
      <p className="asb-scope-note">
        {t("codex.connection.overrideNote")}
      </p>
    </div>
  );
}

function parseHeaders(raw: string): Record<string, string> {
  const parsed = JSON.parse(raw);
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)
    || Object.values(parsed).some((value) => typeof value !== "string")) {
    throw uiMessage("codex.connection.headersMustBeObject");
  }
  return parsed;
}

function parseBody(raw: string): Record<string, unknown> {
  const text = raw.trim();
  if (text === "" || text === "{}" || text === "null") return {};
  const parsed: unknown = JSON.parse(text);
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw uiMessage("codex.connection.bodyMustBeObject");
  }
  return parsed as Record<string, unknown>;
}

export function CodexConnectionFields({ editor, busy }: Props) {
  const { t } = useI18n();
  const { draft, setDraft, connection } = editor;
  const gatewayRoute = requiresGateway({
    app: "codex",
    routeMode: "custom",
    baseUrl: draft.endpoint,
    upstreamProtocol: draft.upstream,
    responsesOptions: draft.upstream === "responses" ? { requestMode: draft.requestMode } : null,
    connection: draft.connection,
    subagentRoute: draft.subagentRoute,
    modelRoutes: draft.modelRoutes,
    authentication: draft.authentication,
  });
  return (
    <section className="asb-editor-section" aria-label={t("codex.connection.title")}>
      <h3 className="asb-section-title">{t("codex.connection.title")}</h3>
      <div className="asb-editor-section-fields">
        <div className="asb-provider-connection-choice">
          <ProtocolField editor={editor} busy={busy} />
        </div>
        <ProviderEndpointField busy={busy} baseUrl={draft.endpoint} protocol={draft.upstream}
          endpoints={connection.endpoints} endpointError={connection.endpointError}
          resolvingEndpoint={connection.resolvingEndpoint}
          onChange={(value) => setDraft((current) => ({ ...current, endpoint: value }))} />
        <ProviderCredentialField busy={busy} value={draft.apiKey} protocol={draft.upstream}
          authentication={draft.authentication}
          onChange={(value) => setDraft((current) => ({ ...current, apiKey: value }))} />
        <label className="asb-field">
          <span>{t("codex.connection.modelsUrl")}</span>
          <Input type="url" value={draft.connection.modelsUrl ?? ""} disabled={busy}
            placeholder={draft.connection.isFullUrl ? t("codex.connection.modelsUrlRequired") : t("codex.optional")}
            onChange={(event) => setDraft((current) => ({
              ...current,
              connection: { ...current.connection, modelsUrl: event.target.value.trim() || null },
            }))} />
          <p className="asb-scope-note">{t("providers.editor.modelsUrlNote")}</p>
        </label>
        <div className="asb-provider-route-note">
          {gatewayRoute
            ? <p className="asb-scope-note asb-warn-text">{connection.gatewayRouteWarning}</p>
            : <p className="asb-scope-note">
              {t("providers.editor.route.direct", { client: clientName("codex") })}
            </p>}
          {draft.upstream !== "responses" && (
            <p className="asb-scope-note asb-warn-text">
              {t("codex.connection.chatRouteWarning")}
            </p>
          )}
        </div>
        {gatewayRoute && <OverrideFields editor={editor} busy={busy} />}
      </div>
    </section>
  );
}
