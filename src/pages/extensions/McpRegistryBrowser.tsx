import { useState } from "react";
import { searchMcpRegistry, type McpRegistryEntry, type McpRegistryPage } from "../../api/extensions/registry";
import { Button } from "../../components/Button";
import { ScrollArea } from "../../components/ScrollArea";
import { SearchInput } from "../../components/SearchInput";
import { ExtensionLoading } from "../../components/extensions/ExtensionLoading";
import { SkillSourceLink } from "../../components/extensions/SkillSourceCandidate";
import { useSkillSourceRequests } from "../../components/extensions/useSkillSourceRequests";
import { SearchIcon } from "../../components/icons";
import { useI18n } from "../../i18n";
import type { ExtensionWorkspace } from "./useExtensionWorkspace";

interface Snapshot extends McpRegistryPage { query: string }

function RegistryEntry({ entry, workspace: w }: { entry: McpRegistryEntry; workspace: ExtensionWorkspace }) {
  const { t } = useI18n();
  const { source } = entry;
  const saved = w.items.find((item) => item.kind === "mcp" && item.mcpMetadata?.registry?.name === entry.name);
  const label = entry.title ?? entry.name;
  return <li className="asb-skill-source-card">
    <div className="asb-skill-source-card-heading">
      <h3 className="asb-group-title">{label}</h3>
      <SkillSourceLink url={entry.docs} label={t("extensions.registry.docs", { name: label })} />
    </div>
    <p className="asb-skill-source-origin"><span>{t("extensions.registry.sourceVersion", { name: entry.name, version: entry.version })}</span></p>
    {entry.description && <p className="asb-skill-source-description">{entry.description}</p>}
    {source && <p className="asb-skill-source-origin"><span>{source.endpoint}</span></p>}
    <div className="asb-skill-source-card-footer">
      {saved ? <span>{t("extensions.registry.saved", { version: saved.mcpMetadata?.registry?.version ?? "" })}</span>
        : source ? <Button variant="primary" disabled={w.writeBlocked}
          onClick={() => w.nav.setDialog({ type: "newMcp", registryEntry: { ...entry, source } })}>{t("extensions.registry.configure")}</Button>
        : <span>{t("extensions.registry.manualConfig")}</span>}
    </div>
  </li>;
}

export function McpRegistryBrowser({ workspace: w }: { workspace: ExtensionWorkspace }) {
  const { t } = useI18n();
  const [input, setInput] = useState("");
  const query = input.trim();
  const request = useSkillSourceRequests<Snapshot>(w.busy);
  const { cancel, result, loading } = request;
  const search = () => request.run(async () => ({ ...await searchMcpRegistry(query, null), query }));
  const loadMore = (previous: Snapshot, cursor: string) => request.run(async () => {
    const next = await searchMcpRegistry(previous.query, cursor);
    const entries = new Map(previous.items.map((item) => [item.name, item]));
    for (const item of next.items) entries.set(item.name, item);
    return { ...next, items: [...entries.values()], query: previous.query };
  });
  const page = result?.query === query ? result : null;
  const nextCursor = page?.nextCursor;
  return <div className="asb-ext-market">
    <form className="asb-ext-market-form" aria-label={t("extensions.registry.search")}
      onSubmit={(event) => { event.preventDefault(); if (!w.busy && !loading) void search(); }}>
      <div className="asb-ext-market-query">
        <SearchInput value={input} onChange={(value) => { cancel(); setInput(value); }}
          ariaLabel={t("extensions.registry.search")} placeholder={t("extensions.registry.search")} />
      </div>
      <Button type="submit" variant="primary" disabled={w.busy || loading}>
        <SearchIcon />{t(query ? "extensions.sources.search" : "extensions.registry.browse")}
      </Button>
      <Button variant="secondary" disabled={w.writeBlocked} onClick={() => w.nav.setDialog({ type: "newMcp" })}>
        {t("extensions.toolbar.addMcp")}
      </Button>
    </form>
    <ScrollArea className="asb-ext-market-results" role="region" aria-label={t("extensions.registry.marketMcp")} tabIndex={0}>
      <div className="asb-skill-source-browser">
        {request.error && <p className="asb-warn-text" role="alert">{request.error}</p>}
        {loading && !page && <ExtensionLoading />}
        {page && <section aria-label={t("extensions.registry.results")} aria-busy={loading}>
          {page.items.length > 0 ? <ul className="asb-skill-source-grid">
            {page.items.map((entry) => <RegistryEntry key={entry.name} entry={entry} workspace={w} />)}
          </ul> : <div className="asb-skill-source-empty" role="status"><p>{t("extensions.directory.noMatches")}</p></div>}
          {nextCursor && <div className="asb-skill-source-more"><Button variant="secondary" disabled={loading || w.busy}
            onClick={() => void loadMore(page, nextCursor)}>{t("extensions.directory.loadMore")}</Button></div>}
        </section>}
      </div>
    </ScrollArea>
  </div>;
}
