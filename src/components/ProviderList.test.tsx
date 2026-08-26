import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ProviderList } from "./ProviderList";
import type { ProviderProfile } from "../api/client";

const profiles: ProviderProfile[] = [
  {
    id: "codex-relay-a",
    app: "codex",
    mode: "custom",
    name: "中继 A",
    model: "gpt-5.1",
    baseUrl: "https://relay-a.internal/v1",
    envKey: "ASB_RELAY_A_KEY",
    modelOptions: null,
  },
  {
    id: "codex-official",
    app: "codex",
    mode: "official",
    name: "官方 OpenAI",
    model: null,
    baseUrl: null,
    envKey: null,
    modelOptions: null,
  },
];

describe("ProviderList", () => {
  it("renders rows with model and an explicit routing-mode label", () => {
    render(
      <ProviderList profiles={profiles} activeProfileId={null} selectedId="codex-relay-a" onSelect={() => {}} />,
    );
    expect(screen.getByRole("option", { name: /官方 OpenAI/ })).toBeInTheDocument();
    expect(screen.getByText("自定义服务")).toBeInTheDocument();
    expect(screen.getByText("官方登录")).toBeInTheDocument();
  });

  it("marks the matching row with text and a dot, not color alone", () => {
    render(
      <ProviderList profiles={profiles} activeProfileId="codex-relay-a" selectedId={null} onSelect={() => {}} />,
    );
    expect(screen.getAllByText("当前", { exact: false }).length).toBeGreaterThan(0);
  });

  it("selects a row on click", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(
      <ProviderList profiles={profiles} activeProfileId={null} selectedId={null} onSelect={onSelect} />,
    );
    await user.click(screen.getByRole("option", { name: /官方 OpenAI/ }));
    expect(onSelect).toHaveBeenCalledWith("codex-official");
  });
});
