import type { ConfigFileStatus, ProviderProfile } from "../api/client";

/**
 * Provider identity comes from the live connection, not configuration drift
 * or the profile mentioned by a historical write.
 */
export function currentProviderName(
  status: ConfigFileStatus | undefined,
  profiles: readonly ProviderProfile[],
): string {
  if (!status?.route) return "未加载";

  const active = profiles.find(
    (profile) => profile.app === status.app && profile.id === status.activeProfileId,
  );
  if (active) return active.name;

  if (status.route.providerName) return status.route.providerName;
  if (status.route.routeMode === "official") return "官方登录";
  return "未识别的供应商";
}
