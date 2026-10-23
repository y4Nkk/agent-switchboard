import { createContext, useCallback, useContext, useEffect, useId, useMemo, useState, type ReactNode } from "react";
import type { ToastContent, ToastKind } from "./use-toast";

export interface InformationNotice {
  id: string;
  kind: ToastKind;
  title: ToastContent;
  description?: ToastContent;
  action?: ReactNode;
}

export interface InformationSourceMessage extends InformationNotice {
  acknowledged?: boolean;
  onAcknowledge?: () => void;
}

type Publish = (id: string, messages: readonly InformationSourceMessage[] | null) => void;
const PublishContext = createContext<Publish | null>(null);
const MessagesContext = createContext<readonly InformationSourceMessage[]>([]);

/** Source owners publish their current view; this provider stores no history or acknowledgement. */
export function InformationSources({ children }: { children: ReactNode }) {
  const [sources, setSources] = useState(() => new Map<string, readonly InformationSourceMessage[]>());
  const publish = useCallback<Publish>((id, messages) => {
    setSources((previous) => {
      if (messages === null && !previous.has(id) || previous.get(id) === messages) return previous;
      const next = new Map(previous);
      if (messages === null) next.delete(id);
      else next.set(id, messages);
      return next;
    });
  }, []);
  const messages = useMemo(() => [...sources].flatMap(([source, items]) =>
    items.map((item) => ({ ...item, id: `${source}:${item.id}` }))), [sources]);
  return <PublishContext.Provider value={publish}>
    <MessagesContext.Provider value={messages}>{children}</MessagesContext.Provider>
  </PublishContext.Provider>;
}

export function useInformationSource(messages: readonly InformationNotice[]) {
  const publish = useContext(PublishContext);
  const id = useId();
  const [acknowledged, setAcknowledged] = useState(() => new Set<string>());
  useEffect(() => {
    const active = new Set(messages.map((message) => message.id));
    setAcknowledged((previous) => {
      const next = new Set([...previous].filter((key) => active.has(key)));
      return next.size === previous.size ? previous : next;
    });
  }, [messages]);
  const presented = useMemo(() => messages.map((message) => ({ ...message,
    acknowledged: acknowledged.has(message.id),
    onAcknowledge: () => setAcknowledged((previous) => previous.has(message.id)
      ? previous : new Set([...previous, message.id])),
  })), [messages, acknowledged]);
  if (!publish) throw new Error("Information source requires the application information provider");
  useEffect(() => { publish(id, presented); }, [publish, id, presented]);
  useEffect(() => () => publish(id, null), [publish, id]);
}

export const useInformationSources = () => useContext(MessagesContext);
