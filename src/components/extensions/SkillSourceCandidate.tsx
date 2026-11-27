import { useMessageState } from "../../i18n/use-message-state";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Download, ExternalLink, FileArchive, FolderGit2, LoaderCircle } from "lucide-react";
import { useI18n } from "../../i18n";
import { isBrowserDevelopment } from "../../lib/runtime";
import { clientName } from "../../lib/client-name";
import { Button } from "../Button";
import { Tooltip } from "../Tooltip";
import { CheckIcon, FolderOpenIcon } from "../icons";
import { ExtensionMarketResult } from "./ExtensionMarket";
import { skillCandidateHost, sourceErrorMessage, type SkillSourceKind, type SkillSourceRow } from "./skill-source-model";
import type { SkillSourceState } from "./useSkillSource";

export function SkillSourceLink({ url, label }: { url: string | null; label: string }) {
  const { t } = useI18n();
  const [error, setError] = useMessageState();
  if (!url) return null;
  return <>
    <Tooltip label={label}>
      <a href={url} target="_blank" rel="noreferrer" className="asb-btn asb-btn-icon" aria-label={label}
        onClick={(event) => {
          if (isBrowserDevelopment) return;
          event.preventDefault(); setError(null);
          void openUrl(url).catch((reason: unknown) => setError(reason));
        }}><ExternalLink size={16} /></a>
    </Tooltip>
    {error && <span className="asb-warn-text" role="alert">{sourceErrorMessage(error, t("extensions.sources.openLinkFailed"))}</span>}
  </>;
}

interface CandidateProps {
  row: SkillSourceRow;
  installed: boolean;
  imports: SkillSourceState["imports"];
  source: SkillSourceKind;
  busy: boolean;
  installs?: number;
  hasOtherVersion?: boolean;
}

export function SkillSourceCandidate({ row, installed, imports, source, busy, installs, hasOtherVersion }: CandidateProps) {
  const { t } = useI18n();
  const { candidate } = row;
  const installing = imports.active === candidate.candidateId;
  const rejected = candidate.diagnostics.length > 0;
  const host = skillCandidateHost(candidate);
  const clients = host ? clientName(host) : `${clientName("codex")} / ${clientName("claude")}`;
  const label = installing ? t("extensions.sources.installingBadge")
    : rejected ? t("extensions.sources.uninstallable") : source === "directory"
      ? t("extensions.registry.installSkillFor", { clients }) : t("extensions.sources.install");
  return (
    <ExtensionMarketResult title={candidate.name} description={candidate.description ?? undefined}
      origin={<Tooltip asChild label={row.label}>
        <p className="asb-skill-source-origin">
          {source === "catalog" || source === "directory" ? <FolderGit2 size={14} /> :
            source === "zip" ? <FileArchive size={14} /> : <FolderOpenIcon />}
          <span>{row.label}</span>
        </p>
      </Tooltip>}
      actions={<>
        <SkillSourceLink url={row.sourceUrl} label={t("extensions.sources.viewSourceAria", { name: candidate.name })} />
        {installed ? <span className="asb-skill-source-installed"><CheckIcon />{t("extensions.sources.installed")}</span>
          : hasOtherVersion ? <span className="asb-scope-note">{t("extensions.registry.useSkillUpdate")}</span>
          : <Button variant="primary" disabled={busy || imports.busy || rejected}
            onClick={() => void imports.install(candidate)} aria-label={`${label} ${candidate.name}`}>
            {installing ? <LoaderCircle size={16} className="asb-skill-source-spinner" /> : <Download size={16} />}
            {label}
          </Button>}
      </>}>
      <span className="asb-skill-source-meta">
        <span>{host ? t("extensions.sources.hostClaudeOnly") : t("extensions.sources.hostBoth")}</span>
        <span className="asb-num">{t("extensions.sources.fileCount", { count: candidate.fileCount })}</span>
        {source === "directory" && candidate.source?.resolvedCommit && <code>{candidate.source.resolvedCommit.slice(0, 12)}</code>}
        {installs !== undefined && <span className="asb-num">{t("extensions.sources.installs", { count: installs.toLocaleString() })}</span>}
      </span>
      {rejected && <ul className="asb-skill-source-diagnostics" aria-label={t("extensions.sources.diagnosticsAria", { name: candidate.name })}>
        {candidate.diagnostics.map((message, index) => <li key={index}>{sourceErrorMessage(message, t("extensions.sources.invalidManifest"))}</li>)}
      </ul>}
    </ExtensionMarketResult>
  );
}
