import type { Page } from "./navigation.ts";

/** A sidebar re-click on the page already shown; every click is a fresh object. */
export interface PageRootRequest { page: Page }

/** One registered second-level view: whether it is open, what closes it, and whether its back control is disabled. */
export interface PageRootView { open: boolean; busy: boolean; close: () => void }

/** An unsaved-changes decision, read at the moment of the return. */
export interface PageRootGuard { dirty: boolean; busy: boolean; request: (run: () => void) => void }

type Member<T> = { current: T };

/** The second-level views of one page: which of them may hold the return, and what closes them. */
export interface PageRoot {
  views: Array<Member<PageRootView>>;
  guards: Array<Member<PageRootGuard>>;
  returnToRoot: () => void;
}

/** Registers a member; the returned function removes it again. */
export function joinPageRoot<T>(members: Array<Member<T>>, member: Member<T>): () => void {
  members.push(member);
  return () => {
    const index = members.indexOf(member);
    if (index >= 0) members.splice(index, 1);
  };
}

export function createPageRoot(): PageRoot {
  const views: Array<Member<PageRootView>> = [];
  const guards: Array<Member<PageRootGuard>> = [];
  return {
    views,
    guards,
    returnToRoot: () => {
      const shown = views.map((view) => view.current).filter((view) => view.open);
      // Already at the root: nothing closes, so nobody is asked.
      if (!shown.length) return;
      const protecting = guards.map((guard) => guard.current);
      // A view that is saving or scanning keeps its back control disabled; the sidebar follows it.
      if ([...shown, ...protecting].some((member) => member.busy)) return;
      const run = () => shown.forEach((view) => view.close());
      const dirty = protecting.find((guard) => guard.dirty);
      if (dirty) dirty.request(run);
      else run();
    },
  };
}
