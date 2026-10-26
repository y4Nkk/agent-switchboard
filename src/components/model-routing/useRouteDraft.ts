import { useEffect, useRef, useState } from "react";
import type { AppKind } from "../../api/client";
import { useUnsavedChanges } from "../useUnsavedChanges";
import { routeDocument, routeRecords, type ModelRouteRequest, type RouteDocument, type RouteInventory, type RouteRole } from "./model";

interface Props {
  inventory: RouteInventory;
  initialApp: AppKind;
  request: ModelRouteRequest | null;
  busy: boolean;
}

export function useRouteDraft({ inventory, initialApp, request, busy }: Props) {
  const [app, setApp] = useState(request?.app ?? initialApp);
  const [document, setDocument] = useState<RouteDocument | null>(null);
  const [role, setRole] = useState<RouteRole>("main");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const requestSeen = useRef(0);
  const initialized = useRef(false);
  const sources = routeRecords(inventory, app);
  const original = document ? routeDocument(document) : null;
  const dirty = !!document && JSON.stringify(document.draft) !== JSON.stringify(original?.draft);
  const leave = useUnsavedChanges(dirty, busy);
  const load = (nextApp: AppKind, id: string | null) => {
    const source = routeRecords(inventory, nextApp).find(({ record }) => record.profile.id === id);
    setApp(nextApp);
    setSelectedId(id);
    setDocument(source ? routeDocument(source) : null);
    setRole("main");
    initialized.current = true;
  };
  useEffect(() => {
    if (busy || !request || request.revision === requestSeen.current) return;
    requestSeen.current = request.revision;
    leave.request(() => load(request.app, request.profileId));
  }, [request, busy]);
  useEffect(() => {
    if (initialized.current || request) return;
    const activeId = inventory.activeIds[app];
    if (activeId && sources.some(({ record }) => record.profile.id === activeId)) load(app, activeId);
  }, [inventory, app, sources, request]);
  const current = sources.find(({ record }) => record.profile.id === selectedId);
  const stale = !!selectedId && (!current || current.record.fileHash !== document?.record.fileHash);
  return { app, document, setDocument, selectedId, role, setRole, sources, dirty, stale,
    selectApp: (next: AppKind) => leave.request(() => load(next, inventory.activeIds[next] ?? null)),
    selectProfile: (id: string) => leave.request(() => load(app, id)),
    reload: () => leave.request(() => load(app, selectedId)), confirmation: leave.confirmation };
}
