import { invoke } from "./client";
import type { AppKind } from "./shared";
import type { ClientConfigurationApplyPreview } from "./settings";

export interface NativeTelemetrySetup {
  receiver: {
    endpoint: string;
    listening: boolean;
    lastExportAtMs: number | null;
    acceptedRecords: number;
    ignoredRecords: number;
    error: string | null;
  };
  clients: Array<{ app: AppKind; configured: boolean; error: string | null }>;
}
export const readNativeTelemetry = (): Promise<NativeTelemetrySetup> => invoke("read_native_telemetry");
export const retryNativeTelemetry = (): Promise<void> => invoke("retry_native_telemetry");
export const previewNativeTelemetry = (target: AppKind, enabled: boolean): Promise<ClientConfigurationApplyPreview> =>
  invoke("preview_native_telemetry", { target, enabled });
export function commitNativeTelemetry(target: AppKind, enabled: boolean, preview: ClientConfigurationApplyPreview): Promise<void> {
  return invoke("commit_native_telemetry", { request: { target, enabled,
    expectedHash: preview.file.contentHash, expectedRenderedHash: preview.file.renderedHash,
    expectedSettingsHash: preview.settingsHash, expectedTargetExisted: preview.targetExisted }, confirmWrite: true });
}
