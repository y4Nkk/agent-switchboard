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
      { key: "model_providers.asb.api_key", kind: "remove", before: "••••••••", after: null },
    ],
    warnings: ["该供应商未设置服务地址，将移除托管表中的 base_url"],
    backupDir: "F:/appdata/backups",
  },
};

describe("PreviewInspector", () => {
  it("names every changed key with before and after values", () => {
    render(<PreviewInspector filePreview={preview} />);
    expect(screen.getByText("model")).toBeInTheDocument();
    expect(screen.getByText("model_providers.asb.api_key")).toBeInTheDocument();
    expect(screen.getByText("gpt-5.1")).toBeInTheDocument();
    expect(screen.getByText("gpt-5.2")).toBeInTheDocument();
    expect(screen.getByText("••••••••")).toBeInTheDocument();
    expect(screen.getByText("移除")).toBeInTheDocument();
  });

  it("renders the pretty-printed candidate file so preserved host keys stay visible in place", () => {
    render(<PreviewInspector filePreview={preview} />);
    const fileView = screen.getByLabelText("~/.codex/config.toml 配置预览");
    expect(fileView).toHaveTextContent('model = "gpt-5.2"');
    expect(fileView).toHaveTextContent('[projects]');
    expect(fileView).toHaveTextContent('"f:\\projects\\wo" = "trusted"');
    expect(screen.getByText("3 行")).toBeInTheDocument();
    expect(screen.getByText(/服务地址/)).toBeInTheDocument();
    expect(screen.getByText("F:/appdata/backups")).toBeInTheDocument();
  });

  it("never renders raw config text or paths it was not given", () => {
    render(<PreviewInspector filePreview={preview} />);
    expect(screen.queryByText(/config\.toml\.tmp/)).toBeNull();
    expect(screen.queryByText(/writeFile/)).toBeNull();
  });
});

describe("PreviewInspector empty states", () => {
  it("shows an explicit empty state without inventing data", () => {
    render(<PreviewInspector filePreview={null} />);
    expect(screen.getByText("选择供应商后生成预览")).toBeInTheDocument();
  });
});
