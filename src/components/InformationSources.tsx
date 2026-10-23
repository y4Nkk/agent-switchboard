import { createContext, useContext, useEffect, useMemo, useReducer, type Dispatch, type ReactNode } from "react";
import { reduceInformationSources, type InformationNotice, type InformationSource,
  type InformationSourceAction, type InformationSourceState } from "./information-source-state";

export type { InformationNotice } from "./information-source-state";

export interface InformationSourceMessage extends InformationNotice {
  acknowledged?: boolean;
  onAcknowledge?: () => void;
  onDismiss?: () => void;
}

const DispatchContext = createContext<Dispatch<InformationSourceAction> | null>(null);
const MessagesContext = createContext<readonly InformationSourceMessage[]>([]);

/** One application owner keeps source decisions across view withdrawal and remount. */
export function InformationSources({ children }: { children: ReactNode }) {
  const [sources, dispatch] = useReducer(reduceInformationSources, undefined, (): InformationSourceState => new Map());
  const messages = useMemo(() => [...sources].flatMap(([source, state]) =>
    (state.messages ?? []).filter((item) => state.statuses.get(item.id) !== "dismissed")
      .map((item) => ({ ...item, id: `${source}:${item.id}`,
        acknowledged: state.statuses.get(item.id) === "acknowledged",
        onAcknowledge: () => dispatch({ type: "acknowledge", source, id: item.id }),
        onDismiss: () => dispatch({ type: "dismiss", source, id: item.id }),
      }))), [sources]);
  return <DispatchContext.Provider value={dispatch}>
    <MessagesContext.Provider value={messages}>{children}</MessagesContext.Provider>
  </DispatchContext.Provider>;
}

export function useInformationSource(source: InformationSource, messages: readonly InformationNotice[] | null) {
  const dispatch = useContext(DispatchContext);
  if (!dispatch) throw new Error("Information source requires the application information provider");
  useEffect(() => { dispatch({ type: "publish", source, messages }); }, [dispatch, source, messages]);
  useEffect(() => () => dispatch({ type: "publish", source, messages: null }), [dispatch, source]);
}

export const useInformationSources = () => useContext(MessagesContext);
