import type { AppKind, ClientConfigurationApplyPreview, ClientConfigurationResetKind,
  ClientConfigurationResetPreview as ResetPreview } from "../api/client";
import { useI18n } from "../i18n";
import { localizedMessageText } from "../i18n/errors";
import { Button } from "./Button";
import { Checkbox } from "./Checkbox";
import { DiffView } from "./DiffView";

const CLAUDE_EXTRA_MANIFEST_KEYS = new Set([
  "/env/ASB_CLAUDE_COMMON_KEYS",
  "env.ASB_CLAUDE_COMMON_KEYS",
]);

function visibleResetChanges(
  resetKind: ClientConfigurationResetKind,
  preview: ClientConfigurationApplyPreview,
) {
  const changes = preview.file.preview.changes;
  return resetKind === "clearExtraConfiguration"
    ? changes.filter((change) => !CLAUDE_EXTRA_MANIFEST_KEYS.has(change.key))
    : changes;
}

function NativeResetScope({ app, advanced }: { app: AppKind; advanced: boolean }) {
  const { t } = useI18n();
  return (
    <div className="asb-client-settings-reset-scope">
      <span className="asb-client-settings-reset-scope-label">{t("clientConfig.reset.scopeWillRestore")}</span>
      <ul>
        <li>{t("clientConfig.reset.scopeStandardSettings")}</li>
        {app === "codex" && <li>{t("clientConfig.reset.scopeCodexSubagent")}</li>}
        {advanced && <li>{t("clientConfig.reset.scopeUnmanaged")}</li>}
      </ul>
      <p className="asb-field-help">
        {advanced
          ? t("clientConfig.reset.advancedHelp")
          : t("clientConfig.reset.basicHelp")}
      </p>
    </div>
  );
}

function ExtraResetScope({ extraConfigurationCount }: { extraConfigurationCount: number }) {
  const { t } = useI18n();
  return (
    <div className="asb-client-settings-reset-scope">
      <span className="asb-client-settings-reset-scope-label">{t("clientConfig.reset.scopeWillClear")}</span>
      <p>{t("clientConfig.reset.extraScopeLine", { count: extraConfigurationCount })}</p>
      <p className="asb-field-help">
        {t("clientConfig.reset.extraScopeHelp")}
      </p>
    </div>
  );
}

function RemovableFields({ fields, selected, busy, onSelect }: {
  fields: ResetPreview["removableFields"];
  selected: string[];
  busy: boolean;
  onSelect: (key: string, checked: boolean) => void;
}) {
  const { t } = useI18n();
  if (fields.length === 0) return null;
  const selectedKeys = new Set(selected);
  return <details className="asb-client-reset-selection">
    <summary>{t("clientConfig.reset.selectFields", { count: fields.length })}</summary>
    <ul>{fields.map(({ key, before }) => <li key={key}>
      <Checkbox checked={selectedKeys.has(key)} disabled={busy} label={key} onChange={(checked) => onSelect(key, checked)} />
      {before !== null && <code>{before}</code>}
    </li>)}</ul>
  </details>;
}

export function ClientConfigurationResetPreview({
  preview,
  app,
  resetKind,
  extraConfigurationCount,
  busy,
  onConfirm,
  selected = [],
  onSelect,
  onBack,
}: {
  preview: ResetPreview;
  app: AppKind;
  resetKind: ClientConfigurationResetKind;
  extraConfigurationCount: number;
  busy: boolean;
  onConfirm: () => void;
  selected?: string[];
  onSelect?: (key: string, checked: boolean) => void;
  onBack?: () => void;
}) {
  const deep = selected.length > 0;
  const native = resetKind !== "clearExtraConfiguration";
  const changes = visibleResetChanges(resetKind, preview);
  const { t } = useI18n();
  const changeLabel = native
    ? deep ? t("clientConfig.reset.changeFields") : t("clientConfig.reset.changeStandard")
    : t("clientConfig.reset.changeExtra");
  const confirmLabel = !native
    ? t("clientConfig.reset.confirmExtra", { count: extraConfigurationCount })
    : deep ? t("clientConfig.reset.confirmDeep") : t("clientConfig.reset.confirmNative");
  return (
    <div className="asb-client-settings-reset-preview">
      <p className="asb-client-settings-reset-lead">
        {!native
          ? t("clientConfig.reset.extraLead")
          : deep
            ? t("clientConfig.reset.deepLead")
            : t("clientConfig.reset.nativeLead")}
      </p>
      {native ? (
        <>
          <NativeResetScope app={app} advanced={deep} />
          {onSelect && <RemovableFields fields={preview.removableFields} selected={selected} busy={busy} onSelect={onSelect} />}
        </>
      ) : (
        <ExtraResetScope extraConfigurationCount={extraConfigurationCount} />
      )}
      <p className="asb-client-settings-reset-target">
        <span>{t("clientConfig.reset.targetFile")}</span>
        <code>{preview.file.preview.target}</code>
      </p>
      {changes.length > 0 ? (
        <div className="asb-client-settings-reset-changes">
          <DiffView changes={changes} label={changeLabel} />
        </div>
      ) : (
        <p className="asb-field-help">
          {t("clientConfig.reset.noChanges")}
        </p>
      )}
      {preview.file.preview.warnings.map((warning) => (
        <p key={warning.key} className="asb-field-error" role="alert">{localizedMessageText(warning, t)}</p>
      ))}
      <p className="asb-field-help">
        {t("clientConfig.reset.backupHelp")}
      </p>
      <div className="asb-client-settings-reset-actions">
        {onBack && (
          <Button variant="secondary" className="asb-client-settings-reset-back" disabled={busy} onClick={onBack}>
            {t("clientConfig.reset.backToSettings")}
          </Button>
        )}
        <Button variant="danger" disabled={busy} onClick={onConfirm}>
          {confirmLabel}
        </Button>
      </div>
    </div>
  );
}
