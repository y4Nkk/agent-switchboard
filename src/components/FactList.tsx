import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

export interface FactListRow {
  label: ReactNode;
  value: ReactNode;
  icon?: LucideIcon;
  tone?: "active" | "warning";
  format?: "code" | "number";
}

export function FactStatus({ children, tone = "neutral" }: {
  children: ReactNode;
  tone?: "success" | "neutral" | "warning";
}) {
  const state = tone === "success" ? " is-ok" : tone === "neutral" ? " is-idle" : "";
  return <span className={`asb-status-pill${state}`}>
    <span className="asb-status-pill-dot" aria-hidden="true" />
    <span className="asb-num">{children}</span>
  </span>;
}

export function FactList({ rows, inline = false }: { rows: FactListRow[]; inline?: boolean }) {
  return (
    <dl className={inline ? "asb-fact-row is-inline" : "asb-fact-row"}>
      {rows.map(({ label, value, icon: Icon, tone, format }, index) => (
        <div key={index}>
          <dt className={Icon ? "asb-fact-label" : undefined}>
            {Icon ? <>
              <span className="asb-fact-icon" data-tone={tone} aria-hidden="true"><Icon /></span>
              <span>{label}</span>
            </> : label}
          </dt>
          <dd>{format === "code" ? <code className="asb-code asb-fact-code">{value}</code>
            : format === "number" ? <strong className="asb-num">{value}</strong> : value}</dd>
        </div>
      ))}
    </dl>
  );
}
