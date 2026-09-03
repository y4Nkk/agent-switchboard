import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type {
  AppKind,
  ConfigFileStatus,
  GlobalPromptDocument,
} from "../api/client";
import type { CommonSettingsEditorState } from "../app/useCommonSettings";
import { GlobalPromptManager } from "../components/GlobalPromptManager";
import { CommonSettingsPage } from "./CommonSettingsPage";

const documents: Record<AppKind, GlobalPromptDocument> = {
  codex: {
    app: "codex",
    fileName: "AGENTS.md",
    content: "# Codex global instructions\n",
    contentHash: "codex-hash",
    exists: true,
  },
  claude: {
    app: "claude",
    fileName: "CLAUDE.md",
    content: "# Claude global instructions\n",
    contentHash: "claude-hash",
    exists: true,
  },
};

const cleanSettings: CommonSettingsEditorState = {
  phase: "clean",
  editor: {
    app: "codex",
    settings: { settings: { hide_agent_reasoning: { mode: "automatic" } } },
    settingsHash: "settings-hash",
    groups: ["模型行为"],
    specs: [
      {
        key: "hide_agent_reasoning",
        label: "隐藏推理摘要",
        group: "模型行为",
        control: "toggle",
        options: [],
      },
    ],
    directory: [
      {
        title: "隐藏推理摘要",
        paths: ["hide_agent_reasoning"],
        disposition: "direct",
        detail: "通过基础参数编辑。",
      },
      {
        title: "全局指令",
        paths: ["$CODEX_HOME/AGENTS.md"],
        disposition: "separateModule",
        detail: "通过独立文档事务管理。",
      },
      {
        title: "MCP 服务器",
        paths: ["mcp_servers.<id>"],
        disposition: "preserveOnly",
        detail: "当前保留但不写入。",
      },
    ],
  },
  draft: { hide_agent_reasoning: { mode: "automatic" } },
};

const appliedStatus: ConfigFileStatus = {
  app: "codex",
  path: "C:/Users/test/.codex/config.toml",
  exists: true,
  syntaxOk: true,
  route: null,
  readError: null,
  matchStatus: { kind: "matchesProfile", profileId: "gateway", profileName: "网关" },
  lastSwitch: null,
};

function PromptHarness({
  editorState = cleanSettings,
  configStatus,
  hasActiveProvider = true,
  onSave = vi.fn(),
  onRetryLoad = vi.fn(),
  onPreview = vi.fn(),
  onResetGroup = vi.fn(),
}: {
  editorState?: CommonSettingsEditorState;
  configStatus?: ConfigFileStatus;
  hasActiveProvider?: boolean;
  onSave?: (app: AppKind) => void;
  onRetryLoad?: (app: AppKind) => void;
  onPreview?: (app: AppKind) => void;
  onResetGroup?: (app: AppKind, group: string | null) => void;
}) {
  const [app, setApp] = useState<AppKind>("codex");
  const [drafts, setDrafts] = useState<Record<AppKind, string>>({
    codex: documents.codex.content,
    claude: documents.claude.content,
  });
  return (
    <CommonSettingsPage
      app={app}
      onSelectApp={setApp}
      editorState={editorState}
      configStatus={configStatus}
      busy={false}
      hasActiveProvider={hasActiveProvider}
      onValueChange={() => {}}
      onResetGroup={onResetGroup}
      onSave={onSave}
      onRetryLoad={onRetryLoad}
      onPreview={onPreview}
      promptDocument={documents[app]}
      promptDraft={drafts[app]}
      promptDirty={drafts[app] !== documents[app].content}
      onPromptDraftChange={(content) => setDrafts((current) => ({ ...current, [app]: content }))}
      onSavePrompt={() => onSave(app)}
      onDiscardPrompt={() =>
        setDrafts((current) => ({ ...current, [app]: documents[app].content }))
      }
      onReloadPrompt={() => {}}
    />
  );
}

describe("CommonSettingsPage", () => {
  it("keeps base parameters, the official settings directory, and global prompts behind independent main tabs", async () => {
    const user = userEvent.setup();
    render(<PromptHarness configStatus={appliedStatus} />);

    expect(screen.getByRole("tab", { name: "基础参数", selected: true })).toBeInTheDocument();
    expect(screen.getByText("隐藏推理摘要")).toBeInTheDocument();
    expect(screen.getByText("已应用：真实配置与「网关」一致")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "查看通用配置预览" })).toHaveClass(
      "asb-btn-secondary",
    );

    await user.click(screen.getByRole("tab", { name: "官方设置目录" }));
    expect(screen.getByRole("region", { name: "官方设置目录" })).toHaveTextContent("MCP 服务器");
    expect(screen.getByText("保留不写入")).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "AGENTS.md 内容" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "全局指令" }));
    expect(screen.getByRole("textbox", { name: "AGENTS.md 内容" })).toHaveValue(
      "# Codex global instructions\n",
    );

    await user.click(screen.getByRole("tab", { name: "Claude" }));
    expect(screen.getByRole("textbox", { name: "CLAUDE.md 内容" })).toHaveValue(
      "# Claude global instructions\n",
    );
  });

  it("keeps save feedback and the page-wide reset in one footer toolbar", async () => {
    const user = userEvent.setup();
    const onResetGroup = vi.fn();
    const { rerender } = render(
      <PromptHarness
        configStatus={{ ...appliedStatus, matchStatus: { kind: "externallyModified", at: "now" } }}
        onResetGroup={onResetGroup}
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("真实配置已被外部修改");
    expect(screen.getByRole("button", { name: "全部恢复默认值" })).toHaveClass(
      "asb-btn-secondary",
    );
    await user.click(screen.getByRole("button", { name: "全部恢复默认值" }));
    expect(onResetGroup).toHaveBeenCalledWith("codex", null);

    rerender(
      <PromptHarness
        editorState={{ ...cleanSettings, phase: "savedPendingReapply" }}
        hasActiveProvider={false}
        onResetGroup={onResetGroup}
      />,
    );
    expect(screen.getByText("已保存，请在供应商页选择并启用供应商后生效")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "前往供应商页" })).not.toBeInTheDocument();
  });

  it("makes load failure, retry, dirty state, and save action visible", async () => {
    const user = userEvent.setup();
    const onRetryLoad = vi.fn();
    const onSave = vi.fn();
    const { rerender } = render(
      <PromptHarness
        editorState={{ phase: "loadError", error: { code: "store-unreadable", message: "应用数据不可读" } }}
        onRetryLoad={onRetryLoad}
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("应用数据不可读");
    await user.click(screen.getByRole("button", { name: "重新读取" }));
    expect(onRetryLoad).toHaveBeenCalledWith("codex");

    rerender(<PromptHarness editorState={{ ...cleanSettings, phase: "dirty" }} onSave={onSave} />);
    expect(screen.getByText("有未保存修改")).toBeInTheDocument();
    const save = screen.getByRole("button", { name: "保存通用设置" });
    expect(save).toHaveClass("asb-btn-primary");
    await user.click(save);
    expect(onSave).toHaveBeenCalledWith("codex");
  });

  it("shows the backend-rendered common-settings fragment on demand and folds it with the same button", async () => {
    const user = userEvent.setup();
    const onPreview = vi.fn();
    function DemandHarness() {
      const [preview, setPreview] = useState<CommonSettingsEditorState["preview"]>();
      return (
        <PromptHarness
          editorState={preview ? { ...cleanSettings, preview } : cleanSettings}
          onPreview={(target) => {
            onPreview(target);
            setPreview({
              app: "codex",
              target: "~/.codex/config.toml 通用配置片段",
              content: "hide_agent_reasoning = true\n",
            });
          }}
        />
      );
    }
    render(<DemandHarness />);

    await user.click(screen.getByRole("button", { name: "查看通用配置预览" }));
    expect(onPreview).toHaveBeenCalledWith("codex");
    expect(
      screen.getByLabelText("~/.codex/config.toml 通用配置片段 配置预览"),
    ).toHaveTextContent("hide_agent_reasoning = true");
    const collapse = screen.getByRole("button", { name: "收起通用配置预览" });
    expect(collapse).toHaveAttribute("aria-expanded", "true");

    await user.click(collapse);
    expect(
      screen.queryByLabelText("~/.codex/config.toml 通用配置片段 配置预览"),
    ).toBeNull();
    const expand = screen.getByRole("button", { name: "展开通用配置预览" });
    expect(expand).toHaveAttribute("aria-expanded", "false");

    await user.click(expand);
    expect(
      screen.getByLabelText("~/.codex/config.toml 通用配置片段 配置预览"),
    ).toHaveTextContent("hide_agent_reasoning = true");
    // Expanding shows the cached fragment; only the first click renders.
    expect(onPreview).toHaveBeenCalledTimes(1);
  });

  it("keeps document actions separate from parameter controls", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(<PromptHarness onSave={onSave} />);

    await user.click(screen.getByRole("tab", { name: "全局指令" }));
    await user.type(screen.getByRole("textbox", { name: "AGENTS.md 内容" }), "- Keep scope narrow.\n");
    await user.click(screen.getByRole("button", { name: "保存 AGENTS.md" }));
    expect(onSave).toHaveBeenCalledWith("codex");
  });

  it("allows retrying a prompt document that did not load", async () => {
    const user = userEvent.setup();
    const onReload = vi.fn();
    render(
      <GlobalPromptManager
        document={undefined}
        draft=""
        dirty={false}
        busy={false}
        onChange={() => {}}
        onSave={() => {}}
        onDiscard={() => {}}
        onReload={onReload}
      />,
    );

    await user.click(screen.getByRole("button", { name: "重新读取" }));
    expect(onReload).toHaveBeenCalledOnce();
  });
});
