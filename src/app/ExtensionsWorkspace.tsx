import { ExtensionsPage } from "../pages/ExtensionsPage";
import type { SwitchboardModel } from "./useSwitchboardModel";

type ExtensionsWorkspaceModel = Pick<SwitchboardModel,
  "extensionSection" | "setExtensionSection" | "busy" | "setBusy" | "clearError" | "reportError"
>;

export function ExtensionsWorkspace({ model, active }: { model: ExtensionsWorkspaceModel; active: boolean }) {
  return (
    <ExtensionsPage active={active} section={model.extensionSection} onSectionChange={model.setExtensionSection}
      busy={model.busy} setBusy={model.setBusy} clearError={model.clearError} onError={model.reportError} />
  );
}
