import { invoke } from "./client";
import type { CommandError } from "./shared";

/** The native updater owns the candidate and its signed download URL. */
export interface UpdateCheck {
  id: string;
  latestVersion: string;
  releaseNotes: string | null;
  /** null means the release manifest made no level statement at all. */
  advisory: UpdateAdvisory | null;
  checkedAt: string;
}

/** Publisher-authored urgency: routine says "whenever convenient";
 * important and urgent carry the release's own reason in both languages. */
export type UpdateAdvisory = { severity: "routine" } | {
  severity: "important" | "urgent";
  reasonZh: string;
  reasonEn: string;
};

export type UpdateChannel = "github" | "microsoftStore";
export type UpdateDownloadPhase = "downloading" | "verifying" | "cancelling" | "installing"
  | "installed" | "cancelled" | "failed";

export interface UpdateDownloadState {
  id: string;
  revision: number;
  version: string;
  phase: UpdateDownloadPhase;
  downloadedBytes: number;
  totalBytes: number | null;
  error: CommandError | null;
}

export const getUpdateChannel = (): Promise<UpdateChannel> => invoke("update_channel");
export const checkUpdate = (): Promise<UpdateCheck | null> => invoke("check_application_update");
export const startUpdateDownload = (updateId: string): Promise<UpdateDownloadState> =>
  invoke("start_update_download", { updateId });
export const readUpdateDownload = (id: string): Promise<UpdateDownloadState> => invoke("read_update_download", { id });
/** The returned phase is authoritative when installation has already committed. */
export const cancelUpdateDownload = (id: string): Promise<UpdateDownloadState> => invoke("cancel_update_download", { id });
export interface ChangelogEntry {
  version: string;
  date: string | null;
  notes: string;
}

export const getChangelogHistory = (): Promise<ChangelogEntry[]> => invoke("changelog_history");
