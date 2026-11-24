import { useRef, useState } from "react";
import type { ExtensionKind, ExtensionListItem, McpEditViewEnvelope } from "../../api/client";
import type { McpRegistrySource } from "../../api/extensions/mcp";
import type { McpRegistryEntry, McpRegistryUpdate } from "../../api/extensions/registry";
import type { ExtensionSection } from "../../app/navigation";

export type ExtensionDialogView =
  | { type: "management" | "skillEditor"; definitionId: string }
  | { type: "mcpEditor"; envelope: McpEditViewEnvelope }
  | { type: "newMcp"; registryEntry?: McpRegistryEntry & { source: McpRegistrySource } }
  | { type: "mcpRegistryUpdate"; report: McpRegistryUpdate }
  | { type: "remove" | "export"; item: ExtensionListItem }
  | { type: "newSkill" | "portableImport" | "project" | "history" };

export interface ExtensionNavigation {
  section: ExtensionSection;
  onSectionChange: (section: ExtensionSection) => void;
}

export function useExtensionView({ section, onSectionChange }: ExtensionNavigation) {
  const [searches, setSearches] = useState<Record<ExtensionSection, string>>({ skill: "", mcp: "" });
  const [view, changeView] = useState<"library" | "market">("library");
  const marketButton = useRef<HTMLButtonElement>(null);
  const [discoveryOpen, setDiscoveryOpen] = useState(false);
  const [dialog, setDialog] = useState<ExtensionDialogView | null>(null);
  const setSearch = (search: string) => setSearches((current) => ({ ...current, [section]: search }));
  const openManagement = (id: string, nextKind: ExtensionKind) => {
    if (view === "library" && !discoveryOpen) onSectionChange(nextKind);
    setDialog({ type: "management", definitionId: id });
  };
  const openSkillEditor = (id: string) => {
    if (view === "library" && !discoveryOpen) onSectionChange("skill");
    setDialog({ type: "skillEditor", definitionId: id });
  };
  return {
    section,
    kind: section,
    changeSection: onSectionChange,
    search: searches[section],
    setSearch,
    view,
    changeView,
    marketButton,
    openMarket: () => {
      changeView("market");
      marketButton.current?.focus();
    },
    discoveryOpen,
    setDiscoveryOpen,
    dialog,
    setDialog,
    closeDialog: () => setDialog(null),
    openManagement,
    openSkillEditor,
    clearFilters: () => setSearch(""),
  };
}
