import { invoke } from "./client";

/** Egress policy for application-owned HTTP clients: none (always direct),
 * system (follow the environment) or manual (an explicit proxy URL). */
export type OutboundProxyMode = "none" | "system" | "manual";

export interface OutboundProxySettings {
  version: number;
  mode: OutboundProxyMode;
  /** Absent outside manual mode; the backend omits the key entirely. */
  url?: string | null;
  revision: string;
}

export interface OutboundProxyView {
  settings: OutboundProxySettings | null;
  revision: string;
  issue: import("./shared").CommandError | null;
}

export function getOutboundProxySettings(): Promise<OutboundProxyView> {
  return invoke("get_outbound_proxy_settings");
}

export function setOutboundProxySettings(
  settings: { mode: OutboundProxyMode; url: string | null }, expectedRevision: string,
): Promise<OutboundProxyView> {
  return invoke("set_outbound_proxy_settings", { settings, expectedRevision });
}
