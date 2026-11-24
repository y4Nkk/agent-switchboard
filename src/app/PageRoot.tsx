import { createContext, useContext, useEffect, useMemo, useRef, type ReactNode } from "react";
import type { Page } from "./navigation";
import { createPageRoot, joinPageRoot, type PageRootGuard, type PageRootRequest } from "./page-root";

const PageRootContext = createContext<ReturnType<typeof createPageRoot> | null>(null);

/** Frames one page's subtree. Re-clicking that page in the sidebar returns it
 * to its first-level view: the open views registered through useOnReturnToRoot
 * close, and the unsaved-change guards of second-level pages decide whether
 * the return may run. */
export function PageRoot({ page, request, children }: {
  page: Page;
  request: PageRootRequest | null;
  children: ReactNode;
}) {
  const root = useMemo(createPageRoot, []);
  useEffect(() => {
    if (request?.page === page) root.returnToRoot();
  }, [request, page, root]);
  return <PageRootContext.Provider value={root}>{children}</PageRootContext.Provider>;
}

/** Registers the caller's second-level view. When the page returns to its root
 * and the view is open, `close` runs; a busy open view holds the return exactly
 * like its disabled back control. */
export function useOnReturnToRoot(open: boolean, close: () => void, busy = false) {
  const root = useContext(PageRootContext);
  const view = useRef({ open, busy, close });
  view.current = { open, busy, close };
  useEffect(() => {
    if (!root) return;
    return joinPageRoot(root.views, view);
  }, [root]);
}

export function usePageRootGuard(enabled: boolean, guard: PageRootGuard) {
  const root = useContext(PageRootContext);
  const member = useRef(guard);
  member.current = guard;
  useEffect(() => {
    if (!root || !enabled) return;
    return joinPageRoot(root.guards, member);
  }, [enabled, root]);
}
