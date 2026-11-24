import { FileArchive, FolderGit2, LoaderCircle, RefreshCw, Settings2 } from "lucide-react";
import { useI18n } from "../../i18n";
import type { MessageKey } from "../../i18n";
import { Button } from "../Button";
import { Input } from "../Input";
import { SearchInput } from "../SearchInput";
import { Select } from "../Select";
import { FolderOpenIcon, SearchIcon } from "../icons";
import type { SkillSourceKind } from "./skill-source-model";
import type { SkillSourceState } from "./useSkillSource";

const SOURCES: { value: SkillSourceKind; labelKey: MessageKey }[] = [
  { value: "directory", labelKey: "extensions.sources.kindDirectory" }, { value: "catalog", labelKey: "extensions.sources.kindCatalog" },
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
    <div className="asb-ext-market-query">
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
  const { source, repositories } = state;
  const scanning = source === "directory" ? state.directory.searching : state.loading;
  const scanDisabled = busy || state.loading || state.imports.busy || (source === "catalog" &&
    (!repositories.ready || repositories.loading || !repositories.items.some((repo) => repo.enabled)));
  const scanLabel = scanning ? t(source === "directory" ? "extensions.sources.searching" : "extensions.sources.scanning")
    : t(source === "catalog" ? "extensions.sources.refreshRepos" : source === "directory" ? "extensions.sources.search"
      : source === "zip" ? "extensions.sources.scanZip" : "extensions.sources.scanSource");
  return (
    <form className="asb-ext-market-form" aria-label={t("extensions.sources.formAria")} onSubmit={(event) => {
      event.preventDefault();
      if (!scanDisabled) void state.search();
    }}>
      <Select value={source} ariaLabel={t("extensions.sources.kindAria")} disabled={disabled || state.directory.resolving !== null}
        options={SOURCES.map(({ value, labelKey }) => ({ value, label: t(labelKey) }))}
        onChange={(value) => state.changeSource(value as SkillSourceKind)} />
      <SourceInput state={state} disabled={disabled} busy={busy} />
      <Button type="submit" variant="primary" disabled={scanDisabled}>
        {scanning ? <LoaderCircle size={16} className="asb-skill-source-spinner" />
          : source === "catalog" ? <RefreshCw size={16} /> : <SearchIcon />}
        {scanLabel}
      </Button>
      {source === "catalog" && <Button variant="secondary" disabled={disabled}
        onClick={() => state.setManagerOpen(true)}>
        <Settings2 size={16} />{t("extensions.sources.manageRepos")}
      </Button>}
    </form>
  );
}
