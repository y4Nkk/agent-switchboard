import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { AppKind, GlobalPromptDocument } from "../api/client";
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

function PromptHarness({ onSave = vi.fn() }: { onSave?: (app: AppKind) => void }) {
  const [promptApp, setPromptApp] = useState<AppKind>("codex");
  const [drafts, setDrafts] = useState<Record<AppKind, string>>({
    codex: documents.codex.content,
    claude: documents.claude.content,
  });
  return (
    <CommonSettingsPage
      app="codex"
      onSelectApp={() => {}}
      toggles={[]}
      choices={{ groups: [], choices: [] }}
      commonPreview={null}
      busy={false}
      onApplyLine={() => {}}
      promptApp={promptApp}
      promptDocument={documents[promptApp]}
      promptDraft={drafts[promptApp]}
      promptDirty={drafts[promptApp] !== documents[promptApp].content}
      onSelectPromptApp={setPromptApp}
      onPromptDraftChange={(content) => setDrafts((current) => ({ ...current, [promptApp]: content }))}
      onSavePrompt={() => onSave(promptApp)}
      onDiscardPrompt={() =>
        setDrafts((current) => ({ ...current, [promptApp]: documents[promptApp].content }))
      }
      onReloadPrompt={() => {}}
    />
  );
}

describe("CommonSettingsPage", () => {
  it("keeps model configuration and prompt management behind independent main tabs", async () => {
    const user = userEvent.setup();
    render(<PromptHarness />);

    expect(screen.getByRole("tab", { name: "模型配置" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tab", { name: "提示词管理" })).toHaveAttribute("aria-selected", "false");

    await user.click(screen.getByRole("tab", { name: "提示词管理" }));

    expect(screen.getByRole("tab", { name: "提示词管理" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("textbox", { name: "AGENTS.md 内容" })).toHaveValue(
      "# Codex global instructions\n",
    );

    await user.click(screen.getByRole("tab", { name: "Claude" }));

    expect(screen.getByRole("textbox", { name: "CLAUDE.md 内容" })).toHaveValue(
      "# Claude global instructions\n",
    );
  });

  it("keeps document actions separate from the model controls", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(<PromptHarness onSave={onSave} />);

    await user.click(screen.getByRole("tab", { name: "提示词管理" }));
    const editor = screen.getByRole("textbox", { name: "AGENTS.md 内容" });
    await user.type(editor, "- Keep the scope narrow.\n");

    expect(screen.getByRole("button", { name: "放弃草稿" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "保存 AGENTS.md" })).toBeEnabled();

    await user.click(screen.getByRole("button", { name: "保存 AGENTS.md" }));

    expect(onSave).toHaveBeenCalledWith("codex");
  });

  it("allows retrying a prompt document that did not load", async () => {
    const user = userEvent.setup();
    const onReload = vi.fn();
    render(
      <GlobalPromptManager
        app="codex"
        document={undefined}
        draft=""
        dirty={false}
        busy={false}
        onSelectApp={() => {}}
        onChange={() => {}}
        onSave={() => {}}
        onDiscard={() => {}}
        onReload={onReload}
      />,
    );

    const reload = screen.getByRole("button", { name: "重新读取" });
    expect(reload).toBeEnabled();
    await user.click(reload);
    expect(onReload).toHaveBeenCalledOnce();
  });
});
