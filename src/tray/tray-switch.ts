/** How long a marked switch target waits for the confirming click. */
export const ARMED_SWITCH_TTL_MS = 5_000;

export interface ArmedSwitch { providerId: string; expiresAt: number }

/** The deadline also applies when a backgrounded WebView delays its timer. */
export function switchClickOutcome(armed: ArmedSwitch | null, providerId: string, now: number): "arm" | "switch" {
  return armed?.providerId === providerId && now < armed.expiresAt ? "switch" : "arm";
}
