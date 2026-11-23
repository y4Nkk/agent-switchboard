import { useMemo, useState } from "react";
import { useI18n } from "../../i18n";
import { AppDialog } from "../AppDialog";
import { Button } from "../Button";
import { Checkbox } from "../Checkbox";
import { Input } from "../Input";

export interface PickerCandidate {
  id: string;
  /** Read-only summary of what checking this row will fill in. */
  hint?: string;
}

interface Props {
  title: string;
  candidates: readonly PickerCandidate[];
  /** Ids that live in the catalog today; checking state starts from here. */
  selected: ReadonlySet<string>;
  busy: boolean;
  /** Receives the complete next id set — add and remove apply in one pass. */
  onApply: (ids: ReadonlySet<string>) => void;
  onClose: () => void;
}

/**
 * Batch selection over the editor's current candidates. The editor owns
 * discovery, catalog enrichment and the manual model field.
 */
export function ModelPickerDialog({ title, candidates, selected, busy, onApply, onClose }: Props) {
  const { t } = useI18n();
  const [checked, setChecked] = useState(() => new Set(selected));
  const [query, setQuery] = useState("");
  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return needle ? candidates.filter(({ id }) => id.toLowerCase().includes(needle)) : candidates;
  }, [candidates, query]);
  const allVisible = visible.length > 0 && visible.every(({ id }) => checked.has(id));
  const toggle = (id: string) => setChecked((current) => {
    const next = new Set(current);
    if (next.has(id)) next.delete(id); else next.add(id);
    return next;
  });
  const toggleAll = () => setChecked((current) => {
    const next = new Set(current);
    for (const { id } of visible) if (allVisible) next.delete(id); else next.add(id);
    return next;
  });
  return <AppDialog title={title} busy={busy} onClose={onClose}
    footer={<>
      <Button variant="secondary" disabled={busy} onClick={onClose}>{t("clientConfig.common.cancel")}</Button>
      <Button variant="primary" disabled={busy} onClick={() => { onApply(checked); onClose(); }}>{t("providerSetup.pickerApply")}</Button>
    </>}>
    <div className="asb-model-picker-dialog">
      <label className="asb-field"><span>{t("providerSetup.pickerSearch")}</span>
        <Input code value={query} disabled={busy} placeholder={t("providerSetup.pickerPlaceholder")}
          onChange={(event) => setQuery(event.target.value)} /></label>
      <div className="asb-model-picker-dialog-head">
        <Checkbox checked={allVisible} indeterminate={!allVisible && visible.some(({ id }) => checked.has(id))}
          disabled={busy || visible.length === 0} label={t(allVisible ? "providerSetup.pickerClearAll" : "providerSetup.pickerSelectAll")}
          onChange={toggleAll} />
        <span className="asb-field-help">{t("providerSetup.pickerCount", { count: checked.size })}</span>
      </div>
      {visible.length > 0 ? <ul className="asb-model-picker-dialog-list">
        {visible.map(({ id, hint }) => <li key={id}>
          <Checkbox checked={checked.has(id)} disabled={busy} label={id} onChange={() => toggle(id)} />
          {hint && <span className="asb-field-help">{hint}</span>}
        </li>)}
      </ul> : <p className="asb-field-help">{t("providerSetup.pickerEmpty")}</p>}
    </div>
  </AppDialog>;
}
