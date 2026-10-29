import { useCallback, useState } from "react";
import type { AppKind } from "../api/client";
import type { ModelRouteRequest } from "../components/model-routing/model";

export function useModelRouteNavigation(openPage: (page: "gateway") => void) {
  const [request, setRequest] = useState<ModelRouteRequest | null>(null);
  const open = useCallback((app: AppKind, profileId: string) => {
    setRequest((current) => ({ app, profileId, revision: (current?.revision ?? 0) + 1 }));
    openPage("gateway");
  }, [openPage]);
  return { request, open };
}
