import type { MessageKey, TFunction } from "../i18n";

const details = {
  "approval_policy": "clientConfig.settings.approvalPolicyHelp",
  "tui.theme": "clientConfig.settings.themeHelp",
  "tui.auto_recap": "clientConfig.settings.recapHelp",
  "tui.notification_method": "clientConfig.settings.notificationMethodHelp",
  "tui.notification_condition": "clientConfig.settings.notificationConditionHelp",
  "permissions.defaultMode": "clientConfig.settings.permissionModeHelp",
  "permissions.allow": "clientConfig.settings.permissionsHelp",
  "permissions.ask": "clientConfig.settings.permissionsHelp",
  "permissions.deny": "clientConfig.settings.permissionsHelp",
  "permissions.additionalDirectories": "clientConfig.settings.directoriesHelp",
  "sandbox.allowUnsandboxedCommands": "clientConfig.settings.unsandboxedHelp",
  "sandbox.failIfUnavailable": "clientConfig.settings.failSandboxHelp",
  "defaultShell": "clientConfig.settings.defaultShellHelp",
  "plansDirectory": "clientConfig.settings.plansHelp",
  "cleanupPeriodDays": "clientConfig.settings.cleanupHelp",
  "attribution.commit": "clientConfig.settings.emptyAttribution",
  "attribution.pr": "clientConfig.settings.emptyAttribution",
  "outputStyle": "clientConfig.settings.outputStyleHelp",
  "features.ultrafast_mode": "clientConfig.settings.ultrafastHelp",
  "model_post_turn_compact_threshold_percent": "clientConfig.settings.postTurnCompactHelp",
  "maxEffortLevel": "clientConfig.settings.effortCapHelp",
  "promptCacheTtl": "clientConfig.settings.cacheTtlHelp",
  "subagentPromptCacheTtl": "clientConfig.settings.cacheTtlHelp",
} as const satisfies Record<string, MessageKey>;

export function settingDetail(key: string, t: TFunction): string | undefined {
  const message = details[key as keyof typeof details];
  if (message) return t(message);
  if (key.startsWith("sandbox.filesystem.")) return t("clientConfig.settings.sandboxPathsHelp");
  if (key.startsWith("sandbox.network.")) return t("clientConfig.settings.sandboxDomainsHelp");
  return undefined;
}
