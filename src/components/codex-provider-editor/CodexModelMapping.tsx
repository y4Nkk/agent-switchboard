import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { Input } from "../Input";
import { Select } from "../Select";
import { SettingsGroup } from "../SettingsLayout";
import { Table } from "../Table";
import type { CodexEditorState } from "./useCodexProviderEditor";

export function CodexModelMapping({ editor, busy }: { editor: CodexEditorState; busy: boolean }) {
  const { t } = useI18n();
  const { draft, setDraft } = editor;
  const update = (index: number, patch: Partial<(typeof draft.modelRoutes)[number]>) => setDraft((current) => ({ ...current,
    modelRoutes: current.modelRoutes.map((route, row) => row === index ? { ...route, ...patch } : route),
  }));
  return <SettingsGroup title={t("providerSetup.mapping")} description={t("codex.model.mappingNote")} surface="plain" actions={
      <Button variant="ghost" size="compact" disabled={busy || !draft.catalog.length} onClick={() => setDraft((current) => ({ ...current,
        modelRoutes: [...current.modelRoutes, { clientModel: current.catalog[0].id, upstreamModel: "" }],
      }))}>{t("codex.model.mappingAdd")}</Button>
    }>
    {draft.modelRoutes.length > 0 && <Table ariaLabel={t("providerSetup.mapping")}
      rows={draft.modelRoutes.map((route, index) => ({ ...route, index }))} rowKey={(_, index) => String(index)} columns={[
        { key: "client", header: t("providerSetup.clientModel"), render: (row) => <Select disabled={busy}
          ariaLabel={t("codex.model.mappingClientAria", { index: row.index + 1 })} value={row.clientModel}
          options={draft.catalog.map(({ id }) => ({ value: id, label: id }))} onChange={(clientModel) => update(row.index, { clientModel })} /> },
        { key: "upstream", header: t("providerSetup.upstreamModel"), render: (row) => <Input code required disabled={busy}
          aria-label={t("codex.model.mappingUpstreamAria", { index: row.index + 1 })} value={row.upstreamModel}
          onChange={(event) => update(row.index, { upstreamModel: event.target.value })} /> },
        { key: "remove", header: t("providerSetup.actions"), render: (row) => <Button variant="dangerText" size="compact" disabled={busy}
          aria-label={t("codex.model.mappingDeleteAria", { index: row.index + 1 })}
          onClick={() => setDraft((current) => ({ ...current, modelRoutes: current.modelRoutes.filter((_, index) => index !== row.index) }))}>
          {t("providerSetup.remove")}</Button> },
      ]} />}
  </SettingsGroup>;
}
