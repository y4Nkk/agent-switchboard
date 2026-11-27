import { useId, useMemo, useRef, useState } from "react";
import type {
  ProviderConnectionOptions,
  ProviderRequestTarget,
  ResponsesOptions,
  UpstreamProtocol,
} from "../../api/client";
import { usesClaudeManagedAuth } from "../../api/claude-accounts";
import { useI18n } from "../../i18n";
import { SettingsGroup } from "../SettingsLayout";
import { Button } from "../Button";
import { Collapsible } from "../Collapsible";
import { ConnectivityIcon } from "../icons";
import { ProviderTestPanel } from "../ProviderTestPanel";

interface Props {
  app: import("../../api/shared").AppKind;
  baseUrl: string | null;
  connection?: ProviderConnectionOptions | null;
  apiKey: string;
  authentication?: import("../../api/shared").AuthenticationScheme | null;
  upstreamProtocol: UpstreamProtocol | null;
  /** Required exactly when the protocol is Responses, matching the backend contract. */
  responsesOptions: ResponsesOptions | null;
  defaultModel: string | null;
  busy: boolean;
  active: boolean;
}

export function ProviderConnectionTest({
  app, baseUrl, connection, apiKey, authentication, upstreamProtocol, responsesOptions, defaultModel, busy, active,
}: Props) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const id = useId();
  const trigger = useRef<HTMLButtonElement>(null);
  const valid = upstreamProtocol === "responses" ? responsesOptions !== null : responsesOptions === null;
  const target = useMemo<ProviderRequestTarget | null>(() =>
    baseUrl?.trim() && (apiKey.trim() || (app === "claude" && usesClaudeManagedAuth(connection))) && upstreamProtocol && valid ? {
      kind: "draft",
      connection: {
        app,
        baseUrl: baseUrl.trim(),
        apiKey: apiKey.trim(),
        ...(authentication ? { authentication } : {}),
        connection: connection ?? {},
        upstreamProtocol,
        responsesOptions,
        defaultModel: defaultModel?.trim() || null,
      },
    } : null,
  [app, baseUrl, connection, apiKey, authentication, upstreamProtocol, responsesOptions, defaultModel, valid]);
  return <SettingsGroup title={t("providers.editor.section.connectionTest")}>
    <div className="asb-editor-section-fields">
      <div className="asb-editor-action-row">
        <Button ref={trigger} variant="secondary" disabled={busy} aria-expanded={open && active}
          aria-controls={open && active ? id : undefined} onClick={() => setOpen((value) => !value)}>
          <ConnectivityIcon />{open ? t("providers.editor.test.collapse") : t("providers.editor.test.open")}
        </Button>
        {!open && !target && <span className="asb-field-help">{t("providers.editor.test.needFields")}</span>}
      </div>
      {active && <Collapsible open={open}>
        <ProviderTestPanel id={id} name={t("providers.test.draftName")} url={baseUrl?.trim() || null} target={target}
          onClose={() => { setOpen(false); trigger.current?.focus(); }} />
      </Collapsible>}
    </div>
  </SettingsGroup>;
}
