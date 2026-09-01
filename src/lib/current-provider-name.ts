import type { ConfigFileStatus } from "../api/client";

/**
 * The profile match is the only trustworthy source for the supplier that is
 * currently projected into a client configuration. Route parsing supplies
 * connection facts, but client files do not contain the profile name.
 */
export function currentProviderName(status: ConfigFileStatus | undefined): string {
  if (!status?.route) return "未加载";

  if (
    status.matchStatus.kind === "matchesProfile" ||
    status.matchStatus.kind === "profileChanged"
  ) {
    return status.matchStatus.profileName;
  }

  if (status.route.providerName) return status.route.providerName;
  if (status.route.routeMode === "official") return "官方登录";
  return "未识别的供应商";
}
