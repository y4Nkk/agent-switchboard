import { usesClaudeManagedAuth } from "../../api/claude-accounts";
import type { UpstreamProtocol } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientName } from "../../lib/client-name";
import { requiresGateway } from "../../lib/protocol";
import { Input } from "../Input";
import { Select } from "../Select";
import { applyProviderPreset } from "./draft";
import { providerPreset } from "../../lib/provider-catalog";
import { ProviderCredentialField } from "./ProviderCredentialField";
import { ProviderEndpointField } from "./ProviderEndpointField";
import type { ProviderEditorState } from "./useProviderEditor";

interface Props { editor: ProviderEditorState; busy: boolean; autoFill?: boolean }

function ProtocolField({ editor, busy }: Props) {
  const { t } = useI18n();
  const { draft, setDraft } = editor;
  return (
    <div className="asb-field">
      <span>{t("providers.label.apiFormat")}</span>
      <Select ariaLabel={t("providers.label.apiFormat")} value={draft.upstreamProtocol} disabled={busy}
        options={[
          { value: "anthropicMessages", label: "Anthropic Messages (/v1/messages)" },
          { value: "chatCompletions", label: "Chat Completions (/chat/completions)" },
          { value: "responses", label: "Responses (/responses)" },
          { value: "geminiGenerateContent", label: "Gemini Native (generateContent)" },
        ]}
        onChange={(value) => {
          const upstreamProtocol = value as UpstreamProtocol;
          setDraft((current) => ({
            ...current, upstreamProtocol,
            authentication: upstreamProtocol === "geminiGenerateContent" && current.authentication === "xApiKey"
              || upstreamProtocol !== "geminiGenerateContent" && current.authentication === "xGoogApiKey"
              ? null : current.authentication,
            responsesOptions: upstreamProtocol === "responses"
              ? { requestMode: "standard" } : null,
          }));
        }} />
    </div>
  );
}

function RouteNotice({ editor }: Pick<Props, "editor">) {
  const { t } = useI18n();
  const { draft, connection } = editor;
  return (
    <div className="asb-provider-route-note">
      {draft.connection?.claudeNative ? <p className="asb-scope-note">{t("providers.editor.route.claudeNative", { kind: draft.connection.claudeNative.kind })}</p> : requiresGateway(draft)
        ? <p className="asb-scope-note asb-warn-text">{connection.gatewayRouteWarning}</p>
        : <p className="asb-scope-note">{t("providers.editor.route.direct", { client: clientName(draft.app) })}</p>}
    </div>
  );
}

export function ProviderConnectionFields({ editor, busy, autoFill }: Props) {
  const { t } = useI18n();
  const { draft, setDraft, connection } = editor;
  return (
    <section className="asb-editor-section" aria-label={t("providers.editor.section.connection")}>
      <h3 className="asb-section-title">{t("providers.editor.section.connection")}</h3>
      <div className="asb-editor-section-fields">
        <div className="asb-provider-connection-choice">
          <ProtocolField editor={editor} busy={busy || !!draft.connection?.claudeNative} />
        </div>
        <ProviderEndpointField busy={busy} required={!draft.connection?.claudeNative} baseUrl={draft.baseUrl} protocol={draft.upstreamProtocol}
          endpoints={connection.endpoints} endpointError={connection.endpointError}
          resolvingEndpoint={connection.resolvingEndpoint}
          onChange={(value) => setDraft((current) => {
            const preset = autoFill && !current.model ? providerPreset(value) : undefined;
            return preset ? { ...applyProviderPreset(current, preset), baseUrl: value } : { ...current, baseUrl: value };
          })} />
        <ProviderCredentialField key={`credential-${draft.app}`} busy={busy}
          value={draft.apiKey} protocol={draft.upstreamProtocol}
          authentication={draft.authentication}
          required={!usesClaudeManagedAuth(draft.connection) && !draft.connection?.claudeNative}
          onChange={(value) => setDraft((current) => ({ ...current, apiKey: value }))} />
        {!draft.connection?.claudeNative && !usesClaudeManagedAuth(draft.connection) && <label className="asb-field">
          <span>{t("providers.editor.modelsUrl")}</span>
          <Input type="url" value={draft.connection?.modelsUrl ?? ""} disabled={busy}
            placeholder={draft.connection?.isFullUrl ? t("providers.editor.modelsUrlRequired") : t("providers.editor.optional")}
            onChange={(event) => setDraft((current) => ({
              ...current,
              connection: { ...current.connection, modelsUrl: event.target.value.trim() || null },
            }))} />
          <p className="asb-scope-note">{t("providers.editor.modelsUrlNote")}</p>
        </label>}
        <RouteNotice editor={editor} />
      </div>
    </section>
  );
}
