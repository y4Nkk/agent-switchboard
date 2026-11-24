import type { ReactNode } from "react";
import { useI18n } from "../../i18n";
import { Button } from "../Button";
import { ChevronRightIcon } from "../icons";
import { RouteCardArt } from "./RouteCardArt";
import type { routeCardState, RouteRole } from "./model";

const STATUS_LABELS = {
  assigned: "modelRoutes.status.assigned",
  automatic: "modelRoutes.automatic",
  needsAttention: "modelRoutes.status.needsAttention",
} as const;

const RULE_LABELS = {
  main: "gatewayOverview.mainConnection",
  primary: "gatewayOverview.primaryConnection",
  provider: "gatewayOverview.fixedProvider",
} as const;

interface Props {
  role: RouteRole;
  state: ReturnType<typeof routeCardState>;
  fallback: string;
  children?: ReactNode;
  onOpen?: () => void;
  disabled?: boolean;
}

export function RouteCard({ role, state, fallback, children, onOpen, disabled }: Props) {
  const { t } = useI18n();
  const title = t(`modelRoutes.role.${role}`);
  const status = t(STATUS_LABELS[state.status]);
  const model = state.model ?? (state.target ? fallback : t("modelRoutes.clientDefault"));
  const provider = state.target?.provider ?? t("modelRoutes.unresolved");
  const note = state.target ? [provider, state.target.provider && t(RULE_LABELS[state.rule]),
    state.inherited && t("modelRoutes.inheritOpus")].filter(Boolean).join(" · ") : null;
  const summary = <>
    <span className="asb-purpose-card-header">
      <span className="asb-purpose-card-title">{title}</span>
      <span className="asb-purpose-card-status">{status}</span>
    </span>
    <span className="asb-purpose-card-destination">
      <RouteCardArt status={state.status} />
      <span className="asb-purpose-card-target">
        <span className="asb-purpose-card-model" data-kind={state.model ? "model" : undefined}>{model}</span>
        {note && <span className="asb-purpose-card-note">{note}</span>}
        {onOpen && state.oneM && <span className="asb-purpose-card-note">1M</span>}
      </span>
      {onOpen && <span className="asb-purpose-card-arrow" aria-hidden="true"><ChevronRightIcon /></span>}
    </span>
  </>;
  return <li className="asb-purpose-card" data-status={state.status} data-route-role={role} tabIndex={onOpen ? undefined : -1}>
    {onOpen ? <Button variant="ghost" className="asb-purpose-card-open" disabled={disabled} onClick={onOpen}
      aria-label={`${t("gatewayOverview.editPurpose", {
        role: title, provider: state.target ? provider : t("modelRoutes.automatic"), model,
      })}. ${status}${state.oneM ? ", 1M" : ""}`}>
      {summary}
      {state.issue && <span className="asb-purpose-card-issue asb-warn-text">{t(state.issue)}</span>}
    </Button> : <>
      {summary}
      {state.issue && <p className="asb-purpose-card-issue asb-warn-text" role="status">{t(state.issue)}</p>}
      {children}
    </>}
  </li>;
}
