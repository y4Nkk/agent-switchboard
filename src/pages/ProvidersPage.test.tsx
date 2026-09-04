import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { FilePreview, ProviderProfile } from "../api/client";
import { ProvidersPage } from "./ProvidersPage";

const profile: ProviderProfile = {
  id: "relay-a",
  app: "codex",
  routeMode: "custom",
  name: "中继 A",
  model: null,
  baseUrl: "https://relay.example/v1",
  apiKey: "test-key",
  modelOptions: null,
  websiteUrl: null,
  usageQuery: null,
};

const previewFile: FilePreview = {
  contentHash: "hash-1",
  renderedHash: "rendered-1",
  content: `model = "gpt-5.4"\n`,
  preview: {
    app: "codex",
    target: "C:/Users/test/.codex/config.toml",
    changes: [{ key: "model", kind: "set", before: null, after: "gpt-5.4" }],
    warnings: [],
    backupDir: "C:/backups",
  },
};

type PageProps = Parameters<typeof ProvidersPage>[0];

function renderPage(overrides: Partial<PageProps> = {}) {
  return render(
    <ProvidersPage
      profiles={[profile]}
      appFilter="codex"
      activeProfileId={null}
      userConfigModel={null}
      userConfigWarnings={[]}
      selectedId={null}
      selectedProfile={null}
      editorMode={null}
      preview={null}
      busy={false}
      collapsedUsageIds={[]}
      onSelectApp={() => {}}
      onNew={() => {}}
      onCloseEditor={() => {}}
      onSave={async () => {}}
      onSaveUsageQuery={async () => true}
      onSelect={() => {}}
      onReorder={() => {}}
      onToggleUsage={() => {}}
      onActivate={() => {}}
      onTogglePreview={() => {}}
      onEdit={() => {}}
      onDelete={() => {}}
      onRequestSwitch={() => {}}
      onCancelPreview={() => {}}
      {...overrides}
    />,
  );
}

describe("ProvidersPage", () => {
  it("opens and saves the independent usage workspace from a provider card", async () => {
    const user = userEvent.setup();
    const onSaveUsageQuery = vi.fn(async () => true);
    renderPage({ onSaveUsageQuery });

    await user.click(screen.getByRole("button", { name: "配置 中继 A 用量" }));
    expect(screen.getByRole("region", { name: "用量查询" })).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("用量查询地址"), {
      target: { value: "{{baseUrl}}/balance" },
    });
    fireEvent.change(screen.getByLabelText("余额提取路径"), {
      target: { value: "data/balance" },
    });
    await user.click(screen.getByRole("button", { name: "保存查询" }));

    expect(onSaveUsageQuery).toHaveBeenCalledWith(profile, {
      kind: "declarative",
      url: "{{baseUrl}}/balance",
      remainingPath: "data/balance",
      usedPath: null,
      totalPath: null,
      unit: null,
      refreshIntervalMinutes: 0,
    });
    expect(await screen.findByRole("region", { name: "供应商工作区" })).toBeInTheDocument();
  });

  it("cancels or confirms the pending switch from the preview header", async () => {
    const user = userEvent.setup();
    const onCancelPreview = vi.fn();
    const onRequestSwitch = vi.fn();
    renderPage({
      preview: { profileId: profile.id, file: previewFile },
      userConfigModel: "gpt-5.3-codex",
      userConfigWarnings: ["使用 --profile 启动时会覆盖这里的用户级设置"],
      onCancelPreview,
      onRequestSwitch,
    });

    const previewPanel = screen.getByRole("region", { name: "变更预览" });
    expect(within(previewPanel).getByText("当前用户级配置模型")).toBeInTheDocument();
    expect(within(previewPanel).getByText("gpt-5.3-codex")).toBeInTheDocument();
    expect(
      within(previewPanel).getByText("使用 --profile 启动时会覆盖这里的用户级设置"),
    ).toBeInTheDocument();
    await user.click(within(previewPanel).getByRole("button", { name: "取消" }));
    expect(onCancelPreview).toHaveBeenCalledTimes(1);

    await user.click(within(previewPanel).getByRole("button", { name: "确认切换" }));
    expect(onRequestSwitch).toHaveBeenCalledTimes(1);
  });
});
