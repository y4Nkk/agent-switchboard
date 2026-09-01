import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ProviderProfile } from "../api/client";
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
  usageQuery: null,
};

describe("ProvidersPage", () => {
  it("opens and saves the independent usage workspace from a provider card", async () => {
    const user = userEvent.setup();
    const onSaveUsageQuery = vi.fn(async () => true);
    render(
      <ProvidersPage
        profiles={[profile]}
        appFilter="codex"
        activeProfileId={null}
        selectedId={null}
        selectedProfile={null}
        editorMode={null}
        preview={null}
        busy={false}
        onSelectApp={() => {}}
        onNew={() => {}}
        onCloseEditor={() => {}}
        onSave={async () => {}}
        onSaveUsageQuery={onSaveUsageQuery}
        onSelect={() => {}}
        onReorder={() => {}}
        onActivate={() => {}}
        onTogglePreview={() => {}}
        onEdit={() => {}}
        onDelete={() => {}}
        onRequestSwitch={() => {}}
      />,
    );

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
    });
    expect(await screen.findByRole("region", { name: "供应商工作区" })).toBeInTheDocument();
  });
});
