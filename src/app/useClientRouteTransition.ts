import type { AppKind } from "../api/client";
import { usePanelSlideTransition } from "./usePanelSlideTransition";

const CLIENT_ORDER = ["codex", "claude"] as const;

export function useClientRouteTransition({ app, active, onSelectApp }: {
  app: AppKind;
  active: boolean;
  onSelectApp: (app: AppKind) => void;
}) {
  const { stageRef, panelRefs, leaving, select } = usePanelSlideTransition({
    value: app, order: CLIENT_ORDER, active, onChange: onSelectApp,
  });
  const selectApp = (next: AppKind) => {
    const focused = document.activeElement;
    const focusPicker = focused instanceof HTMLInputElement &&
      stageRef.current?.contains(focused) === true && focused.closest(".asb-client-picker") !== null;
    select(next, focusPicker ? (stage) => {
      stage.querySelector<HTMLInputElement>(
        `[data-client-route="${next}"] .asb-client-picker input:checked`,
      )?.focus({ preventScroll: true });
    } : undefined);
  };
  return { workspaceRef: stageRef, contentRefs: panelRefs, leavingApp: leaving, selectApp };
}
