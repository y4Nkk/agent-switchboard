import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { PreviewInspector } from "./PreviewInspector";
import type { FilePreview } from "../api/client";

const preview: FilePreview = {
  contentHash: "abc123",
  renderedHash: "rendered-abc123",
  content: 'model = "gpt-5.2"\n[projects]\n"f:\\projects\\wo" = "trusted"\n',
  preview: {
    app: "codex",
    target: "~/.codex/config.toml",
    changes: [
      { key: "model", kind: "set", before: "gpt-5.1", after: "gpt-5.2" },
      { key: "experimental_bearer_token", kind: "remove", before: "••••••••", after: null },
    ],
    warnings: ["将使用 Codex 内置 OpenAI 路由覆盖服务地址"],
    backupDir: "F:/appdata/backups",
  },
};

describe("PreviewInspector", () => {
  it("names every changed key with before and after values", () => {
    render(<PreviewInspector filePreview={preview} userConfigModel={null} userConfigWarnings={[]} />);
    expect(screen.getByText("model")).toBeInTheDocument();
    expect(screen.getByText("experimental_bearer_token")).toBeInTheDocument();
    expect(screen.getByText("gpt-5.1")).toBeInTheDocument();
    expect(screen.getByText("gpt-5.2")).toBeInTheDocument();
    expect(screen.getByText("••••••••")).toBeInTheDocument();
    expect(screen.getByText("移除")).toBeInTheDocument();
  });

  it("renders the pretty-printed candidate file so preserved host keys stay visible in place", () => {
    render(<PreviewInspector filePreview={preview} userConfigModel={null} userConfigWarnings={[]} />);
    const fileView = screen.getByLabelText("~/.codex/config.toml 配置预览");
    expect(fileView).toHaveTextContent('model = "gpt-5.2"');
    expect(fileView).toHaveTextContent('[projects]');
    expect(fileView).toHaveTextContent('"f:\\projects\\wo" = "trusted"');
    expect(screen.getByText("3 行")).toBeInTheDocument();
    expect(screen.getByText(/服务地址/)).toBeInTheDocument();
    expect(screen.getByText("F:/appdata/backups")).toBeInTheDocument();
  });

  it("never renders raw config text or paths it was not given", () => {
    render(<PreviewInspector filePreview={preview} userConfigModel={null} userConfigWarnings={[]} />);
    expect(screen.queryByText(/config\.toml\.tmp/)).toBeNull();
    expect(screen.queryByText(/writeFile/)).toBeNull();
  });

  it("shows the user-level configuration model and its scope warning", () => {
    const { rerender } = render(
      <PreviewInspector
        filePreview={preview}
        userConfigModel="glm-4.6"
        userConfigWarnings={["使用 --profile 启动时会覆盖这里的用户级设置"]}
      />,
    );
    expect(screen.getByText("当前用户级配置模型")).toBeInTheDocument();
    expect(screen.getByText("glm-4.6")).toBeInTheDocument();
    expect(screen.getByText("使用 --profile 启动时会覆盖这里的用户级设置")).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "范围警告" })).toBeInTheDocument();

    rerender(<PreviewInspector filePreview={preview} userConfigModel={null} userConfigWarnings={[]} />);
    expect(screen.getByText("当前用户级配置模型")).toBeInTheDocument();
    expect(screen.getByText("默认模型")).toBeInTheDocument();
  });
});

describe("PreviewInspector empty states", () => {
  it("shows an explicit empty state without inventing data", () => {
    render(<PreviewInspector filePreview={null} userConfigModel={null} userConfigWarnings={[]} />);
    expect(screen.getByText("选择供应商后生成预览")).toBeInTheDocument();
  });
});
