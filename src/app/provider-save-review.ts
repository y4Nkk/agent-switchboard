import { routeDescription, routeRecords, routeRoles, type RouteDocument, type RouteInventory, type RouteRole } from "../components/model-routing/model.ts";
import type { AppKind } from "../api/client";

export interface ProviderSaveReview {
  editedName: string;
  activeName: string | null;
  changes: { role: RouteRole; before: ReturnType<typeof routeDescription>; after: ReturnType<typeof routeDescription> }[];
}

export function providerSaveReview(app: AppKind, before: RouteDocument, after: RouteDocument, inventory: RouteInventory): ProviderSaveReview {
  const activeName = routeRecords(inventory, app).find(({ record }) => record.profile.id === inventory.activeIds[app])?.record.profile.name ?? null;
  return { editedName: after.draft.name, activeName,
    changes: routeRoles(after).map((role) => ({ role, before: routeDescription(before, role, inventory), after: routeDescription(after, role, inventory) }))
      .filter(({ before, after }) => JSON.stringify(before) !== JSON.stringify(after)) };
}
