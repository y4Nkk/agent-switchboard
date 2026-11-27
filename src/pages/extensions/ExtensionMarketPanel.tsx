import { pickDirectory, pickFile } from "../../api/client";
import type { ExtensionSection } from "../../app/navigation";
import { SkillSourceBrowser } from "../../components/extensions/SkillSourceBrowser";
import { McpRegistryBrowser } from "./McpRegistryBrowser";
import type { ExtensionWorkspace } from "./useExtensionWorkspace";

export function ExtensionMarketPanel({ workspace: w, kind, active }: {
  workspace: ExtensionWorkspace; kind: ExtensionSection; active: boolean;
}) {
  if (kind === "mcp") return <McpRegistryBrowser workspace={w} active={active} />;
  return <SkillSourceBrowser active={active} busy={w.writeBlocked} items={w.items} onScanLocal={w.ext.scanLocal}
    onImport={w.importCandidate} onPickDirectory={pickDirectory} onPickZip={() => pickFile("ZIP archives", ["zip"])} />;
}
