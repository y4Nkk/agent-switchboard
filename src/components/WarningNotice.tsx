import { useRef, useState, type ReactNode } from "react";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { ScrollArea } from "./ScrollArea";

export interface WarningNoticeItem {
  id: string;
  content: ReactNode;
}

/** Acknowledgement belongs to each warning's identity for this mounted workspace. */
export function WarningNotice({ label, items, actions, disabled = false }: {
  label: string;
  items: WarningNoticeItem[];
  actions?: ReactNode;
  disabled?: boolean;
}) {
  const { t } = useI18n();
  const [acknowledged, setAcknowledged] = useState<Set<string>>(() => new Set());
  const focusTarget = useRef<"review" | "body" | null>(null);
  const visible = items.filter((item) => !acknowledged.has(item.id));
  if (items.length === 0) return null;
  if (visible.length === 0) return <Button variant="unstyled" className="asb-warning-review"
    aria-label={`${label} · ${t("notice.review", { count: items.length })}`} aria-expanded={false}
    ref={(node) => { if (node && focusTarget.current === "review") { node.focus(); focusTarget.current = null; } }}
    onClick={() => { focusTarget.current = "body"; setAcknowledged(new Set()); }}>{t("notice.review", { count: items.length })}</Button>;
  return <section className="asb-warning-notice" aria-label={label}>
    <ScrollArea className="asb-warning-body" aria-label={label} tabIndex={0}
      viewportRef={(node) => { if (node && focusTarget.current === "body") { node.focus(); focusTarget.current = null; } }}>
      <ul>{visible.map((item) => <li key={item.id}>{item.content}</li>)}</ul>
    </ScrollArea>
    <div className="asb-warning-actions">
      {actions}
      <Button variant="secondary" disabled={disabled} onClick={() => {
        focusTarget.current = "review";
        setAcknowledged((previous) => new Set([...previous, ...visible.map((item) => item.id)]));
      }}>{t("notice.acknowledge")}</Button>
    </div>
  </section>;
}
