import type { RefCallback } from "react";
import { ClientSettingsPanel } from "../components/ClientSettingsPanel";
import { CodexSubagentSettingsPanel } from "../components/CodexSubagentSettingsPanel";
import { GlobalPromptManager } from "../components/GlobalPromptManager";
import { useI18n } from "../i18n";
import type { SwitchboardModel } from "./useSwitchboardModel";
import type { AppKind } from "../api/client";
import { useClientRouteTransition } from "./useClientRouteTransition";

export function ClientConfigurationWorkspace({ model, active }: { model: SwitchboardModel; active: boolean }) {
  const route = useClientRouteTransition({
    app: model.appFilter, active, onSelectApp: model.providers.selectApp, revealPicker: true,
  });
  return <div ref={route.workspaceRef} className="asb-client-route-workspace asb-client-configuration-workspace">
    {(["codex", "claude"] as const).map((app) => {
      const leaving = route.leavingApp === app;
      return <div key={app} className="asb-client-configuration-route" data-client-route={app}
        data-client-route-leaving={leaving || undefined}
        hidden={model.appFilter !== app && !leaving}
        inert={leaving || undefined} aria-hidden={leaving || undefined}>
        <ClientConfigurationPanel model={model} app={app} active={active && model.appFilter === app}
          onSelectApp={route.selectApp} routeContentRef={route.contentRefs[app]} />
      </div>;
    })}
  </div>;
}

function ClientConfigurationPanel({ model, app, active, onSelectApp, routeContentRef }: {
  model: SwitchboardModel;
  app: AppKind;
  active: boolean;
  onSelectApp: (app: AppKind) => void;
  routeContentRef: RefCallback<HTMLElement>;
}) {
  const { t } = useI18n();
  const {
    clientSettings: settings,
    codexSubagentSettings,
    promptDocuments: prompts,
    snapshot,
    busy,
  } = model;
  const promptDocument = prompts.documents[app];
  return (
    <section className="asb-panel" aria-label={t("clientConfig.title")}>
      <ClientSettingsPanel
        active={active}
        app={app}
        routeContentRef={routeContentRef}
        onSelectApp={onSelectApp}
        editorState={settings.states[app]}
        busy={busy}
        configStatus={snapshot.statuses?.find((status) => status.app === app)}
        hasUnsavedConfigurationDraft={settings.states[app].phase === "dirty" ||
          (app === "codex" && codexSubagentSettings.editorState.phase === "dirty")}
        onValueChange={settings.changeValue}
        onKeepFileValue={settings.keepFileValue}
        onPermissionProfileChange={settings.changePermissionProfile}
        onDiscardSettings={() => {
          settings.reloadFromStored(app);
          if (app === "codex") codexSubagentSettings.reloadFromFile();
        }}
        onApplied={() => {
          settings.reloadFromStored(app);
          if (app === "codex") codexSubagentSettings.reloadFromFile();
          model.providerSwitch.invalidateCandidates();
          void snapshot.refresh();
        }} onRetryLoad={settings.retryLoad}
        onFileApplied={() => {
          if (settings.states[app].phase !== "dirty") settings.reloadFromStored(app);
          else settings.reviewConfiguration(app);
          if (app === "codex" && codexSubagentSettings.editorState.phase !== "dirty") codexSubagentSettings.reloadFromFile();
          model.providerSwitch.invalidateCandidates();
          void snapshot.refresh();
        }}
        onReview={settings.reviewConfiguration}
        onClaudeExtraChange={settings.changeClaudeExtra}
        globalInstructions={
          <GlobalPromptManager document={promptDocument}
            draft={prompts.drafts[app] ?? promptDocument?.content ?? ""}
            dirty={prompts.isPromptDirty(app)} busy={busy}
            onChange={(content) => prompts.setPromptDraft(app, content)}
            onSave={() => void prompts.savePromptDocument(app)}
            onDiscard={() => prompts.discardPromptDraft(app)}
            onReload={() => prompts.reloadPromptDocument(app)} />
        }
        globalInstructionsDirty={prompts.isPromptDirty(app)}
        onDiscardGlobalInstructions={() => prompts.discardPromptDraft(app)}
        subagentDraft={app === "codex" ? codexSubagentSettings.editorState.draft : undefined}
        subagentBaseline={app === "codex" ? codexSubagentSettings.editorState.snapshot : undefined}
        renderSubagentSettings={app === "codex" ? (settingsBusy) => (
          <CodexSubagentSettingsPanel
            editorState={codexSubagentSettings.editorState}
            busy={settingsBusy}
            onChange={codexSubagentSettings.changeValue}
            onRetryLoad={codexSubagentSettings.retryLoad}
          />
        ) : undefined}
      />
    </section>
  );
}
