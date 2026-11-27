import { useId, type ComponentPropsWithRef, type ReactNode } from "react";

export function SettingsLayout({ className, ...props }: ComponentPropsWithRef<"div">) {
  return <div {...props} className={["asb-settings-layout", className].filter(Boolean).join(" ")} />;
}

export function SettingsGroup({ title, description, actions, surface = "muted", busy, className, children }: {
  title: string;
  description?: string;
  actions?: ReactNode;
  surface?: "muted" | "plain";
  busy?: boolean;
  className?: string;
  children: ReactNode;
}) {
  const heading = useId();
  return (
    <section className={["asb-settings-group", className].filter(Boolean).join(" ")}
      aria-labelledby={heading} aria-busy={busy || undefined}>
      <div className="asb-settings-group-heading">
        <h3 className="asb-section-title" id={heading}>{title}</h3>
        {actions}
      </div>
      {description && <p className="asb-field-help">{description}</p>}
      <div className="asb-settings-group-body" data-surface={surface}>{children}</div>
    </section>
  );
}

export function SettingsRow({ label, description, labelId, descriptionId, labelAction, details, wide, children }: {
  label: ReactNode;
  description?: ReactNode;
  labelId?: string;
  descriptionId?: string;
  labelAction?: ReactNode;
  details?: ReactNode;
  wide?: boolean;
  children: ReactNode;
}) {
  return (
    <div className="asb-settings-row" data-wide={wide || undefined}>
      <div className="asb-settings-row-copy">
        <div className="asb-settings-row-label"><span id={labelId}>{label}</span>{labelAction}</div>
        {description && <div id={descriptionId} className="asb-settings-row-description">{description}</div>}
      </div>
      {children != null && <div className="asb-settings-row-controls">{children}</div>}
      {details && <div className="asb-settings-row-details">{details}</div>}
    </div>
  );
}
