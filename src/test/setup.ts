import "@testing-library/jest-dom/vitest";
import { afterEach, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import { cloneElement, createElement, isValidElement, type ReactElement, type ReactNode } from "react";
import { clearToasts } from "../components/use-toast";

// jsdom reports every element as 0x0 and has no ResizeObserver, so Recharts'
// ResponsiveContainer never measures a positive size and renders nothing.
// Swap in a fixed-size passthrough that stamps the plot size onto the chart
// element; plot geometry is not under test.
vi.mock("recharts", async (importOriginal) => {
  const original = await importOriginal<typeof import("recharts")>();
  return {
    ...original,
    ResponsiveContainer: ({ children }: { children: ReactNode }) =>
      createElement(
        "div",
        { style: { width: 640, height: 240 } },
        isValidElement(children)
          ? cloneElement(children as ReactElement<{ width?: number; height?: number }>, {
              width: 640,
              height: 240,
            })
          : children,
      ),
  };
});

// jsdom has no canvas implementation and logs an error on getContext;
// the particle effect intentionally bails out when the context is null.
HTMLCanvasElement.prototype.getContext = () => null;

// jsdom lacks the pointer-capture APIs Radix Select uses for its listbox
// pointer handling, and scrollIntoView for focusing the open menu's chosen
// item; stub them so dropdown interactions can be tested.
Element.prototype.hasPointerCapture = () => false;
Element.prototype.setPointerCapture = () => {};
Element.prototype.releasePointerCapture = () => {};
Element.prototype.scrollIntoView = () => {};

afterEach(() => {
  cleanup();
  // Error toasts never auto-close; the module-level store must not leak
  // alerts from one test into the next.
  clearToasts();
});
