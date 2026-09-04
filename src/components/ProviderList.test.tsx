import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import userEvent from "@testing-library/user-event";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ProviderList as ProviderListComponent } from "./ProviderList";
import type { ProviderProfile } from "../api/client";

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);
const baseCss = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "../styles/base.css"),
  "utf8",
);

const profiles: ProviderProfile[] = [
  {
    id: "codex-relay-a",
    app: "codex",
    routeMode: "custom",
    name: "中继 A",
    model: "gpt-5.1",
    baseUrl: "https://relay-a.internal/v1",
    websiteUrl: "https://relay-a.example",
    apiKey: "ASB_RELAY_A_KEY",
    modelOptions: null,
  },
  {
    id: "codex-official",
    app: "codex",
    routeMode: "custom",
    name: "官方 OpenAI",
    model: null,
    baseUrl: "https://api.openai.com/v1",
    websiteUrl: "https://openai.com",
    apiKey: "test-api-key",
    modelOptions: null,
  },
];

function ProviderList({
  userConfigModel = null,
  ...props
}: Omit<ComponentProps<typeof ProviderListComponent>, "userConfigModel"> & {
  userConfigModel?: string | null;
}) {
  return <ProviderListComponent {...props} userConfigModel={userConfigModel} />;
}

describe("ProviderList", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("renders model and website host together on the meta line", () => {
    render(
      <ProviderList profiles={profiles} activeProfileId={null} selectedId="codex-relay-a" onSelect={() => {}} />,
    );
    expect(screen.getByRole("option", { name: /官方 OpenAI/ })).toBeInTheDocument();
    const link = screen.getByRole("link", { name: "relay-a.example" });
    expect(link).toHaveAttribute("href", "https://relay-a.example");
    expect(link.classList.contains("asb-row-host")).toBe(true);
    expect(link.closest(".asb-row-meta")?.textContent).toBe("gpt-5.1 · relay-a.example");
    expect(screen.getByText("openai.com")).toBeInTheDocument();
    expect(screen.queryByText("relay-a.internal")).not.toBeInTheDocument();
    expect(screen.queryByText("api.openai.com")).not.toBeInTheDocument();
  });

  it("opens the configured website in the system browser on click, not in-app", async () => {
    const user = userEvent.setup();
    vi.mocked(openUrl).mockClear();
    render(
      <ProviderList profiles={profiles} activeProfileId={null} selectedId={null} onSelect={() => {}} />,
    );
    await user.click(screen.getByRole("link", { name: "relay-a.example" }));
    expect(openUrl).toHaveBeenCalledWith("https://relay-a.example");
  });

  it("does not render a substitute when no website is configured", () => {
    render(
      <ProviderList
        profiles={[{ ...profiles[0], websiteUrl: null }]}
        activeProfileId={null}
        selectedId={null}
        onSelect={() => {}}
      />,
    );

    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    expect(screen.queryByText("relay-a.internal")).not.toBeInTheDocument();
    expect(screen.queryByText("官网未设置")).not.toBeInTheDocument();
  });

  it("marks the live-matched row with a status pill, not color alone", () => {
    render(
      <ProviderList profiles={profiles} activeProfileId="codex-relay-a" selectedId={null} onSelect={() => {}} />,
    );
    expect(screen.getByText("使用中")).toBeInTheDocument();
  });

  it("shows the user-level configuration model instead of the stored profile model on the active row", () => {
    render(
      <ProviderList
        profiles={profiles}
        activeProfileId="codex-relay-a"
        userConfigModel="gpt-5.4"
        selectedId={null}
        onSelect={() => {}}
      />,
    );

    const activeRow = screen.getByRole("option", { name: /中继 A/ });
    expect(activeRow.querySelector(".asb-row-meta")).toHaveTextContent(
      "当前用户级配置模型：gpt-5.4",
    );
    expect(activeRow.querySelector(".asb-row-meta")).not.toHaveTextContent("gpt-5.1");
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

  it("offers re-login and edit on official rows", async () => {
    const user = userEvent.setup();
    const onEdit = vi.fn();
    render(
      <ProviderList
        profiles={[
          {
            id: "codex-official-login",
            app: "codex",
            routeMode: "official",
            name: "Codex 官方登录",
            model: null,
            baseUrl: null,
            apiKey: "",
            modelOptions: null,
            websiteUrl: null,
          },
        ]}
        activeProfileId={null}
        selectedId={null}
        onSelect={() => {}}
        onEdit={onEdit}
      />,
    );

    expect(screen.getByText("官方登录")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "编辑 Codex 官方登录" }));
    expect(onEdit).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole("button", { name: "重新登录 Codex 官方登录" }));

    expect(await screen.findByRole("button", { name: "开始官方登录" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "收起 Codex 官方登录 登录" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
  });

  it("keeps selection semantic without adding a card highlight class", () => {
    render(
      <ProviderList
        profiles={profiles}
        activeProfileId={null}
        selectedId="codex-official"
        onSelect={() => {}}
      />,
    );

    const selectedRow = screen.getByRole("option", { name: /官方 OpenAI/ });
    expect(selectedRow).toHaveAttribute("aria-selected", "true");
    const selectedCard = selectedRow.closest("li");
    expect(selectedCard).toHaveClass("is-selected");
    expect(selectedCard?.querySelector(".asb-starlight")).toHaveAttribute(
      "data-active",
      "true",
    );
    expect(selectedCard?.querySelector(".asb-starlight")).toHaveAttribute(
      "data-variant",
      "warm",
    );
  });

  it("swaps the preview eye for a closed-eye toggle when that row's preview is open", async () => {
    const user = userEvent.setup();
    const onPreview = vi.fn();
    const { rerender } = render(
      <ProviderList
        profiles={profiles}
        activeProfileId={null}
        selectedId={null}
        onSelect={() => {}}
        onPreview={onPreview}
      />,
    );
    const eye = screen.getByRole("button", { name: "预览 中继 A 变更" });
    expect(eye).toHaveAttribute("aria-expanded", "false");
    await user.click(eye);
    expect(onPreview).toHaveBeenCalledWith(profiles[0]);

    rerender(
      <ProviderList
        profiles={profiles}
        activeProfileId={null}
        selectedId={null}
        openPreviewId="codex-relay-a"
        onSelect={() => {}}
        onPreview={onPreview}
      />,
    );
    const eyeOff = screen.getByRole("button", { name: "收起 中继 A 预览" });
    expect(eyeOff).toHaveAttribute("aria-expanded", "true");
    expect(screen.queryByRole("button", { name: "预览 中继 A 变更" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "预览 官方 OpenAI 变更" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
  });

  it("unfolds preview content inside the previewed row's own card", () => {
    render(
      <ProviderList
        profiles={profiles}
        activeProfileId={null}
        selectedId={null}
        openPreviewId="codex-relay-a"
        onSelect={() => {}}
        renderPreview={(profile) => (
          <div className="asb-preview-inline">{`预览内容 ${profile.id}`}</div>
        )}
      />,
    );
    const row = screen.getByRole("option", { name: /中继 A/ }).closest("li");
    expect(row).toHaveClass("is-previewing");
    expect(screen.getByRole("option", { name: /官方 OpenAI/ }).closest("li")).not.toHaveClass(
      "is-previewing",
    );
    expect(row?.querySelector(".asb-row-line + .asb-preview-inline")).toHaveTextContent(
      "预览内容 codex-relay-a",
    );
    expect(screen.getAllByText(/预览内容/)).toHaveLength(1);
  });

  it("shows a configured usage ledger inside its provider card by default", async () => {
    const user = userEvent.setup();
    const onToggleUsage = vi.fn();
    const configured = {
      ...profiles[0],
      usageQuery: {
        kind: "declarative" as const,
        url: "{{baseUrl}}/balance",
        remainingPath: "balance",
        unit: "USD",
        refreshIntervalMinutes: 0,
      },
    };
    invokeMock.mockResolvedValue({
      readings: [{ remaining: 18.5, used: 7, total: 25.5, unit: "USD" }],
      at: "2026-08-31T08:00:00Z",
    });
    render(
      <ProviderList
        profiles={[configured, profiles[1]]}
        activeProfileId={null}
        selectedId={null}
        onSelect={() => {}}
        onToggleUsage={onToggleUsage}
      />,
    );

    const toggle = screen.getByRole("button", { name: "收起 中继 A 用量" });
    expect(toggle).toHaveClass("asb-btn-icon");
    expect(toggle).not.toHaveTextContent("用量");
    expect(toggle.closest(".asb-iconcluster")).toBeTruthy();
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(await screen.findByRole("region", { name: "中继 A 用量" })).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("query_profile_usage", {
      profileId: "codex-relay-a",
    });
    const row = screen.getByRole("option", { name: /中继 A/ }).closest("li");
    expect(row?.querySelector(".asb-row-line + .asb-provider-usage")).toBeTruthy();

    // Collapsing reports the flip to the persisted settings owner.
    await user.click(toggle);
    expect(onToggleUsage).toHaveBeenCalledWith(configured);
  });

  it("keeps a persisted collapsed usage panel hidden until toggled again", async () => {
    const user = userEvent.setup();
    const onToggleUsage = vi.fn();
    const configured = {
      ...profiles[0],
      usageQuery: {
        kind: "declarative" as const,
        url: "{{baseUrl}}/balance",
        remainingPath: "balance",
        unit: "USD",
        refreshIntervalMinutes: 0,
      },
    };
    render(
      <ProviderList
        profiles={[configured]}
        activeProfileId={null}
        selectedId={null}
        collapsedUsageIds={["codex-relay-a"]}
        onSelect={() => {}}
        onToggleUsage={onToggleUsage}
      />,
    );

    expect(screen.queryByRole("region", { name: "中继 A 用量" })).not.toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("query_profile_usage", {
      profileId: "codex-relay-a",
    });
    const toggle = screen.getByRole("button", { name: "查看 中继 A 用量" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    await user.click(toggle);
    expect(onToggleUsage).toHaveBeenCalledWith(configured);
  });

  it("renders an official Codex quota ledger without custom usage settings", async () => {
    const official = {
      ...profiles[1],
      routeMode: "official" as const,
      baseUrl: null,
      apiKey: "",
    };
    invokeMock.mockResolvedValue({
      status: "available",
      windows: [{ label: "5 小时", usedPercent: 24, resetsAt: "2026-09-01T08:00:00Z" }],
      at: "2026-09-01T03:00:00Z",
      stale: false,
    });
    const onToggleUsage = vi.fn();

    render(
      <ProviderList
        profiles={[profiles[0], official]}
        activeProfileId={null}
        selectedId={null}
        onSelect={() => {}}
        onConfigureUsage={vi.fn()}
        onToggleUsage={onToggleUsage}
      />,
    );

    expect(
      await screen.findByRole("region", { name: "官方 OpenAI 官方订阅额度" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("row", { name: /5 小时/ })).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("query_codex_official_quota", {
      profileId: "codex-official",
    });
    expect(screen.queryByRole("button", { name: "配置 官方 OpenAI 用量" })).not.toBeInTheDocument();

    // The ledger unfolds by default and its icon toggles through the same
    // persisted usage-collapse owner as the custom panels.
    const toggle = screen.getByRole("button", { name: "收起 官方 OpenAI 订阅额度" });
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(toggle).toHaveAttribute("aria-controls", "codex-official-quota-codex-official");
    await userEvent.setup().click(toggle);
    expect(onToggleUsage).toHaveBeenCalledWith(official);
  });

  it("keeps an official Codex quota ledger collapsed when persisted so", () => {
    const official = {
      ...profiles[1],
      routeMode: "official" as const,
      baseUrl: null,
      apiKey: "",
    };

    render(
      <ProviderList
        profiles={[official]}
        activeProfileId={null}
        selectedId={null}
        collapsedUsageIds={["codex-official"]}
        onSelect={() => {}}
        onToggleUsage={() => {}}
      />,
    );

    expect(screen.queryByRole("region", { name: "官方 OpenAI 官方订阅额度" })).not.toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("query_codex_official_quota", {
      profileId: "codex-official",
    });
    expect(screen.getByRole("button", { name: "查看 官方 OpenAI 订阅额度" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
  });

  it("uses the same visible card action to configure an unconfigured provider", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    const onConfigureUsage = vi.fn();
    render(
      <ProviderList
        profiles={profiles}
        activeProfileId={null}
        selectedId={null}
        onSelect={onSelect}
        onConfigureUsage={onConfigureUsage}
      />,
    );

    const usageButton = screen.getByRole("button", { name: "配置 中继 A 用量" });
    expect(usageButton).toHaveClass("asb-btn-icon");
    expect(usageButton).not.toHaveTextContent("用量");
    expect(usageButton.closest(".asb-iconcluster")).toBeTruthy();
    expect(screen.queryByRole("region", { name: "中继 A 用量" })).not.toBeInTheDocument();
    await user.click(usageButton);
    expect(onConfigureUsage).toHaveBeenCalledWith(profiles[0]);
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("fires row actions directly without selecting the row", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    const onPreview = vi.fn();
    const onEdit = vi.fn();
    const onDelete = vi.fn();
    render(
      <ProviderList
        profiles={profiles}
        activeProfileId={null}
        selectedId={null}
        onSelect={onSelect}
        onPreview={onPreview}
        onEdit={onEdit}
        onDelete={onDelete}
      />,
    );

    await user.click(screen.getByRole("button", { name: "预览 中继 A 变更" }));
    expect(onPreview).toHaveBeenCalledWith(profiles[0]);
    await user.click(screen.getByRole("button", { name: "编辑 中继 A" }));
    expect(onEdit).toHaveBeenCalledWith(profiles[0]);
    await user.click(screen.getByRole("button", { name: "更多 官方 OpenAI 操作" }));
    await user.click(screen.getByRole("menuitem", { name: "删除 官方 OpenAI" }));
    expect(onDelete).toHaveBeenCalledWith(profiles[1]);
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("opens the more menu on the three-dot trigger and closes it after firing delete", async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    render(
      <ProviderList
        profiles={profiles}
        activeProfileId={null}
        selectedId={null}
        onSelect={() => {}}
        onDelete={onDelete}
      />,
    );

    expect(screen.queryByRole("menu", { name: "中继 A 更多操作" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "更多 中继 A 操作" }));
    const menu = screen.getByRole("menu", { name: "中继 A 更多操作" });
    expect(menu).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "删除 中继 A" })).toBeInTheDocument();

    await user.click(screen.getByRole("menuitem", { name: "删除 中继 A" }));
    expect(onDelete).toHaveBeenCalledWith(profiles[0]);
    expect(screen.queryByRole("menu", { name: "中继 A 更多操作" })).not.toBeInTheDocument();
  });

  it("closes the more menu with Escape while focus stays on the trigger", async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    render(
      <ProviderList
        profiles={profiles}
        activeProfileId={null}
        selectedId={null}
        onSelect={() => {}}
        onDelete={onDelete}
      />,
    );

    const trigger = screen.getByRole("button", { name: "更多 中继 A 操作" });
    await user.click(trigger);
    expect(screen.getByRole("menu", { name: "中继 A 更多操作" })).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu", { name: "中继 A 更多操作" })).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
    expect(onDelete).not.toHaveBeenCalled();
  });

  it("orders card actions as edit, preview, connectivity, usage, then the more trigger", () => {
    render(
      <ProviderList
        profiles={profiles}
        activeProfileId={null}
        selectedId={null}
        onSelect={() => {}}
        onConfigureUsage={() => {}}
        onPreview={() => {}}
        onEdit={() => {}}
        onDelete={() => {}}
      />,
    );

    const labels = Array.from(
      screen.getByRole("group", { name: "中继 A 操作" }).querySelectorAll("button"),
      (button) => button.getAttribute("aria-label"),
    );
    expect(labels).toEqual([
      "编辑 中继 A",
      "预览 中继 A 变更",
      "测试 中继 A 连通性",
      "配置 中继 A 用量",
      "更多 中继 A 操作",
    ]);
    expect(screen.queryByRole("button", { name: "删除 中继 A" })).not.toBeInTheDocument();
  });

  it("expands, collapses, then re-runs a provider endpoint test without selecting the row", async () => {
    invokeMock.mockResolvedValue({
      grade: "ok",
      status: 204,
      latencyMs: 320,
      error: null,
      at: "2026-08-31T08:00:00Z",
    });
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(
      <ProviderList
        profiles={profiles}
        activeProfileId={null}
        selectedId={null}
        onSelect={onSelect}
      />,
    );

    const button = screen.getByRole("button", { name: "测试 中继 A 连通性" });
    expect(button).toHaveClass("asb-btn-icon");
    expect(button).not.toHaveTextContent("连通");
    expect(button.closest(".asb-iconcluster")).toBeTruthy();

    await user.click(button);

    expect(invokeMock).toHaveBeenCalledWith("probe_endpoint", { url: "https://relay-a.internal/v1" });
    expect(await screen.findByText(/连通正常 · HTTP 204 · 320 毫秒/)).toBeInTheDocument();
    expect(button).toHaveAttribute("aria-describedby", "provider-probe-codex-relay-a");
    expect(button).toHaveAttribute("aria-expanded", "true");
    expect(onSelect).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "收起 中继 A 连通性结果" }));
    expect(screen.queryByText(/连通正常 · HTTP 204 · 320 毫秒/)).not.toBeInTheDocument();
    expect(button).toHaveAttribute("aria-expanded", "false");
    expect(button).not.toHaveAttribute("aria-describedby");
    expect(invokeMock).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole("button", { name: "测试 中继 A 连通性" }));
    expect(await screen.findByText(/连通正常 · HTTP 204 · 320 毫秒/)).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("collapses a failed provider endpoint result", async () => {
    invokeMock.mockRejectedValueOnce(new Error("连接超时"));
    const user = userEvent.setup();
    render(
      <ProviderList profiles={profiles} activeProfileId={null} selectedId={null} onSelect={() => {}} />,
    );

    await user.click(screen.getByRole("button", { name: "测试 中继 A 连通性" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("连接超时");

    await user.click(screen.getByRole("button", { name: "收起 中继 A 连通性结果" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("renders row-level 启用 on non-live rows, routing to the preview flow", async () => {
    const user = userEvent.setup();
    const onActivate = vi.fn();
    render(
      <ProviderList
        profiles={profiles}
        activeProfileId="codex-relay-a"
        selectedId="codex-official"
        onSelect={() => {}}
        onActivate={onActivate}
      />,
    );

    // The button stays in the tree; its pointer-only reveal is owned by the
    // stylesheet, and selection never changes that visual state.
    const button = screen.getByRole("button", { name: "启用 官方 OpenAI" });
    expect(button.classList.contains("asb-row-activate")).toBe(true);
    await user.click(button);
    expect(onActivate).toHaveBeenCalledWith(profiles[1]);
  });

  it("reveals 启用 only while its provider card is hovered", () => {
    const activationRule = baseCss.match(/\.asb-row-item:hover \.asb-row-activate \{[^}]+\}/)?.[0] ?? "";

    expect(activationRule).toContain("display: inline-flex");
    expect(baseCss).not.toContain(":focus-within .asb-row-activate");
    expect(baseCss).not.toContain('[aria-selected="true"] + .asb-row-activate');
  });

  it("keeps the host link quiet until hovered", () => {
    expect(baseCss).toMatch(/\.asb-row-host \{[^}]*color: inherit/);
    expect(baseCss).toMatch(/\.asb-row-host:hover \{[^}]*color: var\(--asb-action\)/);
  });

  it("keeps the live-matched row without a 启用 button even when selected", () => {
    render(
      <ProviderList
        profiles={profiles}
        activeProfileId="codex-relay-a"
        selectedId="codex-relay-a"
        onSelect={() => {}}
        onActivate={vi.fn()}
      />,
    );
    expect(screen.queryByRole("button", { name: "启用 中继 A" })).not.toBeInTheDocument();
  });

  it("renders drag handles only when reordering is enabled", () => {
    const { rerender } = render(
      <ProviderList profiles={profiles} activeProfileId={null} selectedId={null} onSelect={() => {}} />,
    );
    expect(screen.queryByRole("button", { name: /拖动调整/ })).not.toBeInTheDocument();

    rerender(
      <ProviderList
        profiles={profiles}
        activeProfileId={null}
        selectedId={null}
        onSelect={() => {}}
        onReorder={() => {}}
      />,
    );
    expect(screen.getByRole("button", { name: "拖动调整 中继 A 的顺序" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "拖动调整 官方 OpenAI 的顺序" })).toBeInTheDocument();
  });

  it("reports a keyboard drag reorder as the new id order", async () => {
    const onReorder = vi.fn();
    render(
      <ProviderList
        profiles={profiles}
        activeProfileId={null}
        selectedId={null}
        onSelect={() => {}}
        onReorder={onReorder}
      />,
    );

    // jsdom has no layout; give the rows real vertical geometry so the
    // sortable keyboard coordinate getter can resolve the next row.
    document.querySelectorAll("li.asb-row-item").forEach((row, index) => {
      const top = index * 76;
      row.getBoundingClientRect = () =>
        ({ top, bottom: top + 68, left: 0, right: 320, width: 320, height: 68, x: 0, y: top }) as DOMRect;
    });

    const grip = screen.getByRole("button", { name: "拖动调整 中继 A 的顺序" });
    grip.focus();
    // The sensor attaches its follow-up key handling on a macrotask and the
    // drag state must flush between keystrokes, so each key gets its own
    // awaited act block.
    const press = async (code: string, key: string) => {
      await act(async () => {
        fireEvent.keyDown(grip, { key, code, bubbles: true, cancelable: true });
        await new Promise((resolve) => setTimeout(resolve, 0));
      });
    };
    await press("Space", " ");
    await press("ArrowDown", "ArrowDown");
    await press("Space", " ");

    expect(onReorder).toHaveBeenCalledWith(["codex-official", "codex-relay-a"]);
  });
});
