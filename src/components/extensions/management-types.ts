import type {
  BindingStatus,
  ClientCapabilityReport,
  ExtensionListItem,
  ProjectRegistration,
  SkillUpdateReport,
  TargetOutcome,
} from "../../api/client";
import type { CommandError } from "../../api/shared";
import type { McpConnectionCheck } from "../../pages/extensions/useMcpConnectionCheck";
import type { McpRegistryUpdateReport } from "../../api/extensions/registry";

export type BindingFeedback = TargetOutcome | CommandError;

export interface ExtensionManagementProps {
  item: ExtensionListItem;
  projects: ProjectRegistration[];
  capabilities: ClientCapabilityReport[];
  busy: boolean;
  pendingBindings: ReadonlySet<string>;
  bindingFeedback: ReadonlyMap<string, BindingFeedback>;
  check: McpConnectionCheck;
  installTargets: string[];
  updateReport: SkillUpdateReport | null;
  registryUpdate?: McpRegistryUpdateReport;
  projectNames: ReadonlyMap<string, string>;
  onInstallTargetsChange: (values: string[]) => void;
  onInstall: (targets: string[]) => void;
  onChangeBinding: (binding: BindingStatus, enable: boolean) => void;
  onRemoveBinding: (binding: BindingStatus) => void;
  onToggleLock: (binding: BindingStatus, locked: boolean) => void;
  onDelete: () => void;
  onCheckUpdates: () => void;
  onApplyUpdate: () => void;
  onEditUpdateConfig: () => void;
  onDeployCurrent: () => void;
  onExportPortable?: () => void;
}
