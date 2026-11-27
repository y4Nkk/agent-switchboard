import type { AppKind } from "../api/client";
import { usePanelSlideTransition } from "./usePanelSlideTransition";

const CLIENT_ORDER = ["codex", "claude"] as const;

export function useClientRouteTransition({ app, active, onSelectApp, revealPicker = false }: {
  app: AppKind;
  active: boolean;
  onSelectApp: (app: AppKind) => void;
  revealPicker?: boolean;
}) {
  const { stageRef, panelRefs, leaving, select } = usePanelSlideTransition({
    value: app, order: CLIENT_ORDER, active, onChange: onSelectApp,
  });
  const selectApp = (next: AppKind) => {
    const focused = document.activeElement;
    const focusPicker = focused instanceof HTMLInputElement &&
      stageRef.current?.contains(focused) === true && focused.closest(".asb-client-picker") !== null;
    select(next, (focusPicker || revealPicker) ? (stage) => {
      const picker = stage.querySelector<HTMLInputElement>(
        `[data-client-route="${next}"] .asb-client-picker input:checked`,
      );
      if (revealPicker) picker?.closest(".asb-scroll-viewport")?.scrollTo({ top: 0, behavior: "instant" });
      if (focusPicker) picker?.focus({ preventScroll: true });
    } : undefined);
  };
  return { workspaceRef: stageRef, contentRefs: panelRefs, leavingApp: leaving, selectApp };
}
