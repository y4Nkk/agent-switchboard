import { useState } from "react";
import { useI18n } from "../../../i18n";
import { CollapsibleSection } from "../../Collapsible";
import { Input } from "../../Input";
import { SettingsRow } from "../../SettingsLayout";
import { Textarea } from "../../Textarea";
import type { MetadataDraft } from "./metadata";

export function McpMetadataFields({ value, busy, onChange }: {
  value: MetadataDraft; busy: boolean; onChange: (value: MetadataDraft) => void;
}) {
  const { t } = useI18n();
  const [expanded, setExpanded] = useState(Boolean(value.description || value.tags || value.homepage || value.docs));
  const change = (key: keyof MetadataDraft, text: string) => onChange({ ...value, [key]: text });
  return <>
    <SettingsRow label={t("mcp.field.displayName")}>
      <Input aria-label={t("mcp.field.displayName")} value={value.displayName} disabled={busy} placeholder={t("mcp.placeholder.optional")}
        onChange={(event) => change("displayName", event.target.value)} />
    </SettingsRow>
    <CollapsibleSection quiet open={expanded} onOpenChange={setExpanded}
      label={t("mcp.field.metadataSummary")}>
      <div>
        <SettingsRow label={t("mcp.field.description")} wide>
          <Textarea aria-label={t("mcp.field.description")} value={value.description} rows={2} disabled={busy}
            onChange={(event) => change("description", event.target.value)} />
        </SettingsRow>
        <SettingsRow label={t("mcp.field.tags")}>
          <Input aria-label={t("mcp.field.tags")} value={value.tags} placeholder="docs, search" disabled={busy}
            onChange={(event) => change("tags", event.target.value)} />
        </SettingsRow>
        {([["homepage", "mcp.field.homepage"], ["docs", "mcp.field.docs"]] as const).map(([key, labelKey]) =>
          <SettingsRow label={t(labelKey)} key={key}>
            <Input code aria-label={t(labelKey)} value={value[key]} disabled={busy} placeholder="https://"
              onChange={(event) => change(key, event.target.value)} />
          </SettingsRow>)}
      </div>
    </CollapsibleSection>
  </>;
}
