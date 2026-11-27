import { useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n";
import { Button } from "./Button";
import { Tooltip } from "./Tooltip";
import { CheckIcon, RestoreIcon } from "./icons";

/** One visible clipboard result for code, device codes and generated answers.
 * `iconOnly` renders a fixed-footprint icon action; the accessible name then
 * comes from `label` / `copiedLabel` instead of visible text. */
export function CopyButton({ content, iconOnly = false, label, copiedLabel }: {
  content: string;
  iconOnly?: boolean;
  label?: string;
  copiedLabel?: string;
}) {
  const { t } = useI18n();
  const [status, setStatus] = useState<"idle" | "copying" | "copied" | "failed">("idle");
  const locked = useRef(false);
  const revision = useRef(0);
  useEffect(() => { revision.current += 1; setStatus("idle"); }, [content]);
  useEffect(() => () => { revision.current += 1; }, []);
  const copy = async () => {
    if (locked.current) return;
    locked.current = true;
    const current = revision.current;
    setStatus("copying");
    try {
      await navigator.clipboard.writeText(content);
      if (current === revision.current) setStatus("copied");
    } catch {
      if (current === revision.current) setStatus("failed");
    } finally { locked.current = false; }
  };
  const copied = status === "copied";
  const text = t(copied ? "common.copied" : "common.copy");
  const actionLabel = (copied ? copiedLabel : label) ?? text;
  const button = <Button variant={iconOnly ? "icon" : "secondary"} disabled={status === "copying"}
    onClick={() => void copy()} aria-label={iconOnly ? actionLabel : undefined}>
    {copied ? <CheckIcon /> : <RestoreIcon />}
    {!iconOnly && text}
  </Button>;
  return <>
    {iconOnly ? <Tooltip label={actionLabel}>{button}</Tooltip> : button}
    {status === "failed" && <span className="asb-field-error" role="alert">{t("common.copyFailed")}</span>}
  </>;
}
