import { useCallback } from "react";
import { queryProfileUsage, type ProviderProfile } from "../api/client";
import { useAutoQuery } from "./use-auto-query";
import { useUsageHistory } from "./use-usage-history";

/** The card owns queries so collapsing its details does not stop polling. */
export function useProviderUsage(profile: ProviderProfile) {
  const revision = JSON.stringify(profile.usageQuery);
  const history = useUsageHistory({ kind: "provider", profileId: profile.id }, revision);
  const query = useCallback(async (profileId: string) => {
    const summary = await queryProfileUsage(profileId);
    void history.refresh();
    return summary;
  }, [history.refresh, revision]);
  const reading = useAutoQuery(
    profile.id,
    profile.usageQuery?.refreshIntervalMinutes ?? 0,
    query,
    "用量查询失败",
  );
  return { ...reading, history };
}

export type ProviderUsage = ReturnType<typeof useProviderUsage>;
