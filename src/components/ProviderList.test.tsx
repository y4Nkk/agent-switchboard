import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ProviderList } from "./ProviderList";
import type { ProviderProfile } from "../api/client";

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

const profiles: ProviderProfile[] = [
  {
    id: "codex-relay-a",
    app: "codex",
    name: "中继 A",
    model: "gpt-5.1",
    baseUrl: "https://relay-a.internal/v1",
    apiKey: "ASB_RELAY_A_KEY",
    modelOptions: null,
  },
  {
    id: "codex-official",
    app: "codex",
    name: "官方 OpenAI",
    model: null,
    baseUrl: "https://api.openai.com/v1",
    apiKey: "test-api-key",
    modelOptions: null,
  },
];

describe("ProviderList", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("renders the service address as a link on its own line, model below", () => {
    render(
      <ProviderList profiles={profiles} activeProfileId={null} selectedId="codex-relay-a" onSelect={() => {}} />,
    );
    expect(screen.getByRole("option", { name: /官方 OpenAI/ })).toBeInTheDocument();
    const link = screen.getByRole("link", { name: "relay-a.internal" });
    expect(link).toHaveAttribute("href", "https://relay-a.internal/v1");
    expect(link.classList.contains("asb-row-meta")).toBe(true);
    expect(link.textContent).toBe("relay-a.internal");
    expect(
      screen.getByText(
        (_, el) => el?.classList.contains("asb-row-meta") === true && el.textContent === "gpt-5.1",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("api.openai.com")).toBeInTheDocument();
  });

  it("opens the service URL in the system browser on click, not in-app", async () => {
    const user = userEvent.setup();
    vi.mocked(openUrl).mockClear();
    render(
      <ProviderList profiles={profiles} activeProfileId={null} selectedId={null} onSelect={() => {}} />,
    );
    await user.click(screen.getByRole("link", { name: "relay-a.internal" }));
    expect(openUrl).toHaveBeenCalledWith("https://relay-a.internal/v1");
  });

  it("marks the live-matched row with a status pill, not color alone", () => {
    render(
      <ProviderList profiles={profiles} activeProfileId="codex-relay-a" selectedId={null} onSelect={() => {}} />,
    );
    expect(screen.getByText("使用中")).toBeInTheDocument();
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
    expect(selectedRow.closest("li")).not.toHaveClass("is-selected");
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
    expect(row?.querySelector(".asb-row-line + .asb-preview-inline")).toHaveTextContent(
      "预览内容 codex-relay-a",
    );
    expect(screen.getAllByText(/预览内容/)).toHaveLength(1);
  });

  it("unfolds a configured usage readout inside its provider card", async () => {
    const user = userEvent.setup();
    const configured = {
      ...profiles[0],
      usageQuery: {
        kind: "declarative" as const,
        url: "{{baseUrl}}/balance",
        remainingPath: "balance",
        unit: "USD",
      },
    };
    invokeMock.mockResolvedValue({
      remaining: 18.5,
      used: 7,
      total: 25.5,
      unit: "USD",
      at: "2026-08-31T08:00:00Z",
    });
    render(
      <ProviderList
        profiles={[configured, profiles[1]]}
        activeProfileId={null}
        selectedId={null}
        onSelect={() => {}}
      />,
    );

    const toggle = screen.getByRole("button", { name: "查看 中继 A 用量" });
    expect(toggle).toHaveClass("asb-btn-icon");
    expect(toggle).not.toHaveTextContent("用量");
    expect(toggle.closest(".asb-iconcluster")).toBeTruthy();
    await user.click(toggle);

    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(await screen.findByRole("region", { name: "中继 A 用量" })).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("test_usage_query", {
      query: configured.usageQuery,
      apiKey: "ASB_RELAY_A_KEY",
      baseUrl: "https://relay-a.internal/v1",
    });
    const row = screen.getByRole("option", { name: /中继 A/ }).closest("li");
    expect(row?.querySelector(".asb-row-line + .asb-provider-usage")).toBeTruthy();
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
    await user.click(screen.getByRole("button", { name: "删除 官方 OpenAI" }));
    expect(onDelete).toHaveBeenCalledWith(profiles[1]);
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("tests a provider endpoint from the icon action without selecting the row", async () => {
    invokeMock.mockResolvedValueOnce({
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
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("renders row-level 启用 on unselected rows too, routing to the preview flow", async () => {
    const user = userEvent.setup();
    const onActivate = vi.fn();
    render(
      <ProviderList
        profiles={profiles}
        activeProfileId="codex-relay-a"
        selectedId="codex-relay-a"
        onSelect={() => {}}
        onActivate={onActivate}
      />,
    );

    // Reveal-on-hover/focus/selected is owned by the stylesheet, so the
    // button is in the tree (and actionable) even before the row is
    // selected.
    const button = screen.getByRole("button", { name: "启用 官方 OpenAI" });
    expect(button.classList.contains("asb-row-activate")).toBe(true);
    await user.click(button);
    expect(onActivate).toHaveBeenCalledWith(profiles[1]);
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
