import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { useCloudBackup } from "../app/useCloudBackup";
import { BackupsPage } from "./BackupsPage";

const cloudBackup: ReturnType<typeof useCloudBackup> = {
  settings: null,
  loaded: true,
  saveSettings: vi.fn().mockResolvedValue(true),
  testConnection: vi.fn().mockResolvedValue(true),
  upload: vi.fn().mockResolvedValue(true),
  restore: vi.fn().mockResolvedValue(true),
};

describe("BackupsPage", () => {
  it("separates local history and encrypted cloud backup into two tabs", async () => {
    const user = userEvent.setup();
    render(
      <BackupsPage
        records={[]}
        busy={false}
        lastSwitch={null}
        cloudBackup={cloudBackup}
        onRestore={vi.fn()}
        onUndo={vi.fn()}
        onOpenDir={vi.fn()}
      />,
    );

    const localTab = screen.getByRole("tab", { name: "本地备份" });
    const cloudTab = screen.getByRole("tab", { name: "加密云端备份" });
    expect(localTab).toHaveAttribute("aria-selected", "true");
    const localPanel = screen.getByRole("tabpanel", { name: "本地备份" });
    expect(localPanel).toHaveClass("asb-backup-local");
    expect(localPanel).toHaveTextContent("暂无备份");
    expect(screen.queryByLabelText("Supabase 云端备份设置")).not.toBeInTheDocument();

    await user.click(cloudTab);

    expect(cloudTab).toHaveAttribute("aria-selected", "true");
    expect(localTab).toHaveAttribute("aria-selected", "false");
    expect(screen.getByRole("tabpanel", { name: "加密云端备份" })).toBeInTheDocument();
    expect(screen.getByLabelText("Supabase 云端备份设置")).toBeInTheDocument();
    expect(screen.queryByText("暂无备份")).not.toBeInTheDocument();
  });
});
