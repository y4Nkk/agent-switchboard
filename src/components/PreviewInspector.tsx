import { useState } from "react";
import type { FilePreview, LocalizedMessage } from "../api/client";
import { useI18n } from "../i18n";
import { localizedMessageText } from "../i18n/errors";
import { CodePreview } from "./CodePreview";
import { CollapsibleSection } from "./Collapsible";
import { DiffView } from "./DiffView";
import { PreviewIcon } from "./icons";

interface Props {
  filePreview: FilePreview | null;
  showUserConfigModel?: boolean;
  showWarnings?: boolean;
  /** Model read from the client's user-level configuration file. */
  userConfigModel: string | null;
  /** Known conditions that can override the user-level configuration. */
  userConfigWarnings: LocalizedMessage[];
}

/**
 * The diff inspector. Configuration text stays high-contrast on a near-solid
 * backdrop: no backdrop blur touches this panel (DESIGN.md §6). Preserved
 * host keys are not listed flat — they stay visible in place inside the
 * pretty-printed candidate file.
 */
export function PreviewInspector({
  filePreview,
  showUserConfigModel = true,
  showWarnings = true,
  userConfigModel,
  userConfigWarnings,
}: Props) {
  const { t } = useI18n();
  const [fileOpen, setFileOpen] = useState(false);
  if (!filePreview) {
    return (
      <div className="asb-empty-state">
        <span className="asb-empty-state-icon" aria-hidden="true">
          <PreviewIcon />
        </span>
        <h3 className="asb-section-title">{t("providers.preview.empty")}</h3>
      </div>
    );
  }
  const { preview } = filePreview;
  return (
    <div className="asb-inspector">
      {showUserConfigModel && <div className="asb-kv">
        <span className="asb-kv-label">{t("providers.preview.userConfigModel")}</span>
        <span className="asb-kv-value asb-code">{userConfigModel ?? t("providers.label.defaultModel")}</span>
      </div>}
      {userConfigWarnings.length > 0 && (
        <ul className="asb-warnings" aria-label={t("providers.preview.scopeWarningsAria")}>
          {userConfigWarnings.map((warning) => (
            <li key={warning.key}>{localizedMessageText(warning, t)}</li>
          ))}
        </ul>
      )}
      {/* Warnings precede the diff: the gateway rewrite is explained before
          the 127.0.0.1 endpoint change is seen. */}
      {showWarnings && preview.warnings.length > 0 && (
        <ul className="asb-warnings" aria-label={t("providers.preview.warningsAria")}>
          {preview.warnings.map((warning) => (
            <li key={warning.key}>{localizedMessageText(warning, t)}</li>
          ))}
        </ul>
      )}
      {preview.changes.length === 0 && <p className="asb-empty">{t("providers.preview.noChanges")}</p>}
      <DiffView changes={preview.changes} label={t("providers.preview.changesLabel")} />
      <div className="asb-kv">
        <span className="asb-kv-label">{t("providers.preview.backupDir")}</span>
        <span className="asb-kv-value asb-code">{preview.backupDir}</span>
      </div>
      <CollapsibleSection quiet open={fileOpen} onOpenChange={setFileOpen}
        label={t("common.candidateFile")}>
        <CodePreview target={preview.target} content={filePreview.content} />
      </CollapsibleSection>
    </div>
  );
}
