import { useState } from "react";
import type { AppKind } from "../../api/client";
import type { ConversionSource } from "../../app/provider-conversion-batch";
import { useI18n } from "../../i18n";
import { AppDialog } from "../AppDialog";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { ProviderEndpoint } from "../ProviderRowControls";
import "../../styles/base/provider-conversion.css";

export function ProviderConversionPicker({ app, sources, busy, onStart, onClose }: {
  app: AppKind; sources: ConversionSource[]; busy: boolean;
  onStart: (ids: string[]) => void; onClose: () => void;
}) {
  const { t } = useI18n();
  const [selected, setSelected] = useState<string[]>([]);
  const chosen = sources.filter(({ id }) => selected.includes(id));
  const source = app === "codex" ? "Codex" : "Claude";
  const target = app === "codex" ? "Claude" : "Codex";
  return <AppDialog title={t("providers.conversion.pickTitle", { source, target })} busy={busy} onClose={onClose}
    footer={<>
      <Button variant="secondary" disabled={busy} onClick={onClose}>{t("confirm.cancel")}</Button>
      <Button variant="primary" disabled={busy || !chosen.length} onClick={() => onStart(chosen.map(({ id }) => id))}>
        {t("providers.conversion.start", { count: chosen.length })}
      </Button>
    </>}>
    <p className="asb-field-help">{t("providers.conversion.pickHelp")}</p>
    {sources.length ? <>
      <Checkbox label={t("providers.conversion.selectAll")} checked={chosen.length === sources.length}
        indeterminate={chosen.length > 0 && chosen.length < sources.length} disabled={busy}
        onChange={(checked) => setSelected(checked ? sources.map(({ id }) => id) : [])} />
      <ul className="asb-conversion-choices" aria-label={t("providers.conversion.sourceList", { source })}>
        {sources.map((item) => <li key={item.id} data-conversion-source-id={item.id}>
          <Checkbox label={item.name} checked={selected.includes(item.id)} disabled={busy}
            onChange={(checked) => setSelected((current) => checked ? [...current, item.id] : current.filter((id) => id !== item.id))} />
          <span className="asb-conversion-source-detail">
            <span>{item.model ?? t("providers.label.defaultModel")}</span>
            {item.endpoint && <ProviderEndpoint url={item.endpoint} />}
          </span>
        </li>)}
      </ul>
    </> : <p role="status">{t("providers.conversion.empty", { source })}</p>}
  </AppDialog>;
}
