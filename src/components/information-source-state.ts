import type { ReactNode } from "react";
import type { ToastContent, ToastKind } from "./use-toast";

export type InformationSource = "sessions" | "usage";

export interface InformationNotice {
  id: string;
  kind: ToastKind;
  title: ToastContent;
  description?: ToastContent;
  action?: ReactNode;
}

interface SourceState {
  messages: readonly InformationNotice[] | null;
  statuses: ReadonlyMap<string, "acknowledged" | "dismissed">;
}

export type InformationSourceState = ReadonlyMap<InformationSource, SourceState>;
export type InformationSourceAction =
  | { type: "publish"; source: InformationSource; messages: readonly InformationNotice[] | null }
  | { type: "acknowledge" | "dismiss"; source: InformationSource; id: string };

/** Null withdraws a view; only a current snapshot can retire resolved identities. */
export function reduceInformationSources(state: InformationSourceState,
  action: InformationSourceAction): InformationSourceState {
  const previous = state.get(action.source);
  if (action.type === "publish") {
    if (previous?.messages === action.messages || !previous && action.messages === null) return state;
    const messages = action.messages === null ? null
      : [...new Map(action.messages.map((message) => [message.id, message])).values()];
    const active = messages === null ? null : new Set(messages.map((message) => message.id));
    const statuses = new Map([...(previous?.statuses ?? [])].filter(([id]) => active === null || active.has(id)));
    const next = new Map(state);
    if (messages === null && statuses.size === 0) next.delete(action.source);
    else next.set(action.source, { messages, statuses });
    return next;
  }
  if (!previous?.messages?.some((message) => message.id === action.id)) return state;
  const status = action.type === "dismiss" ? "dismissed" : "acknowledged";
  const current = previous.statuses.get(action.id);
  if (current === "dismissed" || current === status) return state;
  return new Map(state).set(action.source, {
    ...previous, statuses: new Map(previous.statuses).set(action.id, status),
  });
}
