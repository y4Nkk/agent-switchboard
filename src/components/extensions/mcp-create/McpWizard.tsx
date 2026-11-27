import { useMessageState } from "../../../i18n/use-message-state";
import { useId, useRef, useState } from "react";
import { useI18n } from "../../../i18n";
import { Button } from "../../Button";
import { WorkspaceSubpage } from "../../WorkspaceSubpage";
import { useUnsavedChanges } from "../../useUnsavedChanges";
import { Input } from "../../Input";
import { Select } from "../../Select";
import { SettingsGroup, SettingsLayout, SettingsRow } from "../../SettingsLayout";
import { CheckIcon, CloseIcon } from "../../icons";
import { McpArguments } from "./McpArguments";
import { McpCodexOptions } from "./McpCodexOptions";
import { McpBearerField, McpCredentialRows } from "./McpCredentials";
import { McpIdentityFields } from "./McpFormFields";
import { validateMcpSource } from "./mcp-draft";
import type { CreateSource, McpType } from "./mcp-json";
import { changeTransport, initializeWizard, transportDropsFields, wizardSource, type WizardServer } from "./wizard-state";

function ServerFields({ server, busy, original, onChange }: {
  server: WizardServer; busy: boolean; original?: CreateSource; onChange: (server: WizardServer) => void;
}) {
  const { t } = useI18n();
  if (server.type === "stdio") {
    return (
      <>
        <SettingsRow label={t("mcp.field.command")}>
          <Input code aria-label={t("mcp.field.command")} value={server.command} disabled={busy} placeholder="npx"
            onChange={(event) => onChange({ ...server, command: event.target.value })} />
        </SettingsRow>
        <McpArguments rows={server.args} busy={busy} onChange={(args) => onChange({ ...server, args })} />
        <McpCredentialRows kind="env" rows={server.env} busy={busy}
          onChange={(env) => onChange({ ...server, env })} />
        <McpCodexOptions value={server.codexOptions} busy={busy}
          onChange={(codexOptions) => onChange({ ...server, codexOptions })} />
      </>
    );
  }
  return (
    <>
      <SettingsRow label={t("mcp.field.url")}>
        <Input code aria-label={t("mcp.field.url")} value={server.url} disabled={busy} placeholder="https://mcp.example.com"
          onChange={(event) => onChange({ ...server, url: event.target.value })} />
      </SettingsRow>
      <McpCredentialRows kind="headers" rows={server.headers} busy={busy}
        onChange={(headers) => onChange({ ...server, headers })} />
      {server.type === "http" && <McpBearerField value={server.bearer} busy={busy}
        canKeep={original?.server.type === "http" && original.server.bearer?.mode === "secretConfigured"}
        onChange={(bearer) => onChange({ ...server, bearer })} />}
    </>
  );
}

interface Props {
  initial: CreateSource;
  original?: CreateSource;
  busy: boolean;
  onApply: (source: CreateSource) => void;
  onCancel: () => void;
}

export function McpWizard({ initial, original, busy, onApply, onCancel }: Props) {
  const { t } = useI18n();
  const transports = [
    { value: "stdio", label: t("mcp.transport.stdio") },
    { value: "http", label: t("mcp.transport.http") },
    { value: "sse", label: t("mcp.transport.sse") },
    { value: "ws", label: t("mcp.transport.ws") },
  ];
  const [name, setName] = useState(initial.name);
  const [server, setServer] = useState(() => initializeWizard(initial.server));
  const baseline = useRef(JSON.stringify({ name, server }));
  const leave = useUnsavedChanges(baseline.current !== JSON.stringify({ name, server }), busy, { guardPageRoot: true });
  const id = useId();
  const close = () => leave.request(onCancel);
  const [pendingType, setPendingType] = useState<McpType | null>(null);
  const [error, setError] = useMessageState();
  const update = (next: WizardServer) => { setServer(next); setError(null); };
  const chooseTransport = (value: string) => {
    const type = value as McpType;
    if (transportDropsFields(server, type)) setPendingType(type);
    else { update(changeTransport(server, type)); setPendingType(null); }
  };
  const apply = () => {
    if (busy || pendingType) return;
    try {
      const source = wizardSource(name, server);
      validateMcpSource(source, [], original);
      onApply(source);
    } catch (caught) { setError(caught); }
  };
  return (
    <WorkspaceSubpage layout="fixed-actions" title={t("mcp.wizard.aria")} backLabel={t("mcp.action.cancel")} busy={busy} onBack={close}
      footer={<><Button variant="secondary" disabled={busy} onClick={close}><CloseIcon />{t("mcp.action.cancel")}</Button>
        <Button type="submit" form={id} variant="primary" disabled={busy || pendingType !== null}><CheckIcon />{t("mcp.wizard.apply")}</Button></>}>
      <SettingsLayout>
        <form id={id} className="asb-form asb-mcp-create" aria-label={t("mcp.wizard.aria")} noValidate
          aria-describedby={error ? `${id}-error` : undefined}
          onSubmit={(event) => { event.preventDefault(); apply(); }}
          onKeyDown={(event) => {
            if (event.key === "Escape" && !event.defaultPrevented && !busy) { event.preventDefault(); event.stopPropagation(); close(); }
          }}>
          <SettingsGroup title={t("mcp.group.identity")}>
            <McpIdentityFields name={name} busy={busy} autoFocus onNameChange={(next) => { setName(next); setError(null); }} />
            <SettingsRow label={t("mcp.field.transport")}>
              <Select ariaLabel={t("mcp.field.transport")} value={server.type} options={transports} disabled={busy}
                onChange={chooseTransport} />
            </SettingsRow>
            {pendingType && <div className="asb-mcp-transport-confirm" role="alert">
              <p>{t("mcp.wizard.transportConfirm")}</p>
              <Button variant="secondary" disabled={busy} onClick={() => setPendingType(null)}>{t("mcp.wizard.cancelChange")}</Button>
              <Button variant="primary" disabled={busy} onClick={() => {
                update(changeTransport(server, pendingType)); setPendingType(null);
              }}>{t("mcp.wizard.changeTransport")}</Button>
            </div>}
          </SettingsGroup>
          <SettingsGroup title={t("mcp.group.connection")}>
            <ServerFields server={server} busy={busy || pendingType !== null} original={original} onChange={update} />
          </SettingsGroup>
          {error && <p id={`${id}-error`} className="asb-warn-text asb-mcp-error" role="alert">{error}</p>}
        </form>
      </SettingsLayout>
      {leave.confirmation}
    </WorkspaceSubpage>
  );
}
