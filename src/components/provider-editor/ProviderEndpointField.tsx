import { Link2 } from "lucide-react";
import { useId } from "react";
import type { ProviderEndpoints, UpstreamProtocol } from "../../api/client";
import { useI18n } from "../../i18n";
import { Input } from "../Input";
import { FactList } from "../FactList";
import { PROTOCOL_NOTES } from "./draft";

interface Props {
  baseUrl: string | null;
  required?: boolean;
  protocol: UpstreamProtocol | null;
  endpoints: ProviderEndpoints | null;
  endpointError: string | null;
  resolvingEndpoint: boolean;
  busy: boolean;
  onChange: (value: string) => void;
}

export function ProviderEndpointField({
  baseUrl, protocol, endpoints, endpointError, resolvingEndpoint, busy, onChange, required = true,
}: Props) {
  const { t } = useI18n();
  const urlId = useId();
  const helpId = `${urlId}-help`;
  const resultId = `${urlId}-result`;
  return (
    <div className="asb-field asb-provider-endpoint">
      <label htmlFor={urlId}>{t("providers.label.serviceAddress")}</label>
      <Input id={urlId} code type="url" required={required} value={baseUrl ?? ""}
        disabled={busy} aria-describedby={`${helpId} ${resultId}`}
        aria-invalid={endpointError ? true : undefined}
        onChange={(event) => onChange(event.target.value)} />
      {protocol && <p className="asb-scope-note" id={helpId}>
        {t(PROTOCOL_NOTES[protocol])}
      </p>}
      <div id={resultId} aria-live="polite">
        {resolvingEndpoint && <p className="asb-scope-note">{t("providers.editor.resolvingEndpoint")}</p>}
        {endpointError && <p className="asb-scope-note asb-warn-text" role="alert">{endpointError}</p>}
        {endpoints && <FactList rows={[
          { label: t("providers.label.requestUrl"), icon: Link2, format: "code", value: endpoints.requestUrl },
        ]} />}
      </div>
    </div>
  );
}
