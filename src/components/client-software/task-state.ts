import type { ClientUpdateTask } from "../../api/client-software";

export function latestClientUpdate(current: ClientUpdateTask | null, incoming: ClientUpdateTask | null): ClientUpdateTask | null {
  if (!current) return incoming;
  return incoming && incoming.revision > current.revision ? incoming : current;
}
