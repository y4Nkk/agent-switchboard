import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";
import { clearToasts } from "../components/use-toast";

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
