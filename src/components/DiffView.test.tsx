import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { DiffView } from "./DiffView";
import type { KeyChange } from "../api/client";

const changes: KeyChange[] = [
  { key: "model", kind: "set", before: "gpt-5.1", after: "gpt-5.2" },
  { key: "experimental_bearer_token", kind: "remove", before: "••••••••", after: null },
];

describe("DiffView", () => {
  it("names every changed key with its red before → green after line", () => {
    render(<DiffView changes={changes} label="变更键" />);

    expect(screen.getByLabelText("变更键")).toBeInTheDocument();
    expect(screen.getByText("model")).toBeInTheDocument();
    expect(screen.getByText("gpt-5.1")).toBeInTheDocument();
    expect(screen.getByText("gpt-5.2")).toBeInTheDocument();
    // Outgoing reads red, incoming green; removal is red and worded.
    expect(screen.getByText("gpt-5.1")).toHaveClass("asb-diff-old");
    expect(screen.getByText("gpt-5.2")).toHaveClass("asb-diff-new");
    expect(screen.getByText("••••••••")).toHaveClass("asb-diff-old");
    expect(screen.getByText("移除")).toHaveClass("asb-diff-remove");
  });

  it("keeps an absent before value neutral — nothing is being removed", () => {
    render(
      <DiffView
        changes={[{ key: "hide_agent_reasoning", kind: "set", before: null, after: "true" }]}
        label="差异"
      />,
    );
    expect(screen.getByText("（无）")).toHaveClass("asb-diff-none");
    expect(screen.getByText("true")).toHaveClass("asb-diff-new");
  });
});
