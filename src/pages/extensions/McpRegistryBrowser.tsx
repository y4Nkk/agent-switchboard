import { useState } from "react";
import { searchMcpRegistry, type McpRegistryEntry, type McpRegistryPage } from "../../api/extensions/registry";
import { Button } from "../../components/Button";
import { SearchInput } from "../../components/SearchInput";
import { ExtensionLoading } from "../../components/extensions/ExtensionLoading";
import { ExtensionMarket, ExtensionMarketResult, ExtensionMarketSearch } from "../../components/extensions/ExtensionMarket";
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
  const version = t("extensions.registry.version", { version: entry.version });
  const origin = label === entry.name ? version : `${entry.name} · ${version}`;
  return <ExtensionMarketResult title={label} description={entry.description}
    origin={<p className="asb-skill-source-origin"><span>{origin}</span></p>}
    actions={<>
      <SkillSourceLink url={entry.docs} label={t("extensions.registry.docs", { name: label })} />
      {saved ? <span className="asb-scope-note">{t("extensions.registry.saved", { version: saved.mcpMetadata?.registry?.version ?? "" })}</span>
        : source ? <Button variant="primary" disabled={w.writeBlocked}
          aria-label={t("extensions.registry.configureAria", { name: label })}
          onClick={() => w.nav.setDialog({ type: "newMcp", registryEntry: { ...entry, source } })}>{t("extensions.registry.configure")}</Button>
        : <span className="asb-scope-note">{t("extensions.registry.manualConfig")}</span>}
    </>}>
    {source && <p className="asb-skill-source-origin"><span>{t("mcp.transport.http")} · <code>{source.endpoint}</code></span></p>}
  </ExtensionMarketResult>;
}

export function McpRegistryBrowser({ workspace: w, active }: { workspace: ExtensionWorkspace; active: boolean }) {
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
  return <ExtensionMarket label={t("extensions.registry.marketMcp")} active={active} controls={
    <ExtensionMarketSearch label={t("extensions.registry.search")} onSubmit={() => { if (!w.busy && !loading) void search(); }}
      actions={<>
        <Button type="submit" variant="primary" disabled={w.busy || loading}>
          <SearchIcon />{t(query ? "extensions.sources.search" : "extensions.registry.browse")}
        </Button>
        <Button variant="secondary" disabled={w.writeBlocked} onClick={() => w.nav.setDialog({ type: "newMcp" })}>
          {t("extensions.toolbar.addMcp")}
        </Button>
      </>}>
      <SearchInput value={input} onChange={(value) => { cancel(); setInput(value); }}
        ariaLabel={t("extensions.registry.search")} placeholder={t("extensions.registry.search")} />
    </ExtensionMarketSearch>
  }>
    {request.error && <p className="asb-warn-text" role="alert">{request.error}</p>}
    {loading && !page && <ExtensionLoading />}
    {!page && !loading && !request.error && <div className="asb-skill-source-empty" role="status">
      <SearchIcon /><p>{t("extensions.registry.notSearched")}</p>
    </div>}
    {page && <section aria-label={t("extensions.registry.results")} aria-busy={loading}>
      {page.items.length > 0 ? <>
        <p className="asb-skill-source-count asb-num" role="status">{t("extensions.registry.loadedCount", { count: page.items.length })}</p>
        <ul className="asb-ext-market-list">
          {page.items.map((entry) => <RegistryEntry key={entry.name} entry={entry} workspace={w} />)}
        </ul>
      </> : <div className="asb-skill-source-empty" role="status"><p>{t("extensions.registry.noMatches")}</p></div>}
      {nextCursor && <div className="asb-skill-source-more"><Button variant="secondary" disabled={loading || w.busy}
        onClick={() => void loadMore(page, nextCursor)}>{t("extensions.directory.loadMore")}</Button></div>}
    </section>}
  </ExtensionMarket>;
}
