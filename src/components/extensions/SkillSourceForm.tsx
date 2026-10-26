import { FileArchive, FolderGit2, Settings2 } from "lucide-react";
import { useI18n } from "../../i18n";
import type { MessageKey } from "../../i18n";
import { Button } from "../Button";
import { Input } from "../Input";
import { RadioOption } from "../RadioOption";
import { SearchInput } from "../SearchInput";
import { Tooltip } from "../Tooltip";
import { FolderOpenIcon } from "../icons";
import type { SkillSourceKind } from "./skill-source-model";
import type { SkillSourceState } from "./useSkillSource";

const SOURCES: { value: SkillSourceKind; labelKey: MessageKey }[] = [
  { value: "catalog", labelKey: "extensions.sources.kindCatalog" }, { value: "directory", labelKey: "extensions.sources.kindDirectory" },
  { value: "zip", labelKey: "extensions.sources.kindZip" }, { value: "local", labelKey: "extensions.sources.kindLocal" },
];

function SourceInput({ state, disabled, busy }: { state: SkillSourceState; disabled: boolean; busy: boolean }) {
  const { t } = useI18n();
  if (state.source === "catalog") return (
    <span className="asb-skill-source-catalog-count asb-num">
      <FolderGit2 size={16} />
      {state.repositories.loading ? t("extensions.sources.loadingRepos") : state.repositories.ready ?
        t("extensions.sources.repoCounts", {
          total: state.repositories.items.length,
          enabled: state.repositories.items.filter((repo) => repo.enabled).length,
        }) : t("extensions.sources.reposNotLoaded")}
    </span>
  );
  if (state.source === "directory") return (
    <div className="asb-skill-source-query">
      <SearchInput ariaLabel={t("extensions.sources.searchSkillsSh")} placeholder={t("extensions.sources.searchSkillsSh")}
        value={state.directory.input} disabled={disabled} onChange={state.directory.changeInput} />
    </div>
  );
  const kind = state.source;
  return <>
    <Input required aria-label={kind === "zip" ? t("extensions.sources.zipPathAria") : t("extensions.sources.localDirAria")}
      placeholder={kind === "zip" ? t("extensions.sources.zipPathPlaceholder") : t("extensions.sources.localDirPlaceholder")}
      value={state.files.paths[kind]} disabled={disabled}
      onChange={(event) => state.files.changePath(kind, event.target.value)} />
    <Button variant="secondary" disabled={busy || state.loading || state.imports.busy}
      onClick={() => void state.pick()}>
      {kind === "zip" ? <FileArchive size={16} /> : <FolderOpenIcon />}
      {kind === "zip" ? t("extensions.sources.pickZip") : t("extensions.sources.browse")}
    </Button>
  </>;
}

export function SkillSourceForm({ state, busy }: { state: SkillSourceState; busy: boolean }) {
  const { t } = useI18n();
  const disabled = state.imports.busy || (busy && !state.loading);
  return (
    <form id="skill-source-scan" className="asb-skill-source-form" aria-label={t("extensions.sources.formAria")} onSubmit={(event) => {
      event.preventDefault(); void state.search();
    }}>
      <div className="asb-skill-source-input-row">
        <div className="asb-segments" role="radiogroup" aria-label={t("extensions.sources.kindAria")}>
          {SOURCES.map(({ value, labelKey }) => (
            <RadioOption key={value} name="skill-source-type" checked={state.source === value}
              disabled={disabled} label={t(labelKey)} onChange={() => state.changeSource(value)} />
          ))}
        </div>
        <Tooltip label={t("extensions.sources.manageRepos")}>
          <Button variant="icon" aria-label={t("extensions.sources.manageRepos")} disabled={disabled}
            onClick={() => state.setManagerOpen(true)}><Settings2 size={16} /></Button>
        </Tooltip>
      </div>
      <div className="asb-skill-source-input-row">
        <SourceInput state={state} disabled={disabled} busy={busy} />
      </div>
    </form>
  );
}
