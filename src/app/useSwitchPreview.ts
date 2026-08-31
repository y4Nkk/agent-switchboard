import { useCallback, useRef, useState } from "react";
import {
  previewSwitch,
  type CommandError,
  type FilePreview,
  type ProviderProfile,
} from "../api/client";

interface SwitchPreviewDeps {
  busy: boolean;
  selectedId: string | null;
  setSelectedId: (id: string) => void;
  onError: (error: CommandError) => void;
  clearError: () => void;
}

/**
 * The switch-candidate preview lifecycle: shared in-flight requests,
 * versioned responses, and invalidation. Selection changes never fetch a
 * diff; previews are fetched on explicit request only.
 */
export function useSwitchPreview({
  busy,
  selectedId,
  setSelectedId,
  onError,
  clearError,
}: SwitchPreviewDeps) {
  const [preview, setPreview] = useState<{ profileId: string; file: FilePreview } | null>(null);
  const previewVersion = useRef(0);
  const previewRequests = useRef(new Map<string, Promise<FilePreview>>());

  /** Shared in-flight requests: repeated clicks on the same target reuse one
   * backend command instead of piling up synchronous reads and renders. */
  const requestSwitchPreview = useCallback((profileId: string) => {
    const existing = previewRequests.current.get(profileId);
    if (existing) return existing;
    const next = previewSwitch(profileId);
    previewRequests.current.set(profileId, next);
    void next
      .finally(() => {
        if (previewRequests.current.get(profileId) === next) previewRequests.current.delete(profileId);
      })
      .catch(() => {});
    return next;
  }, []);

  /** Retract the displayed preview; cached candidates stay valid because the
   * underlying files have not changed. */
  const retractPreview = useCallback(() => {
    previewVersion.current += 1;
    setPreview(null);
  }, []);

  /** Selection only. Diffs are fetched on explicit request, and a stale
   * preview of another profile must never survive a selection change. */
  const selectProfile = useCallback(
    (profileId: string) => {
      retractPreview();
      setSelectedId(profileId);
      clearError();
    },
    [clearError, retractPreview, setSelectedId],
  );

  /** Writes invalidate every switch candidate. A prior file or in-flight
   * response must never be rendered after its source changes. */
  const invalidateSwitchCandidates = useCallback(() => {
    previewVersion.current += 1;
    previewRequests.current.clear();
    setPreview(null);
  }, []);

  const previewProfile = useCallback(
    async (profile: ProviderProfile) => {
      if (busy) return;
      const version = ++previewVersion.current;
      setSelectedId(profile.id);
      setPreview(null);
      clearError();
      try {
        const file = await requestSwitchPreview(profile.id);
        if (previewVersion.current === version) {
          setPreview({ profileId: profile.id, file });
        }
      } catch (caught) {
        if (previewVersion.current === version) onError(caught as CommandError);
      }
    },
    [busy, clearError, onError, requestSwitchPreview, setSelectedId],
  );

  /** The eye button toggles: open retracts when this row's preview is already
   * showing; any other row swaps the preview (user decision 2026-08-28). */
  const togglePreviewProfile = useCallback(
    (profile: ProviderProfile) => {
      if (preview?.profileId === profile.id) {
        retractPreview();
        return;
      }
      void previewProfile(profile);
    },
    [preview, previewProfile, retractPreview],
  );

  const runPreview = useCallback(async () => {
    if (busy || !selectedId) return;
    const version = ++previewVersion.current;
    const profileId = selectedId;
    clearError();
    setPreview(null);
    try {
      const file = await requestSwitchPreview(profileId);
      if (previewVersion.current === version) setPreview({ profileId, file });
    } catch (caught) {
      if (previewVersion.current === version) onError(caught as CommandError);
    }
  }, [busy, clearError, onError, requestSwitchPreview, selectedId]);

  return {
    preview,
    retractPreview,
    invalidateSwitchCandidates,
    selectProfile,
    previewProfile,
    togglePreviewProfile,
    runPreview,
  };
}
