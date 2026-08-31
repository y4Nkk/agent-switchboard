import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { CodePreview } from "./CodePreview";

const toml = [
  '# host comment',
  'threads = 8',
  'disable_response_storage = true',
  'base_url = "https://relay.example.internal/v1"',
  '[model_providers.openai]',
  '{}',
].join("\n");

describe("CodePreview", () => {
  it("renders numbered lines with token coloring for TOML and JSON shapes", () => {
    render(<CodePreview target="~/.codex/config.toml" content={toml} />);

    expect(screen.getByLabelText("~/.codex/config.toml 配置预览")).toBeInTheDocument();
    expect(screen.getByText("6 行")).toBeInTheDocument();
    expect(screen.getByText("1")).toBeInTheDocument();
    expect(screen.getByText("6")).toBeInTheDocument();
    // Values the overlay writes stay the anchor: booleans green, tables bold.
    expect(screen.getByText("true")).toHaveClass("asb-tok-bool");
    expect(screen.getByText("[model_providers.openai]")).toHaveClass("asb-tok-section");
    expect(screen.getByText("8")).toHaveClass("asb-tok-num");
  });
});
